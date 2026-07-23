use ash::vk;

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

        // Wait for previous upload to complete.
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

        // Create / resize staging buffer if needed.
        if self.staging_size < buffer_size {
            if let Some(buf) = self.staging_buffer.take() {
                unsafe { context.device.destroy_buffer(buf, None) };
            }
            if let Some(alloc) = self.staging_allocation.take()
                && let Ok(mut guard) = context.allocator.lock()
                && let Some(ref mut allocator) = *guard
            {
                let _ = allocator.free(alloc);
            }

            let buffer_info = vk::BufferCreateInfo::default()
                .size(buffer_size)
                .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let new_buffer = unsafe {
                context
                    .device
                    .create_buffer(&buffer_info, None)
                    .map_err(|e| VulkanError::Upload(e.to_string()))?
            };

            let reqs = unsafe { context.device.get_buffer_memory_requirements(new_buffer) };

            let new_alloc = {
                let mut guard = context.allocator.lock().unwrap();
                let alloc = guard.as_mut().ok_or_else(|| {
                    VulkanError::Allocation("Allocator missing during staging upload".to_string())
                })?;
                alloc
                    .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                        name: "Staging Buffer",
                        requirements: reqs,
                        location: gpu_allocator::MemoryLocation::CpuToGpu,
                        linear: true,
                        allocation_scheme:
                            gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
                    })
                    .map_err(|e| VulkanError::Allocation(e.to_string()))?
            };

            unsafe {
                context
                    .device
                    .bind_buffer_memory(new_buffer, new_alloc.memory(), new_alloc.offset())
                    .map_err(|e| VulkanError::Upload(e.to_string()))?;
            }

            self.staging_buffer = Some(new_buffer);
            self.staging_allocation = Some(new_alloc);
            self.staging_size = buffer_size;
        }

        // Map and copy pixel data into staging buffer.
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

        // Record upload command buffer.
        unsafe {
            context
                .device
                .reset_command_buffer(
                    self.upload_command_buffer,
                    vk::CommandBufferResetFlags::empty(),
                )
                .ok();
        }

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        let current_layout = texture.layout;
        let (src_stage, src_access) = if current_layout == vk::ImageLayout::UNDEFINED {
            (
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::AccessFlags::empty(),
            )
        } else {
            (
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::AccessFlags::SHADER_READ,
            )
        };

        unsafe {
            context
                .device
                .begin_command_buffer(self.upload_command_buffer, &begin_info)
                .map_err(|e| VulkanError::Upload(e.to_string()))?;

            let barrier_to_transfer = vk::ImageMemoryBarrier::default()
                .old_layout(current_layout)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(texture.image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .src_access_mask(src_access)
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);

            context.device.cmd_pipeline_barrier(
                self.upload_command_buffer,
                src_stage,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier_to_transfer],
            );

            let copy_region = vk::BufferImageCopy::default()
                .buffer_offset(0)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                .image_extent(vk::Extent3D {
                    width: texture.width,
                    height: texture.height,
                    depth: 1,
                });

            let staging_buf = self
                .staging_buffer
                .ok_or_else(|| VulkanError::Upload("No staging buffer available".to_string()))?;

            context.device.cmd_copy_buffer_to_image(
                self.upload_command_buffer,
                staging_buf,
                texture.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[copy_region],
            );

            let barrier_to_shader = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(texture.image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ);

            context.device.cmd_pipeline_barrier(
                self.upload_command_buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier_to_shader],
            );

            context
                .device
                .end_command_buffer(self.upload_command_buffer)
                .map_err(|e| VulkanError::Upload(e.to_string()))?;
        }

        texture.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

        let submit_info = vk::SubmitInfo::default()
            .command_buffers(std::slice::from_ref(&self.upload_command_buffer));

        let _lock = context.queue_lock();

        unsafe {
            context
                .device
                .queue_submit(context.graphics_queue, &[submit_info], self.upload_fence)
                .map_err(|e| VulkanError::Upload(e.to_string()))?;

            context
                .device
                .wait_for_fences(std::slice::from_ref(&self.upload_fence), true, u64::MAX)
                .map_err(|e| VulkanError::Upload(e.to_string()))?;
        }

        Ok(())
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
        if let Some(buf) = self.staging_buffer.take() {
            unsafe { context.device.destroy_buffer(buf, None) };
        }
        if let Some(alloc) = self.staging_allocation.take()
            && let Ok(mut guard) = context.allocator.lock()
            && let Some(ref mut allocator) = *guard
        {
            let _ = allocator.free(alloc);
        }
        self.staging_size = 0;
    }

    /// Clean up staging buffer and fence handles.
    ///
    /// # Safety
    /// Must be called when GPU execution using this uploader has completed.
    pub unsafe fn destroy(&mut self, context: &VulkanContext) {
        unsafe {
            if let Some(buf) = self.staging_buffer.take() {
                context.device.destroy_buffer(buf, None);
            }
            if let Some(alloc) = self.staging_allocation.take()
                && let Ok(mut guard) = context.allocator.lock()
                && let Some(ref mut allocator) = *guard
            {
                let _ = allocator.free(alloc);
            }
            if self.upload_fence != vk::Fence::null() {
                context.device.destroy_fence(self.upload_fence, None);
                self.upload_fence = vk::Fence::null();
            }
        }
    }
}
