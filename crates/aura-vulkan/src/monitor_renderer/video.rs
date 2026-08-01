use super::{MonitorRenderer, resources};
use crate::{context::VulkanContext, video_decode_pipeline::VideoGpuFrame};
use ash::vk;

/// A video frame currently presented by sampling a DPB image view directly.
pub struct ActiveVideoFrame {
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    /// Timeline semaphore the decode worker signals when the frame is done.
    pub timeline_semaphore: vk::Semaphore,
    pub timeline_value: u64,
    /// Graphics->video timeline: signaled (with `sampled_gfx_value`) in the
    /// presentation submit that samples this frame, so the decode worker
    /// never overwrites the slot while it is being sampled.
    pub gfx_timeline: vk::Semaphore,
    pub sampled_gfx_value: u64,
    pub slot_index: u32,
    pub width: u32,
    pub height: u32,
    /// True once the DPB slot was transitioned to `SHADER_READ_ONLY_OPTIMAL`.
    pub(super) layout_transitioned: bool,
}

impl MonitorRenderer {
    /// Present a hardware-decoded video frame by sampling its DPB image view
    /// directly (no host readback). Replaces the static texture, reclaiming
    /// its VRAM allocation.
    pub fn set_video_frame(&mut self, context: &VulkanContext, frame: &VideoGpuFrame) {
        // Release the static texture if one is still resident.
        if self.active_texture.is_some() {
            unsafe {
                context.device.device_wait_idle().ok();
            }
            if let Some(mut texture) = self.active_texture.take() {
                unsafe { texture.destroy(context) };
            }
        }

        // A new decode worker means a new graphics timeline (values restart
        // from zero); reset the counter and remember the semaphore.
        if self.current_gfx_timeline != frame.gfx_timeline {
            self.current_gfx_timeline = frame.gfx_timeline;
            self.gfx_timeline_value = 0;
        }

        self.active_video_frame = Some(ActiveVideoFrame {
            image: frame.image,
            image_view: frame.image_view,
            timeline_semaphore: frame.timeline_semaphore,
            timeline_value: frame.timeline_value,
            gfx_timeline: frame.gfx_timeline,
            sampled_gfx_value: 0,
            slot_index: frame.slot_index,
            width: frame.width,
            height: frame.height,
            layout_transitioned: false,
        });

        resources::update_video_descriptor_set(
            context,
            self.descriptor_set,
            frame.image_view,
            self.video_sampler,
        );
    }

    /// DPB slot of the currently presented video frame, if any. The caller
    /// uses this to ack slot reuse to the decode worker.
    pub fn active_video_slot(&self) -> Option<u32> {
        self.active_video_frame.as_ref().map(|f| f.slot_index)
    }

    /// Graphics-timeline value signaled after the currently presented video
    /// frame was sampled (0 when never sampled yet). The caller sends it to
    /// the decode worker in the slot-reuse ack.
    pub fn active_video_sampled_gfx_value(&self) -> u64 {
        self.active_video_frame
            .as_ref()
            .map(|f| f.sampled_gfx_value)
            .unwrap_or(0)
    }

    /// Drop the presented video frame so its DPB image view is no longer
    /// sampled (called by the render loop when the decode worker recreates
    /// its session and destroys the DPB images). The 1x1 black texture keeps
    /// the descriptor set valid; the caller acknowledges the reset after
    /// this returns.
    pub fn clear_video_frame(&mut self, context: &VulkanContext) {
        if self.active_video_frame.is_none() {
            return;
        }
        if let Err(e) = self.set_wallpaper_pixels(context, 1, 1, &[0, 0, 0, 255]) {
            tracing::warn!("black fallback upload failed during video reset: {e}");
        }
    }
}
