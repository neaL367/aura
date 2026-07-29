pub mod allocator;
pub mod uploader;

use ash::vk;

use allocator::{ensure_staging_buffer, trim_staging_buffer};
use uploader::record_and_submit_upload;

use crate::{context::VulkanContext, error::VulkanError, texture::GpuTexture};

/// Manages a persistent CPU-to-GPU staging buffer and fence-synchronized
/// texture upload commands without `device_wait_idle`.
pub struct StagingUploader {
    pub staging_buffer: Option<vk::Buffer>,
    pub staging_allocation: Option<gpu_allocator::vulkan::Allocation>,
    pub staging_size: u64,
    pub upload_fence: vk::Fence,
    pub upload_command_buffer: vk::CommandBuffer,
}

impl StagingUploader {
    pub fn create(
        context: &VulkanContext,
        command_buffer: vk::CommandBuffer,
    ) -> Result<Self, VulkanError> {
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        let upload_fence = unsafe {
            context
                .device
                .create_fence(&fence_info, None)
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?
        };

        Ok(Self {
            staging_buffer: None,
            staging_allocation: None,
            staging_size: 0,
            upload_fence,
            upload_command_buffer: command_buffer,
        })
    }

    pub fn upload_pixels(
        &mut self,
        context: &VulkanContext,
        texture: &mut GpuTexture,
        pixels: &[u8],
    ) -> Result<(), VulkanError> {
        let buffer_size = pixels.len() as u64;
        if buffer_size == 0 {
            return Ok(());
        }

        unsafe {
            context
                .device
                .wait_for_fences(std::slice::from_ref(&self.upload_fence), true, u64::MAX)
                .map_err(|e| VulkanError::Upload(e.to_string()))?;
            context
                .device
                .reset_fences(std::slice::from_ref(&self.upload_fence))
                .map_err(|e| VulkanError::Upload(e.to_string()))?;
        }

        ensure_staging_buffer(
            context,
            &mut self.staging_buffer,
            &mut self.staging_allocation,
            &mut self.staging_size,
            buffer_size,
        )?;

        if let Some(ref alloc) = self.staging_allocation {
            if let Some(mapped_ptr) = alloc.mapped_ptr() {
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        pixels.as_ptr(),
                        mapped_ptr.as_ptr() as *mut u8,
                        pixels.len(),
                    );
                }
            } else {
                unsafe {
                    let ptr = context
                        .device
                        .map_memory(
                            alloc.memory(),
                            alloc.offset(),
                            buffer_size,
                            vk::MemoryMapFlags::empty(),
                        )
                        .map_err(|e| VulkanError::Upload(e.to_string()))?;
                    std::ptr::copy_nonoverlapping(pixels.as_ptr(), ptr as *mut u8, pixels.len());
                    context.device.unmap_memory(alloc.memory());
                }
            }

            unsafe {
                let range = vk::MappedMemoryRange::default()
                    .memory(alloc.memory())
                    .offset(alloc.offset())
                    .size(vk::WHOLE_SIZE);
                let _ = context.device.flush_mapped_memory_ranges(&[range]);
            }
        }

        let staging_buf = self
            .staging_buffer
            .ok_or_else(|| VulkanError::Upload("No staging buffer available".to_string()))?;

        record_and_submit_upload(
            context,
            self.upload_command_buffer,
            staging_buf,
            texture,
            self.upload_fence,
        )
    }

    /// Free the CPU-to-GPU staging buffer allocation to reclaim host RAM when uploads are complete.
    pub fn trim(&mut self, context: &VulkanContext) {
        if self.upload_fence != vk::Fence::null() {
            unsafe {
                context
                    .device
                    .wait_for_fences(
                        std::slice::from_ref(&self.upload_fence),
                        true,
                        1_000_000_000,
                    )
                    .ok();
            }
        }
        trim_staging_buffer(
            context,
            &mut self.staging_buffer,
            &mut self.staging_allocation,
            &mut self.staging_size,
        );
    }

    /// Clean up staging buffer and fence handles.
    ///
    /// # Safety
    /// Must be called when GPU execution using this uploader has completed.
    pub unsafe fn destroy(&mut self, context: &VulkanContext) {
        unsafe {
            trim_staging_buffer(
                context,
                &mut self.staging_buffer,
                &mut self.staging_allocation,
                &mut self.staging_size,
            );
            if self.upload_fence != vk::Fence::null() {
                context.device.destroy_fence(self.upload_fence, None);
                self.upload_fence = vk::Fence::null();
            }
        }
    }
}
