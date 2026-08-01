use aura_core::playback::PlaybackCommand;
use aura_vulkan::video_decode_pipeline::{GpuAck, GpuVideoMessage};
use crossbeam_channel::{Receiver, TrySendError};

use super::session::VulkanVideoDecoder;
use super::{ControlFlow, handle_command};

impl VulkanVideoDecoder {
    pub(super) fn process_ack(&mut self, ack: GpuAck) {
        match ack {
            GpuAck::SlotReused { slot, gfx_value } => {
                self.busy_slots.retain(|s| *s != slot);
                self.pipeline.mark_slot_sampled(slot, gfx_value);
            }
            // Session-reset acks are only expected while waiting for them;
            // ignore stray ones in the drain path.
            GpuAck::SessionReset => {}
        }
    }

    pub(super) fn wait_for_ack(
        &mut self,
        cmd_rx: &Receiver<PlaybackCommand>,
    ) -> Result<bool, String> {
        loop {
            if let Ok(cmd) = cmd_rx.try_recv()
                && handle_command(cmd, cmd_rx) == ControlFlow::Stopped
            {
                return Ok(false);
            }
            match self
                .ack_rx
                .recv_timeout(std::time::Duration::from_millis(50))
            {
                Ok(ack) => {
                    if matches!(ack, GpuAck::SlotReused { .. }) {
                        self.process_ack(ack);
                        return Ok(true);
                    }
                    // Stray SessionReset ack while waiting for a slot reuse.
                    continue;
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    return Err("ack channel closed (renderer gone)".into());
                }
            }
        }
    }

    /// Commit new SPS/PPS. No-op when unchanged; otherwise use a parameter
    /// update (add-only, VK_KHR_video_queue semantics) when the pair uses
    /// fresh ids with an otherwise compatible session, or recreate the
    /// session when the profile, coded extent, or DPB capacity changes or
    pub(super) fn request_session_reset(
        &mut self,
        cmd_rx: &Receiver<PlaybackCommand>,
    ) -> Result<bool, String> {
        // Interruptible send: the bounded channel can hold frames while the
        // renderer is paused. Queued frames are still valid until the reset
        // ack (the old session is only destroyed afterwards).
        let mut message = GpuVideoMessage::SessionReset;
        loop {
            match self.gpu_frame_tx.try_send(message) {
                Ok(()) => break,
                Err(TrySendError::Full(m)) => {
                    message = m;
                    if let Ok(cmd) = cmd_rx.try_recv()
                        && handle_command(cmd, cmd_rx) == ControlFlow::Stopped
                    {
                        return Ok(false);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err("gpu channel closed during session reset".into());
                }
            }
        }
        self.wait_for_session_reset_ack(cmd_rx)
    }

    /// Recreate the video session + DPB array for an incompatible SPS/PPS
    /// change. The renderer is told to drop its active DPB image view first
    /// (via a `SessionReset` message + ack) because the old images are
    /// destroyed; decode pipeline state, POC tracking, and layout tracking
    pub(super) fn wait_for_session_reset_ack(
        &mut self,
        cmd_rx: &Receiver<PlaybackCommand>,
    ) -> Result<bool, String> {
        loop {
            if let Ok(cmd) = cmd_rx.try_recv()
                && handle_command(cmd, cmd_rx) == ControlFlow::Stopped
            {
                return Ok(false);
            }
            match self
                .ack_rx
                .recv_timeout(std::time::Duration::from_millis(50))
            {
                Ok(GpuAck::SessionReset) => return Ok(true),
                Ok(ack) => self.process_ack(ack),
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    return Err("ack channel closed (renderer gone)".into());
                }
            }
        }
    }
}
