use super::{MonitorRenderer, resources};
use crate::context::VulkanContext;
use ash::vk;

impl MonitorRenderer {
    /// Clean up all GPU resources.
    ///
    /// # Safety
    /// Must be called when the GPU is idle before destroying `VulkanContext`.
    pub unsafe fn destroy(&mut self, context: &VulkanContext) {
        unsafe {
            if self.command_pool == vk::CommandPool::null() {
                return;
            }
            context.device.device_wait_idle().ok();

            if let Some(mut texture) = self.active_texture.take() {
                texture.destroy(context);
            }

            self.uploader.destroy(context);

            if self.repeat_sampler != vk::Sampler::null() {
                context.device.destroy_sampler(self.repeat_sampler, None);
                self.repeat_sampler = vk::Sampler::null();
            }

            if self.border_sampler != vk::Sampler::null() {
                context.device.destroy_sampler(self.border_sampler, None);
                self.border_sampler = vk::Sampler::null();
            }

            if self.video_sampler != vk::Sampler::null() {
                context.device.destroy_sampler(self.video_sampler, None);
                self.video_sampler = vk::Sampler::null();
            }

            self.active_video_frame = None;
            resources::destroy_framebuffers(&context.device, &mut self.framebuffers);
            if self.descriptor_pool != vk::DescriptorPool::null() {
                context
                    .device
                    .destroy_descriptor_pool(self.descriptor_pool, None);
                self.descriptor_pool = vk::DescriptorPool::null();
            }
            if self.command_pool != vk::CommandPool::null() {
                context.device.destroy_command_pool(self.command_pool, None);
                self.command_pool = vk::CommandPool::null();
            }
            self.pipeline.destroy(&context.device);
            self.frame_sync.destroy(&context.device);
            self.swapchain.destroy(&context.device);
        }
    }
}
