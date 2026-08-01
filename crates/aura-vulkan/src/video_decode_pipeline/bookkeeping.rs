use super::types::{DpbSlotState, VideoDecodePipeline};
use crate::video_session::VulkanVideoSession;
use ash::vk;

impl VideoDecodePipeline {
    /// Reset decode session reference frame state upon loop or seek boundary.
    ///
    /// The next `submit_decode` records a `vkCmdControlVideoCodingKHR` reset
    /// and clears tracked DPB slot state. Tracked layouts and in-flight
    /// busy slots are intentionally untouched: the renderer may still be
    /// sampling a presented frame, and its ack (which marks the slot
    /// sampled) must not be lost.
    pub fn reset_session_state(&mut self, _session: &VulkanVideoSession) {
        self.session_reset_required = true;
        self.slot_state.clear();
        tracing::debug!("VideoDecodePipeline: session reset armed (control reset on next decode)");
    }

    /// Record that the graphics queue sampled the frame in `slot_index`
    /// (transitioned it to `SHADER_READ_ONLY_OPTIMAL` and signaled
    /// `gfx_timeline` with `gfx_value`). The next decode that uses the slot
    /// transitions it back to `VIDEO_DECODE_DPB_KHR` after waiting for
    /// `gfx_value` on the graphics timeline.
    pub fn mark_slot_sampled(&mut self, slot_index: u32, gfx_value: u64) {
        if let Some(layout) = self.slot_layouts.get_mut(slot_index as usize) {
            *layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        }
        if let Some(value) = self.gfx_sampled.get_mut(slot_index as usize) {
            *value = gfx_value;
        }
    }

    /// Forget all per-slot layout tracking. Called after session recreation:
    /// the new DPB images are fresh, so round-trip barriers start from
    /// `UNDEFINED`.
    pub fn reset_slot_layouts(&mut self) {
        for layout in &mut self.slot_layouts {
            *layout = vk::ImageLayout::UNDEFINED;
        }
    }

    /// Current DPB slot bookkeeping (for reference slot mapping).
    pub fn slot_state(&self) -> &[Option<DpbSlotState>] {
        &self.slot_state
    }
}
