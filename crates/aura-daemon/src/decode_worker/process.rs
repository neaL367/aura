use aura_core::playback::PlaybackCommand;
use aura_media::h264_parser::{
    H264Pps, H264Sps, parse_pps, parse_slice_header, parse_sps, remove_emulation_prevention,
    split_annex_b_nal_units,
};
use aura_vulkan::video_decode_pipeline::{DecodeFrameInput, ReferencePicture, VideoGpuFrame};
use crossbeam_channel::Receiver;

use super::session::{ProcessOutcome, VulkanVideoDecoder};

impl VulkanVideoDecoder {
    pub(super) fn process_au(
        &mut self,
        context: &std::sync::Arc<aura_vulkan::VulkanContext>,
        annex_b: &[u8],
        pts_ms: u64,
        cmd_rx: &Receiver<PlaybackCommand>,
    ) -> Result<ProcessOutcome, String> {
        let nals = split_annex_b_nal_units(annex_b);

        let mut slices = Vec::with_capacity(annex_b.len());
        let mut slice_offsets: Vec<u32> = Vec::new();
        let mut first_slice = None;
        let mut is_idr = false;
        let mut is_reference = false;
        // Stashed SPS/PPS that differ from the active session pair; they are
        // committed (via parameter update or session recreation) once a
        // slice actually references them.
        let mut new_sps: Option<H264Sps> = None;
        let mut new_pps: Option<H264Pps> = None;

        for nal in &nals {
            if nal.len() < 2 {
                continue;
            }
            let nal_type = nal[0] & 0x1F;
            let ref_idc = (nal[0] >> 5) & 0x3;
            match nal_type {
                7 => {
                    if let Ok(sps) = parse_sps(&remove_emulation_prevention(&nal[1..]))
                        && sps != self.sps
                    {
                        new_sps = Some(sps);
                    }
                }
                8 => {
                    if let Ok(pps) = parse_pps(&remove_emulation_prevention(&nal[1..]))
                        && pps != self.pps
                    {
                        new_pps = Some(pps);
                    }
                }
                9 | 6 | 12 => {}
                1 | 5 => {
                    slice_offsets.push(slices.len() as u32);
                    slices.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
                    slices.extend_from_slice(nal);
                    let payload = remove_emulation_prevention(&nal[1..]);
                    match parse_slice_header(&payload, &self.sps, nal_type == 5) {
                        Ok(slice) => {
                            if first_slice.is_none() {
                                first_slice = Some(slice);
                            }
                            is_idr |= nal_type == 5;
                            is_reference |= ref_idc != 0;
                        }
                        Err(e) => tracing::debug!("slice header parse failed: {e}"),
                    }
                }
                _ => {}
            }
        }

        let Some(slice) = first_slice else {
            return Ok(ProcessOutcome::Skip);
        };

        // -- SPS/PPS renegotiation ----------------------------------------
        // If this slice references a stashed (changed) parameter set, commit
        // it before decoding. A changed PPS always wins over the active one;
        // the SPS paired with it is committed too. Unreferenced stashes are
        // dropped.
        let reneg_sps: Option<&H264Sps> = match new_pps.as_ref() {
            Some(pps) if slice.pps_id == pps.pic_parameter_set_id => new_sps
                .as_ref()
                .filter(|s| s.seq_parameter_set_id == pps.seq_parameter_set_id),
            Some(_) if slice.pps_id == self.pps.pic_parameter_set_id => new_sps
                .as_ref()
                .filter(|s| s.seq_parameter_set_id == self.pps.seq_parameter_set_id),
            Some(_) | None => {
                if slice.pps_id != self.pps.pic_parameter_set_id {
                    tracing::debug!(
                        "slice PPS {} does not match session PPS {}; skipping",
                        slice.pps_id,
                        self.pps.pic_parameter_set_id
                    );
                    return Ok(ProcessOutcome::Skip);
                }
                new_sps
                    .as_ref()
                    .filter(|s| s.seq_parameter_set_id == self.pps.seq_parameter_set_id)
            }
        };
        let reneg_pps: Option<&H264Pps> = new_pps
            .as_ref()
            .filter(|p| p.pic_parameter_set_id == slice.pps_id);

        if reneg_sps.is_some() || reneg_pps.is_some() {
            let sps = reneg_sps.cloned().unwrap_or_else(|| self.sps.clone());
            let pps = reneg_pps.cloned().unwrap_or_else(|| self.pps.clone());
            self.renegotiate(context, &sps, &pps, cmd_rx)?;
        }

        let poc = self
            .poc_tracker
            .compute(&self.sps, &slice, is_idr, is_reference);

        let references: Vec<ReferencePicture> = self
            .pipeline
            .slot_state()
            .iter()
            .enumerate()
            .filter_map(|(i, state)| {
                state.map(|st| ReferencePicture {
                    slot_index: i as u32,
                    frame_num: st.frame_num,
                    pic_order_cnt: st.pic_order_cnt,
                })
            })
            .collect();

        // Pick a setup slot: prefer an empty slot, then a non-reference slot.
        // Slots currently displayed by the renderer are off-limits until the
        // renderer acks them (the ack also flips their layout tracking back
        // to `SHADER_READ_ONLY_OPTIMAL` for the round-trip barrier).
        let dpb_len = self.session.dpb_slots.len();
        let setup_slot_index = loop {
            let free = (0..dpb_len).find(|i| {
                self.pipeline.slot_state()[*i].is_none() && !self.busy_slots.contains(&(*i as u32))
            });
            if let Some(i) = free {
                break i;
            }
            let unreferenced = (0..dpb_len).find(|i| {
                !references.iter().any(|r| r.slot_index == *i as u32)
                    && !self.busy_slots.contains(&(*i as u32))
            });
            if let Some(i) = unreferenced {
                break i;
            }
            // Every usable slot is displayed by the renderer: wait for an ack.
            if !self.wait_for_ack(cmd_rx)? {
                return Ok(ProcessOutcome::Stopped);
            }
        };

        let input = DecodeFrameInput {
            bitstream: &slices,
            slice_offsets: &slice_offsets,
            seq_parameter_set_id: self.sps.seq_parameter_set_id,
            pic_parameter_set_id: self.pps.pic_parameter_set_id,
            frame_num: slice.frame_num as u16,
            idr_pic_id: slice.idr_pic_id as u16,
            pic_order_cnt: [poc, 0],
            is_idr,
            is_reference,
            setup_slot_index: setup_slot_index as u32,
            references: &references,
        };

        let decoded = self
            .pipeline
            .submit_decode(context, &self.session, &input)
            .map_err(|e| e.to_string())?;

        // The renderer now owns this slot until it acks; it must not be
        // reused for decoding in the meantime.
        self.busy_slots.push(decoded.slot_index);

        let delta_ms = if pts_ms >= self.last_pts_ms {
            pts_ms - self.last_pts_ms
        } else {
            33
        };
        self.last_pts_ms = pts_ms;
        let duration_ms = delta_ms.max(10);

        Ok(ProcessOutcome::Frame(VideoGpuFrame {
            image: decoded.image,
            image_view: decoded.image_view,
            timeline_semaphore: self.pipeline.timeline_semaphore,
            timeline_value: decoded.timeline_value,
            gfx_timeline: self.pipeline.gfx_timeline,
            slot_index: decoded.slot_index,
            width: self.session.width,
            height: self.session.height,
            pts_ms,
            duration_ms,
        }))
    }
}
