use std::path::PathBuf;

use aura_core::playback::PlaybackCommand;
use aura_media::FrameSender;
use aura_media::h264_parser::PocTracker;
use aura_vulkan::video_decode_pipeline::{GpuVideoMessage, VideoGpuFrame};
use crossbeam_channel::{Receiver, TrySendError};

use super::session::{ProcessOutcome, VulkanVideoDecoder};
use super::{ControlFlow, handle_command, run_cpu_video_loop};

fn paced_sleep(duration_ms: u64, cmd_rx: &Receiver<PlaybackCommand>) -> bool {
    const CHUNK: std::time::Duration = std::time::Duration::from_millis(25);
    let mut remaining = std::time::Duration::from_millis(duration_ms.max(1));
    while remaining > std::time::Duration::ZERO {
        let step = remaining.min(CHUNK);
        if let Ok(cmd) = cmd_rx.recv_timeout(step) {
            if handle_command(cmd, cmd_rx) == ControlFlow::Stopped {
                return false;
            }
            break; // command handled (e.g. resumed from a pause); move on
        }
        remaining = remaining.saturating_sub(step);
    }
    true
}

fn send_gpu_frame(
    frame: VideoGpuFrame,
    gpu_frame_tx: &crossbeam_channel::Sender<GpuVideoMessage>,
    cmd_rx: &Receiver<PlaybackCommand>,
) -> bool {
    let duration_ms = frame.duration_ms;
    let mut message = GpuVideoMessage::Frame(frame);
    loop {
        match gpu_frame_tx.try_send(message) {
            Ok(()) => break,
            Err(TrySendError::Full(m)) => {
                message = m;
                if let Ok(cmd) = cmd_rx.try_recv()
                    && handle_command(cmd, cmd_rx) == ControlFlow::Stopped
                {
                    return false;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(TrySendError::Disconnected(_)) => return false,
        }
    }
    paced_sleep(duration_ms, cmd_rx)
}

pub(super) fn run_vulkan_video_loop(
    mut decoder: VulkanVideoDecoder,
    context: &std::sync::Arc<aura_vulkan::VulkanContext>,
    gpu_frame_tx: crossbeam_channel::Sender<GpuVideoMessage>,
    frame_sender: FrameSender,
    cmd_rx: Receiver<PlaybackCommand>,
    path: PathBuf,
) {
    tracing::info!("Vulkan Video worker started for {}", path.display());

    // The AU that carried SPS/PPS is decoded as the first frame.
    let mut next_au = decoder.pending_first_au.take();
    let mut consecutive_errors = 0u32;
    // Fatal (non-stop) failure reason: playback falls back to CPU decoding.
    let mut fatal: Option<String> = None;

    'outer: loop {
        if let Ok(cmd) = cmd_rx.try_recv()
            && handle_command(cmd, &cmd_rx) == ControlFlow::Stopped
        {
            break 'outer;
        }

        // Process any renderer acks so busy slots become reusable.
        while let Ok(ack) = decoder.ack_rx.try_recv() {
            decoder.process_ack(ack);
        }

        let au = match next_au.take() {
            Some(au) => Some(au),
            None => match decoder.demuxer.read_next_annex_b_nal() {
                Ok(au) => au,
                Err(e) => {
                    tracing::error!("Demuxer error: {}", e);
                    fatal = Some(e.to_string());
                    break 'outer;
                }
            },
        };

        let Some((annex_b, pts_ms)) = au else {
            // End of stream: loop the wallpaper. Busy slots stay tracked —
            // the renderer acks the last displayed frame normally; the ack
            // also flips its layout so the round-trip barrier is recorded.
            if let Err(e) = decoder.demuxer.loop_reset() {
                tracing::error!("Failed to reset video loop: {}", e);
                fatal = Some(e.to_string());
                break 'outer;
            }
            decoder.poc_tracker = PocTracker::new();
            decoder.pipeline.reset_session_state(&decoder.session);
            decoder.last_pts_ms = 0;
            continue;
        };

        match decoder.process_au(context, &annex_b, pts_ms, &cmd_rx) {
            Ok(ProcessOutcome::Frame(gpu_frame)) => {
                consecutive_errors = 0;
                if !send_gpu_frame(gpu_frame, &gpu_frame_tx, &cmd_rx) {
                    break 'outer;
                }
            }
            Ok(ProcessOutcome::Skip) => {}
            Ok(ProcessOutcome::Stopped) => break 'outer,
            Err(e) => {
                consecutive_errors += 1;
                tracing::warn!("Vulkan decode error ({}): {}", consecutive_errors, e);
                if consecutive_errors >= 5 {
                    fatal = Some(format!(
                        "{consecutive_errors} consecutive decode errors: {e}"
                    ));
                    break 'outer;
                }
            }
        }
    }

    let fall_back_to_cpu = match fatal {
        Some(reason) => {
            tracing::warn!(
                "Vulkan Video decode failed ({reason}); falling back to Media Foundation CPU decode"
            );
            // Coordinate with the renderer first: it must drop its active
            // DPB view (and any queued frames must be presented/cleared)
            // before the session's images are destroyed. A Stop that
            // arrives during the handshake skips the fallback.
            match decoder.request_session_reset(&cmd_rx) {
                Ok(true) => true,
                Ok(false) => {
                    tracing::info!("Vulkan Video worker stopping; skipping CPU fallback");
                    false
                }
                Err(e) => {
                    tracing::warn!("Session reset handshake failed ({e}); skipping CPU fallback");
                    false
                }
            }
        }
        None => false,
    };

    // RAII teardown: GPU resources must be destroyed explicitly with the
    // context; wait for any in-flight graphics sampling first.
    unsafe {
        context.device.device_wait_idle().ok();
        decoder.pipeline.destroy(context);
        decoder.session.destroy(context);
    }

    if fall_back_to_cpu {
        match aura_win::MfVideoDecoder::open(&path) {
            Ok(cpu_decoder) => {
                tracing::info!("CPU decode fallback started for {}", path.display());
                run_cpu_video_loop(cpu_decoder, frame_sender, cmd_rx, path.clone());
            }
            Err(e) => {
                tracing::error!(
                    "CPU fallback failed to open video wallpaper {}: {}",
                    path.display(),
                    e
                );
            }
        }
    }

    tracing::info!("Vulkan Video worker finished for {}", path.display());
}
