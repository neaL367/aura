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

    /// Wait for in-flight GPU rendering to complete without resetting the fence.
    /// Use when reset must be deferred until after a fallible operation (e.g.
    /// swapchain image acquisition) so a failed acquire does not leave the fence
    /// unsignaled.
    pub fn wait_for_fence(&self, device: &ash::Device) -> Result<(), VulkanError> {
        unsafe {
            device
                .wait_for_fences(std::slice::from_ref(&self.in_flight_fence), true, u64::MAX)
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?;
        }
        Ok(())
    }

    /// Wait for the in-flight fence for a bounded interval.
    /// Returns `Ok(false)` when the interval expires, allowing render threads
    /// to observe shutdown instead of blocking forever inside Vulkan.
    pub fn wait_for_fence_timeout(
        &self,
        device: &ash::Device,
        timeout_ns: u64,
    ) -> Result<bool, VulkanError> {
        unsafe {
            match device.wait_for_fences(
                std::slice::from_ref(&self.in_flight_fence),
                true,
                timeout_ns,
            ) {
                Ok(()) => Ok(true),
                Err(vk::Result::TIMEOUT) => Ok(false),
                Err(e) => Err(VulkanError::FrameSync(e.to_string())),
            }
        }
    }

    /// Reset the fence to unsignaled. Must only be called after a successful
    /// acquire — the matching queue submit will signal it again.
    pub fn reset_fence(&self, device: &ash::Device) -> Result<(), VulkanError> {
        unsafe {
            device
                .reset_fences(std::slice::from_ref(&self.in_flight_fence))
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?;
        }
        Ok(())
    }

    /// Wait for in-flight GPU rendering to complete and reset the fence for the next frame.
    pub fn wait_and_reset(&self, device: &ash::Device) -> Result<(), VulkanError> {
        self.wait_for_fence(device)?;
        self.reset_fence(device)
    }

    /// Destroy synchronization handles.
    ///
    /// # Safety
    /// Must be called when GPU execution using these sync objects has completed.
    pub unsafe fn destroy(&mut self, device: &ash::Device) {
        unsafe {
            if self.image_available_semaphore != vk::Semaphore::null() {
                device.destroy_semaphore(self.image_available_semaphore, None);
                self.image_available_semaphore = vk::Semaphore::null();
            }
            if self.render_finished_semaphore != vk::Semaphore::null() {
                device.destroy_semaphore(self.render_finished_semaphore, None);
                self.render_finished_semaphore = vk::Semaphore::null();
            }
            if self.in_flight_fence != vk::Fence::null() {
                device.destroy_fence(self.in_flight_fence, None);
                self.in_flight_fence = vk::Fence::null();
            }
        }
    }
}
