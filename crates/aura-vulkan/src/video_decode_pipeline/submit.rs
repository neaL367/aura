use super::types::{DecodeFrameInput, DecodedVideoFrame, DpbSlotState, VideoDecodePipeline};
use crate::{context::VulkanContext, error::VulkanError, video_session::VulkanVideoSession};
use ash::vk;

impl VideoDecodePipeline {
    /// Record and submit the decode of a single H.264 picture.
    ///
    /// Non-blocking (M2b pipelining): the video queue executes decodes
    /// asynchronously, gated only by the staging ring — the submit for
    /// frame N+1 waits for the decode that last used its staging slot, so
    /// the bitstream is never overwritten while the decoder reads it.
    /// Completion is tracked via `timeline_value`, which the graphics queue
    /// waits on before sampling each frame.
    pub fn submit_decode(
        &mut self,
        context: &VulkanContext,
        session: &VulkanVideoSession,
        input: &DecodeFrameInput,
    ) -> Result<DecodedVideoFrame, VulkanError> {
        if input.bitstream.is_empty() || input.slice_offsets.is_empty() {
            return Err(VulkanError::Video(
                "submit_decode: empty bitstream or slice list".into(),
            ));
        }

        let Some(queue_family) = context.video_queue_family else {
            return Err(VulkanError::MissingExtension("video decode queue family"));
        };

        let _queue_guard = context.video_queue_lock();

        if self.slot_state.len() != session.dpb_slots.len() {
            self.slot_state = vec![None; session.dpb_slots.len()];
        }
        if self.slot_layouts.len() != session.dpb_slots.len() {
            self.slot_layouts = vec![vk::ImageLayout::UNDEFINED; session.dpb_slots.len()];
        }
        if self.gfx_sampled.len() != session.dpb_slots.len() {
            self.gfx_sampled = vec![0; session.dpb_slots.len()];
        }

        let setup_slot = session
            .dpb_slots
            .get(input.setup_slot_index as usize)
            .ok_or_else(|| {
                VulkanError::Video(format!(
                    "submit_decode: setup slot {} out of range ({} slots)",
                    input.setup_slot_index,
                    session.dpb_slots.len()
                ))
            })?;

        // -- Wait for graphics sampling of the setup slot ---------------------
        // The decode overwrites the setup slot; it must not start until the
        // renderer finished sampling the slot's previous contents. The
        // renderer signaled `gfx_timeline` after sampling (the ack carries
        // the value), so the wait returns immediately in the common case.
        let sampled_value = self.gfx_sampled[input.setup_slot_index as usize];
        if sampled_value > 0 {
            let wait_info = vk::SemaphoreWaitInfo::default()
                .semaphores(std::slice::from_ref(&self.gfx_timeline))
                .values(std::slice::from_ref(&sampled_value));
            unsafe {
                context
                    .device
                    .wait_semaphores(&wait_info, u64::MAX)
                    .map_err(|e| VulkanError::Video(format!("gfx sampling wait: {e:?}")))?;
            }
        }

        // -- Ring staging slot ------------------------------------------------
        // Round-robin over the ring; before reuse, wait for the decode that
        // last used the slot. The wait is short (at most one decode in
        // flight ahead) and the video queue never depends on the graphics
        // queue, so it cannot deadlock.
        let ring_index = self.ring_head;
        self.ring_head = (self.ring_head + 1) % self.staging_slots.len();
        let slot_last_value = self.staging_slots[ring_index].last_value;
        if slot_last_value > 0 {
            let wait_info = vk::SemaphoreWaitInfo::default()
                .semaphores(std::slice::from_ref(&self.timeline_semaphore))
                .values(std::slice::from_ref(&slot_last_value));
            unsafe {
                context
                    .device
                    .wait_semaphores(&wait_info, u64::MAX)
                    .map_err(|e| VulkanError::Video(format!("staging slot wait: {e:?}")))?;
            }
        }

        self.ensure_staging_slot(context, ring_index, input.bitstream.len() as u64)?;
        let decode_command_buffer = {
            let slot = &mut self.staging_slots[ring_index];
            let allocation = slot
                .allocation
                .as_mut()
                .ok_or_else(|| VulkanError::Video("bitstream allocation missing".into()))?;
            let Some(mapped) = allocation.mapped_slice_mut() else {
                return Err(VulkanError::Video(
                    "bitstream buffer is not host-visible".into(),
                ));
            };
            mapped[..input.bitstream.len()].copy_from_slice(input.bitstream);
            slot.command_buffer
        };

        let queue = unsafe { context.device.get_device_queue(queue_family, 0) };

        unsafe {
            context
                .device
                .reset_command_buffer(decode_command_buffer, vk::CommandBufferResetFlags::empty())
                .map_err(|e| VulkanError::Video(format!("cb reset: {e:?}")))?;

            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            context
                .device
                .begin_command_buffer(decode_command_buffer, &begin_info)
                .map_err(|e| VulkanError::Video(format!("cb begin: {e:?}")))?;
        }

        let src_buffer = self.staging_slots[ring_index].buffer;

        unsafe {
            self.record_decode_commands(
                context,
                session,
                input,
                decode_command_buffer,
                src_buffer,
            )?;
        }

        // -- Submit (no completion wait: M2b pipelining) --------------------
        let next_value = self.timeline_value + 1;
        let mut timeline_info = vk::TimelineSemaphoreSubmitInfo::default()
            .signal_semaphore_values(std::slice::from_ref(&next_value));
        let submit_info = vk::SubmitInfo::default()
            .command_buffers(std::slice::from_ref(&decode_command_buffer))
            .signal_semaphores(std::slice::from_ref(&self.timeline_semaphore))
            .push_next(&mut timeline_info);

        unsafe {
            context
                .device
                .queue_submit(queue, &[submit_info], vk::Fence::null())
                .map_err(|e| VulkanError::Video(format!("queue submit: {e:?}")))?;
        }

        // -- Update bookkeeping ---------------------------------------------------
        self.staging_slots[ring_index].last_value = next_value;
        self.slot_state[input.setup_slot_index as usize] = if input.is_reference {
            Some(DpbSlotState {
                frame_num: input.frame_num,
                pic_order_cnt: input.pic_order_cnt,
            })
        } else {
            None
        };
        self.timeline_value = next_value;

        Ok(DecodedVideoFrame {
            image: setup_slot.image,
            image_view: setup_slot.view,
            timeline_value: next_value,
            slot_index: input.setup_slot_index,
        })
    }
}
