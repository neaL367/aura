use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use aura_core::playback::PlaybackCommand;
use aura_core::wallpaper::MediaKind;
use aura_core::wallpaper::detect_media_kind;
use aura_media::{ImageDecoder, MediaDecoder, frame_channel};
use aura_platform_windows::host_window::HostWindow;
use aura_renderer_vulkan::{VulkanContext, VulkanError, monitor_renderer::MonitorRenderer};

use crate::daemon::DaemonError;
use crate::decode_worker::DecodeWorkerHandle;
use crate::render_coordinator::MonitorContext;

#[derive(Debug, Clone)]
pub enum RenderCommand {
    SetWallpaper {
        path: PathBuf,
        fit_mode: Option<aura_core::wallpaper::FitMode>,
    },
    SetFitMode(aura_core::wallpaper::FitMode),
    Resize {
        width: u32,
        height: u32,
    },
    Playback(PlaybackCommand),
}

#[cfg(target_os = "windows")]
pub fn create_monitor_context(
    context: &Arc<VulkanContext>,
    info: &aura_core::monitor::MonitorInfo,
    workerw: windows::Win32::Foundation::HWND,
    wallpaper_path: Option<&Path>,
    fit_mode: aura_core::wallpaper::FitMode,
) -> Result<
    (
        MonitorContext,
        crossbeam_channel::Sender<RenderCommand>,
        Arc<AtomicU64>,
    ),
    DaemonError,
