use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use aura_core::playback::PlaybackCommand;
use aura_core::wallpaper::MediaKind;
use aura_core::wallpaper::detect_media_kind;
use aura_media::{FrameReceiver, ImageDecoder, MediaDecoder, frame_channel};
use aura_renderer_vulkan::{VulkanContext, VulkanError, monitor_renderer::MonitorRenderer};

use super::RenderCommand;
use crate::decode_worker::DecodeWorkerHandle;

pub struct RenderLoopParams {
    pub renderer: MonitorRenderer,
    pub context: Arc<VulkanContext>,
    pub initial_worker: Option<DecodeWorkerHandle>,
    pub initial_frame_rx: Option<FrameReceiver>,
    pub assign_rx: crossbeam_channel::Receiver<RenderCommand>,
    pub shutdown_flag: Arc<AtomicBool>,
    pub pause_flag: Arc<AtomicBool>,
    pub counter: Arc<AtomicU64>,
    pub width: u32,
    pub height: u32,
}

pub fn run_render_loop(params: RenderLoopParams) {
    let RenderLoopParams {
        mut renderer,
        context,
        initial_worker,
        initial_frame_rx,
        assign_rx,
        shutdown_flag,
        pause_flag,
        counter,
        mut width,
        mut height,
    } = params;

    let mut active_worker: Option<DecodeWorkerHandle> = initial_worker;
    let mut current_frame_rx = initial_frame_rx;
    let mut is_dirty = true;
    let mut current_profile = aura_core::playback::PerformanceProfile::Maximum;
    let mut target_fps: u8 = 60;

    loop {
        if shutdown_flag.load(Ordering::Relaxed) {
            break;
        }

        for _ in 0..32 {
            if let Ok(cmd) = assign_rx.try_recv() {
                is_dirty = true;
                match cmd {
                    RenderCommand::SetFitMode(new_mode) => {
                        tracing::info!("Render thread received new fit mode: {:?}", new_mode);
                        renderer.set_fit_mode(new_mode, &context);
                    }
                    RenderCommand::Resize {
                        width: new_w,
                        height: new_h,
                    } => {
                        tracing::info!("Render thread received resize: {}x{}", new_w, new_h);
                        width = new_w;
                        height = new_h;
                        if let Err(e) = renderer.resize(&context, width, height) {
                            tracing::warn!("Resize failed: {}", e);
                        }
                    }
                    RenderCommand::SetWallpaper {
                        path: new_path,
                        fit_mode,
                    } => {
                        tracing::info!("Render thread received new wallpaper path: {:?}", new_path);
                        if let Some(mode) = fit_mode {
                            renderer.set_fit_mode(mode, &context);
                        }
                        if let Some(worker) = active_worker.take() {
                            worker.stop();
                        }
                        current_frame_rx = None;

                        match detect_media_kind(&new_path) {
                            Some(MediaKind::Gif) => {
                                let (tx, rx) = frame_channel();
                                let handle = DecodeWorkerHandle::spawn_gif_worker(new_path, tx);
                                active_worker = Some(handle);
                                current_frame_rx = Some(rx);
                            }
                            Some(MediaKind::Video) => {
                                let (tx, rx) = frame_channel();
                                let handle = DecodeWorkerHandle::spawn_hw_video_worker(
                                    new_path,
                                    tx,
                                    context.clone(),
                                );
                                active_worker = Some(handle);
                                current_frame_rx = Some(rx);
                            }
                            Some(MediaKind::Image) => {
                                load_and_upload_static_image(&new_path, &mut renderer, &context);
                            }
                            _ => {
                                tracing::warn!(
                                    "Unsupported or unhandled media path: {:?}",
                                    new_path
                                );
                            }
                        }
                    }
                    RenderCommand::Playback(cmd) => {
                        if cmd == PlaybackCommand::Play
                            && current_profile == aura_core::playback::PerformanceProfile::Paused
                        {
                            tracing::info!(
                                "Playback(Play) received while paused; unpausing performance profile to Maximum"
                            );
                            current_profile = aura_core::playback::PerformanceProfile::Maximum;
                        }
                        if let Some(ref worker) = active_worker {
                            let _ = worker.command_sender.send(cmd);
                        }
                    }
                    RenderCommand::SetPerformanceProfile(profile) => {
                        tracing::info!(
                            profile = ?profile,
                            "Render thread performance profile updated"
                        );
                        current_profile = profile;
                    }
                    RenderCommand::SetTargetFps(fps) => {
                        let valid_fps = fps.clamp(1, 240);
                        tracing::info!(fps = valid_fps, "Render thread target FPS updated");
                        target_fps = valid_fps;
                    }
                }
            } else {
                break;
            }
        }

        if pause_flag.load(Ordering::Relaxed)
            || current_profile == aura_core::playback::PerformanceProfile::Paused
        {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        let mut has_new_frame = false;
        if let Some(ref rx) = current_frame_rx
            && let Some(frame) = rx.try_recv()
        {
            has_new_frame = true;
            if let Err(e) =
                renderer.set_wallpaper_pixels(&context, frame.width, frame.height, &frame.data)
            {
                tracing::warn!("Texture upload failed: {}", e);
            }
        }

        if current_frame_rx.is_some() {
            // Animated content (GIF/Video): draw whenever new frame or dirty
            if has_new_frame || is_dirty {
                match renderer.frame(&context, [0.0, 0.0, 0.0, 1.0]) {
                    Ok(_) => {
                        counter.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(VulkanError::SwapchainOutOfDate) => {
                        if let Err(e) = renderer.resize(&context, width, height) {
                            tracing::warn!("Swapchain resize failed: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Render frame failed: {}", e);
                    }
                }
                is_dirty = false;
            }
            let sleep_ms = match current_profile {
                aura_core::playback::PerformanceProfile::Balanced => {
                    let balanced_fps = (target_fps / 2).max(15);
                    1000 / balanced_fps as u64
                }
                _ => 1000 / target_fps.max(1) as u64,
            };
            std::thread::sleep(Duration::from_millis(sleep_ms));
        } else {
            // Static image content: draw once when dirty, then sleep (0% CPU/GPU idle)
            if is_dirty {
                match renderer.frame(&context, [0.0, 0.0, 0.0, 1.0]) {
                    Ok(_) => {
                        counter.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(VulkanError::SwapchainOutOfDate) => {
                        if let Err(e) = renderer.resize(&context, width, height) {
                            tracing::warn!("Swapchain resize failed: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Render frame failed: {}", e);
                    }
                }
                renderer.trim_staging(&context);
                aura_platform_windows::trim_working_set();
                is_dirty = false;
            } else {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    if let Some(worker) = active_worker.take() {
        worker.stop();
    }
}

pub fn load_and_upload_static_image(
    path: &Path,
    renderer: &mut MonitorRenderer,
    context: &VulkanContext,
) {
    match ImageDecoder::open(path) {
        Ok(mut decoder) => match decoder.next_frame() {
            Ok(Some(frame)) => {
                let w = frame.width;
                let h = frame.height;
                let res = renderer.set_wallpaper_pixels(context, w, h, &frame.data);
                drop(frame);
                drop(decoder);
                if let Err(e) = res {
                    tracing::warn!("Texture upload failed for {:?}: {}", path, e);
                } else {
                    tracing::info!("Texture upload succeeded for {:?} ({}x{})", path, w, h);
                    renderer.trim_staging(context);
                }
            }
            Ok(None) => tracing::warn!("ImageDecoder produced no frames for {:?}", path),
            Err(e) => tracing::warn!("ImageDecoder next_frame error for {:?}: {}", path, e),
        },
        Err(e) => tracing::warn!("Failed to open image {:?}: {}", path, e),
    }
}
