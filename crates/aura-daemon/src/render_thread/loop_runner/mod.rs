use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use aura_core::playback::PerformanceProfile;
use aura_media::FrameReceiver;
use aura_vulkan::{
    VulkanContext, VulkanError,
    monitor_renderer::MonitorRenderer,
    video_decode_pipeline::{GpuAck, GpuVideoMessage},
};

use super::RenderCommand;
use crate::decode_worker::DecodeWorkerHandle;

mod commands;
mod upload;

pub use upload::load_and_upload_static_image;

pub struct RenderLoopParams {
    pub renderer: MonitorRenderer,
    pub context: Arc<VulkanContext>,
    pub initial_worker: Option<DecodeWorkerHandle>,
    pub initial_frame_rx: Option<FrameReceiver>,
    /// Vulkan Video GPU frame channel (direct DPB sampling).
    pub gpu_frame_rx: Option<crossbeam_channel::Receiver<GpuVideoMessage>>,
    /// Slot-reuse acks sent back to the current HW video worker.
    pub gpu_ack_tx: Option<crossbeam_channel::Sender<GpuAck>>,
    pub assign_rx: crossbeam_channel::Receiver<RenderCommand>,
    pub shutdown_flag: Arc<AtomicBool>,
    pub pause_flag: Arc<AtomicBool>,
    pub counter: Arc<AtomicU64>,
    pub width: u32,
    pub height: u32,
}

pub(super) struct RenderLoopState {
    pub(super) renderer: MonitorRenderer,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) active_worker: Option<DecodeWorkerHandle>,
    pub(super) current_frame_rx: Option<FrameReceiver>,
    pub(super) gpu_frame_rx: Option<crossbeam_channel::Receiver<GpuVideoMessage>>,
    pub(super) gpu_ack_tx: Option<crossbeam_channel::Sender<GpuAck>>,
    pub(super) is_dirty: bool,
    pub(super) current_profile: PerformanceProfile,
    pub(super) target_fps: u8,
}

pub fn run_render_loop(params: RenderLoopParams) {
    let RenderLoopParams {
        renderer,
        context,
        initial_worker,
        initial_frame_rx,
        gpu_frame_rx,
        gpu_ack_tx,
        assign_rx,
        shutdown_flag,
        pause_flag,
        counter,
        width,
        height,
    } = params;

    let mut state = RenderLoopState {
        renderer,
        width,
        height,
        active_worker: initial_worker,
        current_frame_rx: initial_frame_rx,
        gpu_frame_rx,
        gpu_ack_tx,
        is_dirty: true,
        current_profile: PerformanceProfile::Maximum,
        target_fps: 60,
    };

    loop {
        if shutdown_flag.load(Ordering::Relaxed) {
            break;
        }

        state.drain_commands(&assign_rx, &counter, &context);

        if pause_flag.load(Ordering::Relaxed) || state.current_profile == PerformanceProfile::Paused
        {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        let has_animated_source = state.current_frame_rx.is_some() || state.gpu_frame_rx.is_some();

        let mut has_new_frame = false;
        if let Some(ref rx) = state.current_frame_rx
            && let Some(frame) = rx.try_recv()
        {
            has_new_frame = true;
            // A texture-frame source replaced a GPU video frame: release its
            // DPB slot so the old worker can wind down.
            state.ack_active_video_slot();
            if let Err(e) = state.renderer.set_wallpaper_pixels(
                &context,
                frame.width,
                frame.height,
                &frame.data,
            ) {
                tracing::warn!("Texture upload failed: {}", e);
            }
        }
        if let Some(ref rx) = state.gpu_frame_rx
            && let Ok(message) = rx.try_recv()
        {
            match message {
                GpuVideoMessage::Frame(frame) => {
                    has_new_frame = true;
                    // The previous frame's slot is now free for the worker to reuse.
                    state.ack_active_video_slot();
                    state.renderer.set_video_frame(&context, &frame);
                }
                GpuVideoMessage::SessionReset => {
                    // The decode worker is recreating its session and will
                    // destroy the DPB images: drop the active view now and
                    // confirm, so no stale image is sampled.
                    state.renderer.clear_video_frame(&context);
                    if let Some(tx) = state.gpu_ack_tx.as_ref() {
                        let _ = tx.send(GpuAck::SessionReset);
                    }
                }
            }
        }

        if has_animated_source {
            // Animated content (GIF/Video): draw whenever new frame or dirty
            if has_new_frame || state.is_dirty {
                match state.renderer.frame(&context, [0.0, 0.0, 0.0, 1.0]) {
                    Ok(_) => {
                        counter.fetch_add(1, Ordering::Relaxed);
                        state.is_dirty = false;
                    }
                    Err(VulkanError::SwapchainOutOfDate) => {
                        if let Err(e) = state.renderer.resize(&context, state.width, state.height) {
                            tracing::warn!("Swapchain resize failed: {}", e);
                        } else if state.renderer.frame(&context, [0.0, 0.0, 0.0, 1.0]).is_ok() {
                            counter.fetch_add(1, Ordering::Relaxed);
                            state.is_dirty = false;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Render frame failed: {}", e);
                    }
                }
            }
            let sleep_ms = match state.current_profile {
                aura_core::playback::PerformanceProfile::Balanced => {
                    let balanced_fps = (state.target_fps / 2).max(15);
                    1000 / balanced_fps as u64
                }
                _ => 1000 / state.target_fps.max(1) as u64,
            };
            std::thread::sleep(Duration::from_millis(sleep_ms));
        } else {
            // Static image content: draw once when dirty, then sleep (0% CPU/GPU idle)
            if state.is_dirty {
                let mut rendered = false;
                match state.renderer.frame(&context, [0.0, 0.0, 0.0, 1.0]) {
                    Ok(_) => {
                        counter.fetch_add(1, Ordering::Relaxed);
                        rendered = true;
                    }
                    Err(VulkanError::SwapchainOutOfDate) => {
                        if let Err(e) = state.renderer.resize(&context, state.width, state.height) {
                            tracing::warn!("Swapchain resize failed: {}", e);
                        } else if state.renderer.frame(&context, [0.0, 0.0, 0.0, 1.0]).is_ok() {
                            counter.fetch_add(1, Ordering::Relaxed);
                            rendered = true;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Render frame failed: {}", e);
                    }
                }
                if rendered {
                    state.renderer.trim_staging(&context);
                    aura_win::trim_working_set();
                    state.is_dirty = false;
                }
            } else {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    if let Some(worker) = state.active_worker.take() {
        worker.stop();
    }
}
