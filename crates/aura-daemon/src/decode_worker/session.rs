use aura_core::playback::PlaybackCommand;
use aura_media::h264_parser::{
    H264Pps, H264Sps, PocTracker, parse_pps, parse_sps, remove_emulation_prevention,
    split_annex_b_nal_units,
};
use aura_vulkan::{
    video_decode_pipeline::{GpuAck, GpuVideoMessage, VideoDecodePipeline, VideoGpuFrame},
    video_session::VulkanVideoSession,
};
use crossbeam_channel::Receiver;

pub(super) struct VulkanVideoDecoder {
    pub(super) demuxer: aura_win::MfH264Demuxer,
    pub(super) session: VulkanVideoSession,
    pub(super) pipeline: VideoDecodePipeline,
    pub(super) sps: H264Sps,
    pub(super) pps: H264Pps,
    pub(super) poc_tracker: PocTracker,
    /// Renderer slot-reuse acks (slot indices no longer displayed).
    pub(super) ack_rx: Receiver<GpuAck>,
    /// Channel to the render thread for video frames and session-reset
    /// coordination.
    pub(super) gpu_frame_tx: crossbeam_channel::Sender<GpuVideoMessage>,
    /// DPB slots currently displayed by the renderer (not reusable).
    pub(super) busy_slots: Vec<u32>,
    /// First access unit (the one carrying SPS/PPS), pending processing.
    pub(super) pending_first_au: Option<(Vec<u8>, u64)>,
    pub(super) last_pts_ms: u64,
}

pub(super) enum ProcessOutcome {
    /// A decodable picture was produced and staged on the GPU.
    Frame(VideoGpuFrame),
    /// Access unit has no decodable slices (AUD/SEI/SPS/PPS-only, etc.).
    Skip,
    /// A Stop command arrived while waiting for a renderer ack.
    Stopped,
}

impl VulkanVideoDecoder {
    pub(super) fn setup(
        path: &std::path::Path,
        context: &std::sync::Arc<aura_vulkan::VulkanContext>,
        gpu_frame_tx: &crossbeam_channel::Sender<GpuVideoMessage>,
        ack_rx: Receiver<GpuAck>,
    ) -> Result<Self, String> {
        let mut demuxer = aura_win::MfH264Demuxer::open(path).map_err(|e| e.to_string())?;

        // Scan the first AUs until an SPS+PPS pair is found.
        let mut sps = None;
        let mut pps = None;
        let mut pending = None;
        for _ in 0..32 {
            let (au, pts_ms) = match demuxer.read_next_annex_b_nal().map_err(|e| e.to_string())? {
                Some(x) => x,
                None => break,
            };
            if pending.is_none() {
                pending = Some((au.clone(), pts_ms));
            }
            for nal in split_annex_b_nal_units(&au) {
                if nal.is_empty() {
                    continue;
                }
                match nal[0] & 0x1F {
                    7 if sps.is_none() => {
                        if let Ok(s) = parse_sps(&remove_emulation_prevention(&nal[1..])) {
                            sps = Some(s);
                        }
                    }
                    8 if pps.is_none() => {
                        if let Ok(p) = parse_pps(&remove_emulation_prevention(&nal[1..])) {
                            pps = Some(p);
                        }
                    }
                    _ => {}
                }
            }
            if sps.is_some() && pps.is_some() {
                break;
            }
        }

        let (sps, pps) = match (sps, pps) {
            (Some(s), Some(p)) => (s, p),
            _ => return Err("no SPS/PPS found in stream".into()),
        };

        let coded_w = sps.coded_width();
        let coded_h = sps.coded_height();
        let session = VulkanVideoSession::create(context, &sps, &pps, coded_w, coded_h)
            .map_err(|e| e.to_string())?;
        let queue_family = context.video_queue_family.ok_or("no video queue family")?;
        let pipeline =
            VideoDecodePipeline::create(context, queue_family).map_err(|e| e.to_string())?;

        tracing::info!(
            "Vulkan Video worker ready for {}: {}x{} (coded {}x{}), DPB slots: {}",
            path.display(),
            sps.cropped_width(),
            sps.cropped_height(),
            coded_w,
            coded_h,
            session.dpb_slots.len()
        );

        Ok(Self {
            demuxer,
            session,
            pipeline,
            sps,
            pps,
            poc_tracker: PocTracker::new(),
            ack_rx,
            gpu_frame_tx: gpu_frame_tx.clone(),
            busy_slots: Vec::new(),
            pending_first_au: pending,
            last_pts_ms: 0,
        })
    }

    /// Handle a renderer slot-reuse ack: the slot is no longer displayed and
    /// was sampled in `SHADER_READ_ONLY_OPTIMAL` (its `gfx_value` recorded
    /// on the graphics timeline), so it becomes reusable and its layout
    pub(super) fn renegotiate(
        &mut self,
        context: &std::sync::Arc<aura_vulkan::VulkanContext>,
        sps: &H264Sps,
        pps: &H264Pps,
        cmd_rx: &Receiver<PlaybackCommand>,
    ) -> Result<(), String> {
        if *sps == self.sps && *pps == self.pps {
            return Ok(());
        }

        let fresh_ids = sps.seq_parameter_set_id != self.sps.seq_parameter_set_id
            || pps.pic_parameter_set_id != self.pps.pic_parameter_set_id;
        let compatible = sps.coded_width() == self.session.width
            && sps.coded_height() == self.session.height
            && sps.max_num_ref_frames as u32 == self.session.max_ref_frames
            && sps.profile_idc == self.sps.profile_idc;

        if fresh_ids && compatible {
            let sps_changed = *sps != self.sps;
            let pps_changed = *pps != self.pps;
            self.session
                .update_parameters(
                    context,
                    sps_changed.then_some(sps),
                    pps_changed.then_some(pps),
                )
                .map_err(|e| e.to_string())?;
        } else {
            self.recreate_session(context, sps, pps, cmd_rx)?;
        }

        self.sps = sps.clone();
        self.pps = pps.clone();
        tracing::info!(
            "video session parameters renegotiated (SPS {} PPS {}; {}x{} cropped)",
            sps.seq_parameter_set_id,
            pps.pic_parameter_set_id,
            sps.cropped_width(),
            sps.cropped_height()
        );
        Ok(())
    }

    /// Ask the renderer to drop its active video frame (so no frame
    /// references images about to be destroyed) and wait for the ack.
    pub(super) fn recreate_session(
        &mut self,
        context: &std::sync::Arc<aura_vulkan::VulkanContext>,
        sps: &H264Sps,
        pps: &H264Pps,
        cmd_rx: &Receiver<PlaybackCommand>,
    ) -> Result<(), String> {
        tracing::info!(
            "recreating Vulkan Video session for SPS/PPS change ({}x{} coded, SPS {} PPS {})",
            sps.coded_width(),
            sps.coded_height(),
            sps.seq_parameter_set_id,
            pps.pic_parameter_set_id
        );

        if !self.request_session_reset(cmd_rx)? {
            return Err("stopped while resetting session".into());
        }

        unsafe {
            context.device.device_wait_idle().ok();
            self.session.destroy(context);
        }
        self.busy_slots.clear();

        let coded_w = sps.coded_width();
        let coded_h = sps.coded_height();
        self.session = VulkanVideoSession::create(context, sps, pps, coded_w, coded_h)
            .map_err(|e| e.to_string())?;
        self.pipeline.reset_session_state(&self.session);
        self.pipeline.reset_slot_layouts();
        self.poc_tracker = PocTracker::new();
        self.last_pts_ms = 0;
        Ok(())
    }
}
