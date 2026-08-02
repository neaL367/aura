pub mod frame_pass;
pub mod resources;

use ash::vk;
use aura_core::{monitor::MonitorId, wallpaper::FitMode};

use crate::{
    context::VulkanContext, error::VulkanError, frame::FrameSync, pipeline::GraphicsPipeline,
    staging::StagingUploader, surface::Surface, swapchain::Swapchain, texture::GpuTexture,
};

use std::sync::Arc;

mod create;
mod destroy;
mod texture;
mod video;

use video::ActiveVideoFrame;

pub struct MonitorRenderer {
    pub monitor_id: MonitorId,
    pub surface: Surface,
    pub swapchain: Swapchain,
    pub pipeline: GraphicsPipeline,
    pub frame_sync: FrameSync,
    pub command_pool: vk::CommandPool,
    pub command_buffer: vk::CommandBuffer,
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptor_set: vk::DescriptorSet,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub active_texture: Option<GpuTexture>,
    pub active_fit_mode: FitMode,
    pub repeat_sampler: vk::Sampler,
    pub border_sampler: vk::Sampler,
    /// Sampler for direct DPB (NV12 + YCbCr conversion) sampling.
    pub video_sampler: vk::Sampler,
    /// Currently presented Vulkan Video frame (sampled from the DPB).
    pub active_video_frame: Option<ActiveVideoFrame>,
    /// Graphics-timeline counter signaled after each video frame sampling.
    gfx_timeline_value: u64,
    /// Semaphore of the current decode worker's pipeline (reset the counter
    /// when a new worker/pipeline arrives).
    current_gfx_timeline: vk::Semaphore,
    pub uploader: StagingUploader,
    pub virtual_x: i32,
    pub virtual_y: i32,
    pub virtual_desktop_width: u32,
    pub virtual_desktop_height: u32,
    pub context: Arc<VulkanContext>,
}

impl MonitorRenderer {
    /// Acquire, draw, and present one frame.
    pub fn frame(
        &mut self,
        context: &VulkanContext,
        clear_color: [f32; 4],
    ) -> Result<(), VulkanError> {
        frame_pass::execute_frame(self, context, clear_color)
    }

    pub fn set_virtual_geometry(&mut self, mon_x: i32, mon_y: i32, total_w: u32, total_h: u32) {
        self.virtual_x = mon_x;
        self.virtual_y = mon_y;
        self.virtual_desktop_width = total_w;
        self.virtual_desktop_height = total_h;
    }

    /// Update active fit mode and update descriptor set sampler if needed.
    pub fn set_fit_mode(&mut self, fit_mode: FitMode, context: &VulkanContext) {
        self.active_fit_mode = fit_mode;
        if self.active_video_frame.is_some() {
            if let Some(ref frame) = self.active_video_frame {
                resources::update_video_descriptor_set(
                    context,
                    self.descriptor_set,
                    frame.image_view,
                    self.video_sampler,
                );
            }
        } else if let Some(ref texture) = self.active_texture {
            resources::update_descriptor_set(
                context,
                self.descriptor_set,
                texture,
                fit_mode,
                self.repeat_sampler,
                self.border_sampler,
            );
        }
    }

    /// Recreate the swapchain and framebuffers after resolution change.
    pub fn resize(
        &mut self,
        context: &VulkanContext,
        width: u32,
        height: u32,
    ) -> Result<(), VulkanError> {
        unsafe {
            context.device.device_wait_idle().ok();
        }

        let old_swapchain = self.swapchain.swapchain;
        let new_swapchain =
            Swapchain::create(context, &self.surface, width, height, old_swapchain)?;

        unsafe {
            resources::destroy_framebuffers(&context.device, &mut self.framebuffers);
            self.swapchain.destroy(&context.device);
        }
        self.swapchain = new_swapchain;
        self.framebuffers =
            resources::create_framebuffers(context, &self.pipeline, &self.swapchain)?;

        Ok(())
    }

    /// Clear the active wallpaper, releasing textures and stopping media playback.
    /// The descriptor set is updated to sample from a neutral 1x1 black texture.
    pub fn clear(&mut self, context: &VulkanContext) {
        if let Some(mut texture) = self.active_texture.take() {
            unsafe {
                texture.destroy(context);
            }
        }
        self.clear_video_frame(context);
    }
}

impl Drop for MonitorRenderer {
    fn drop(&mut self) {
        let context = self.context.clone();
        unsafe {
            self.destroy(&context);
        }
    }
}