> {
    let host_window = HostWindow::create()?;
    if !workerw.0.is_null() {
        if let Err(e) =
            aura_platform_windows::workerw::attach_to_workerw(host_window.hwnd(), workerw)
        {
            tracing::error!("Failed to attach window to WorkerW: {}", e);
        } else {
            unsafe {
                use windows::Win32::Foundation::POINT;
                use windows::Win32::Graphics::Gdi::{InvalidateRect, ScreenToClient};
                use windows::Win32::UI::WindowsAndMessaging::{MoveWindow, SW_SHOW, ShowWindow};
                let hwnd = host_window.hwnd();
                let mut pt = POINT {
                    x: info.x,
                    y: info.y,
                };
                let _ = ScreenToClient(workerw, &mut pt);
                let _ = MoveWindow(
                    hwnd,
                    pt.x,
                    pt.y,
                    info.width as i32,
                    info.height as i32,
                    true,
                );
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = InvalidateRect(Some(hwnd), None, true);

                let visible =
                    windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool();
                use windows::Win32::Foundation::RECT;
                use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
                let mut wrect = RECT::default();
                let _ = GetWindowRect(hwnd, &mut wrect);
                tracing::info!(
                    "Monitor {} host window placed at client-relative ({}, {}), size {}x{}; resulting screen rect ({},{})-({},{}) visible={}",
                    info.id,
                    pt.x,
                    pt.y,
                    info.width,
                    info.height,
                    wrect.left,
                    wrect.top,
                    wrect.right,
                    wrect.bottom,
                    visible
                );
            }
        }
    } else {
        // No valid WorkerW/Progman target at all — fall back to an unparented
        // top-level window positioned behind Progman in the top-level z-order,
        // rather than leaving the monitor with no visible window whatsoever.
        if let Err(e) = aura_platform_windows::workerw::attach_topmost_bottom(
            host_window.hwnd(),
            info.x,
            info.y,
            info.width as i32,
            info.height as i32,
        ) {
            tracing::error!("Top-level fallback placement failed: {}", e);
        }
    }

    let mut renderer = MonitorRenderer::create_win32(
        context,
        info.id,
        host_window.hwnd(),
        info.width,
        info.height,
    )?;

    renderer.set_fit_mode(fit_mode, context);

    // Upload a 1x1 black fallback so the descriptor set is valid before the render thread starts.
    let black = [0u8, 0u8, 0u8, 255u8];
    renderer.set_wallpaper_pixels(context, 1, 1, &black)?;

    // Handle wallpaper path: static image or animated GIF.
    let (initial_worker, initial_frame_rx) = if let Some(path) = wallpaper_path {
        match detect_media_kind(path) {
            Some(MediaKind::Gif) => {
                let (tx, rx) = frame_channel();
                let handle = DecodeWorkerHandle::spawn_gif_worker(path.to_owned(), tx);
                (Some(handle), Some(rx))
            }
            Some(MediaKind::Video) => {
                let (tx, rx) = frame_channel();
                let handle = DecodeWorkerHandle::spawn_video_worker(path.to_owned(), tx);
                (Some(handle), Some(rx))
            }
            Some(MediaKind::Image) => {
                let mut decoder = ImageDecoder::open(path)?;
                if let Ok(Some(frame)) = decoder.next_frame() {
                    renderer.set_wallpaper_pixels(
                        context,
                        frame.width,
                        frame.height,
                        &frame.data,
                    )?;
                }
                (None, None)
            }
            _ => {
                tracing::warn!("Unsupported wallpaper path: {}", path.display());
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    let (assign_tx, assign_rx) = crossbeam_channel::unbounded::<RenderCommand>();
    let frame_counter = Arc::new(AtomicU64::new(0));

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let pause_flag = Arc::new(AtomicBool::new(false));
    let flag_clone = shutdown_flag.clone();
    let pause_clone = pause_flag.clone();
    let context_clone = context.clone();
    let counter_clone = frame_counter.clone();

    let mut width = info.width;
    let mut height = info.height;

    let handle = std::thread::Builder::new()
        .name(format!("render-{}", info.id))
        .spawn(move || {
            let mut active_worker: Option<DecodeWorkerHandle> = initial_worker;
            let mut current_frame_rx = initial_frame_rx;
            let mut is_dirty = true;

            // Render loop: check for wallpaper commands, new frames, then draw.
            loop {
                if flag_clone.load(Ordering::Relaxed) {
                    break;
                }

                if pause_clone.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }

                if let Ok(cmd) = assign_rx.try_recv() {
                    is_dirty = true;
                    match cmd {
                        RenderCommand::SetFitMode(new_mode) => {
                            tracing::info!("Render thread received new fit mode: {:?}", new_mode);
                            renderer.set_fit_mode(new_mode, &context_clone);
                        }
                        RenderCommand::Resize {
                            width: new_w,
                            height: new_h,
                        } => {
                            tracing::info!("Render thread received resize: {}x{}", new_w, new_h);
                            width = new_w;
                            height = new_h;
                            if let Err(e) = renderer.resize(&context_clone, width, height) {
                                tracing::warn!("Resize failed: {}", e);
                            }
                        }
                        RenderCommand::SetWallpaper {
                            path: new_path,
                            fit_mode,
                        } => {
                            tracing::info!(
                                "Render thread received new wallpaper path: {:?}",
                                new_path
                            );
                            if let Some(mode) = fit_mode {
                                renderer.set_fit_mode(mode, &context_clone);
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
                                    let handle =
                                        DecodeWorkerHandle::spawn_video_worker(new_path, tx);
                                    active_worker = Some(handle);
                                    current_frame_rx = Some(rx);
                                }
                                Some(MediaKind::Image) => match ImageDecoder::open(&new_path) {
                                    Ok(mut decoder) => match decoder.next_frame() {
                                        Ok(Some(frame)) => {
                                            if let Err(e) = renderer.set_wallpaper_pixels(
                                                &context_clone,
                                                frame.width,
                                                frame.height,
                                                &frame.data,
                                            ) {
                                                tracing::warn!(
                                                    "Texture upload failed for {:?}: {}",
                                                    new_path,
                                                    e
                                                );
                                            } else {
                                                tracing::info!(
                                                    "Texture upload succeeded for {:?} ({}x{})",
                                                    new_path,
                                                    frame.width,
                                                    frame.height
                                                );
                                            }
                                        }
                                        Ok(None) => {
                                            tracing::warn!(
                                                "ImageDecoder produced no frames for {:?}",
                                                new_path
                                            );
                                        }
                                        Err(e) => {
                                            tracing::warn!(
                                                "ImageDecoder next_frame error for {:?}: {}",
                                                new_path,
                                                e
                                            );
                                        }
                                    },
                                    Err(e) => {
                                        tracing::warn!("Failed to open image {:?}: {}", new_path, e)
                                    }
                                },
                                _ => {
                                    tracing::warn!(
                                        "Unsupported or unhandled media path: {:?}",
                                        new_path
                                    );
                                }
                            }
                        }
                        RenderCommand::Playback(cmd) => {
                            if let Some(ref worker) = active_worker {
                                let _ = worker.command_sender.send(cmd);
                            }
                        }
                    }
                }

                let mut has_new_frame = false;
                if let Some(ref rx) = current_frame_rx
                    && let Some(frame) = rx.try_recv()
                {
                    has_new_frame = true;
                    if let Err(e) = renderer.set_wallpaper_pixels(
                        &context_clone,
                        frame.width,
                        frame.height,
                        &frame.data,
                    ) {
                        tracing::warn!("Texture upload failed: {}", e);
                    }
                }

                if current_frame_rx.is_some() {
                    // Animated content (GIF/Video): draw whenever new frame or dirty
                    if has_new_frame || is_dirty {
                        match renderer.frame(&context_clone, [0.0, 0.0, 0.0, 1.0]) {
                            Ok(_) => {
                                counter_clone.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(VulkanError::SwapchainOutOfDate) => {
                                if let Err(e) = renderer.resize(&context_clone, width, height) {
                                    tracing::warn!("Swapchain resize failed: {}", e);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Render frame failed: {}", e);
                            }
                        }
                        is_dirty = false;
                    }
                    std::thread::sleep(Duration::from_millis(16)); // ~60 FPS pacing
                } else {
                    // Static image content: draw once when dirty, then sleep (0% CPU/GPU idle)
                    if is_dirty {
                        match renderer.frame(&context_clone, [0.0, 0.0, 0.0, 1.0]) {
                            Ok(_) => {
                                counter_clone.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(VulkanError::SwapchainOutOfDate) => {
                                if let Err(e) = renderer.resize(&context_clone, width, height) {
                                    tracing::warn!("Swapchain resize failed: {}", e);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Render frame failed: {}", e);
                            }
                        }
                        is_dirty = false;
                    } else {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            }

            if let Some(worker) = active_worker.take() {
                worker.stop();
            }
        })
        .map_err(|_| DaemonError::ThreadSpawn)?;

    Ok((
        MonitorContext::new(
            info.id,
            host_window,
            handle,
            shutdown_flag,
            pause_flag,
            info.width,
            info.height,
            info.x,
            info.y,
        ),
        assign_tx,
        frame_counter,
    ))
}
