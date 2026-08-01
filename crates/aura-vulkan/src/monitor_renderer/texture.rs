use super::{MonitorRenderer, resources};
use crate::{context::VulkanContext, error::VulkanError, texture::GpuTexture};
use ash::vk;

impl MonitorRenderer {
    pub fn set_wallpaper_pixels(
        &mut self,
        context: &VulkanContext,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<(), VulkanError> {
        // Switching back to a static texture: the previously presented video
        // frame (and its DPB slot) is released. The caller acks the slot so
        // the decode worker may reuse it.
        self.active_video_frame = None;

        let buffer_size = pixels.len() as u64;
        if buffer_size == 0 {
            return Ok(());
        }

        // 1. Ensure texture exists with correct dimensions.
        let needs_recreate = match &self.active_texture {
            Some(t) => t.width != width || t.height != height,
            None => true,
        };

        if needs_recreate {
            unsafe {
                context.device.device_wait_idle().ok();
            }
            if let Some(mut old_t) = self.active_texture.take() {
                unsafe { old_t.destroy(context) };
            }
            let new_t = GpuTexture::create_2d(context, width, height, vk::Format::R8G8B8A8_UNORM)?;
            self.active_texture = Some(new_t);
        }

        if let Some(ref mut texture) = self.active_texture {
            self.uploader.upload_pixels(context, texture, pixels)?;
            resources::update_descriptor_set(
                context,
                self.descriptor_set,
                texture,
                self.active_fit_mode,
                self.repeat_sampler,
                self.border_sampler,
            );
        }

        Ok(())
    }

    /// Free CPU-to-GPU staging buffer allocation to reclaim host RAM.
    pub fn trim_staging(&mut self, context: &VulkanContext) {
        self.uploader.trim(context);
    }
}
