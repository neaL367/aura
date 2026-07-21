use ash::vk;

use crate::{context::VulkanContext, error::VulkanError};

/// Bounded per-frame Vulkan synchronization primitives (fences & semaphores).
pub struct FrameSync {
    pub image_available_semaphore: vk::Semaphore,
    pub render_finished_semaphore: vk::Semaphore,
    pub in_flight_fence: vk::Fence,
}

impl FrameSync {
    /// Create new synchronization objects (`image_available`, `render_finished`, `in_flight_fence`).
    pub fn new(context: &VulkanContext) -> Result<Self, VulkanError> {
        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

        let image_available_semaphore = unsafe {
            context
                .device
                .create_semaphore(&semaphore_info, None)
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?
        };

        let render_finished_semaphore = unsafe {
            context
                .device
                .create_semaphore(&semaphore_info, None)
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?
        };

        let in_flight_fence = unsafe {
            context
                .device
                .create_fence(&fence_info, None)
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?
        };

        Ok(Self {
            image_available_semaphore,
            render_finished_semaphore,
            in_flight_fence,
        })
    }

    /// Wait for in-flight GPU rendering to complete and reset the fence for the next frame.
    pub fn wait_and_reset(&self, device: &ash::Device) -> Result<(), VulkanError> {
        unsafe {
            device
                .wait_for_fences(std::slice::from_ref(&self.in_flight_fence), true, u64::MAX)
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?;
            device
                .reset_fences(std::slice::from_ref(&self.in_flight_fence))
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?;
        }
        Ok(())
    }

    /// Destroy synchronization handles.
    ///
    /// # Safety
    /// Must be called when GPU execution using these sync objects has completed.
    pub unsafe fn destroy(&mut self, device: &ash::Device) {
        unsafe {
            device.destroy_semaphore(self.image_available_semaphore, None);
            device.destroy_semaphore(self.render_finished_semaphore, None);
            device.destroy_fence(self.in_flight_fence, None);
        }
    }
}
