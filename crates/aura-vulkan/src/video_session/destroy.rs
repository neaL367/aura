use crate::context::VulkanContext;
use crate::video_session::VulkanVideoSession;
use ash::vk;

impl VulkanVideoSession {
    /// Clean up the session, parameters, conversion, and DPB images.
    ///
    /// # Safety
    /// Must be called when GPU execution using this session has completed.
    pub unsafe fn destroy(&mut self, context: &VulkanContext) {
        let Some(loader) = context.video_queue_device_loader.as_ref() else {
            return;
        };

        if self.session_parameters != vk::VideoSessionParametersKHR::null() {
            unsafe {
                (loader.fp().destroy_video_session_parameters_khr)(
                    context.device.handle(),
                    self.session_parameters,
                    std::ptr::null(),
                );
            }
            self.session_parameters = vk::VideoSessionParametersKHR::null();
        }

        if self.session != vk::VideoSessionKHR::null() {
            unsafe {
                (loader.fp().destroy_video_session_khr)(
                    context.device.handle(),
                    self.session,
                    std::ptr::null(),
                );
            }
            self.session = vk::VideoSessionKHR::null();
        }

        if self.ycbcr_conversion != vk::SamplerYcbcrConversion::null() {
            unsafe {
                context
                    .device
                    .destroy_sampler_ycbcr_conversion(self.ycbcr_conversion, None);
            }
            self.ycbcr_conversion = vk::SamplerYcbcrConversion::null();
        }

        let mut allocator_lock = context.allocator.lock().unwrap();
        for slot in self.dpb_slots.drain(..) {
            unsafe {
                if slot.view != vk::ImageView::null() {
                    context.device.destroy_image_view(slot.view, None);
                }
                if slot.image != vk::Image::null() {
                    context.device.destroy_image(slot.image, None);
                }
                if let (Some(allocator), Some(alloc)) = (allocator_lock.as_mut(), slot.allocation) {
                    let _ = allocator.free(alloc);
                }
            }
        }
    }
}

impl Drop for VulkanVideoSession {
    fn drop(&mut self) {
        // destroy() requires the context; if it was never destroyed (e.g.
        // construction failed partway), the handles are already nulled.
        if self.session != vk::VideoSessionKHR::null()
            || self.session_parameters != vk::VideoSessionParametersKHR::null()
        {
            tracing::error!(
                "VulkanVideoSession dropped without destroy() — leaking session handles"
            );
        }
    }
}
