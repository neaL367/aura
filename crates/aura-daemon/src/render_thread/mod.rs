pub mod loop_runner;
pub mod placement;

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    },
};

use crate::daemon::DaemonError;
use crate::decode_worker::DecodeWorkerHandle;
use crate::render_coordinator::MonitorContext;
use aura_core::playback::PlaybackCommand;
use aura_core::wallpaper::MediaKind;
use aura_core::wallpaper::detect_media_kind;
use aura_media::frame_channel;
use aura_vulkan::{
    VulkanContext,
    monitor_renderer::MonitorRenderer,
    video_decode_pipeline::{GpuAck, GpuVideoMessage},
};

pub use loop_runner::load_and_upload_static_image;

#[derive(Debug, Clone)]
pub enum RenderCommand {
    SetWallpaper {
        path: PathBuf,
        fit_mode: Option<aura_core::wallpaper::FitMode>,
    },
    /// Static wallpaper with an already-decoded frame (slideshow preload).
    SetWallpaperPredecoded {
        path: PathBuf,
        fit_mode: Option<aura_core::wallpaper::FitMode>,
        frame: Arc<aura_media::DecodedFrame>,
    },
    SetFitMode(aura_core::wallpaper::FitMode),
    Resize {
        width: u32,
        height: u32,
    },
    Playback(PlaybackCommand),
    SetPerformanceProfile(aura_core::playback::PerformanceProfile),
    SetTargetFps(u8),
    Clear,
}

#[cfg(target_os = "windows")]
pub fn create_monitor_context(
    context: &Arc<VulkanContext>,
    info: &aura_core::monitor::MonitorInfo,
    workerw: windows::Win32::Foundation::HWND,
    wallpaper_path: Option<&Path>,
    fit_mode: aura_core::wallpaper::FitMode,
    virtual_bounds: (i32, i32, u32, u32),
) -> Result<
    (
        MonitorContext,
        crossbeam_channel::Sender<RenderCommand>,
        Arc<AtomicU64>,
    ),
    DaemonError,
> {
    let host_window = placement::setup_host_window_placement(info, workerw)?;

    let mut renderer = MonitorRenderer::create_win32(
        context,
        info.id,
        host_window.hwnd(),
        info.width,
        info.height,
    )?;

    renderer.set_fit_mode(fit_mode, context);
    renderer.set_virtual_geometry(info.x, info.y, virtual_bounds.2, virtual_bounds.3);

    // Upload a 1x1 black fallback so the descriptor set is valid before the render thread starts.
    let black = [0u8, 0u8, 0u8, 255u8];
    renderer.set_wallpaper_pixels(context, 1, 1, &black)?;

    // GPU frame channel (Vulkan Video direct DPB sampling) + slot-reuse acks.
    // Only used when the wallpaper is a video decoded by the HW worker; the
    // CPU fallback keeps sending regular `DecodedFrame`s via `frame_channel`.
    let (gpu_frame_tx, gpu_frame_rx) = crossbeam_channel::bounded::<GpuVideoMessage>(2);
    let (gpu_ack_tx, gpu_ack_rx) = crossbeam_channel::unbounded::<GpuAck>();

    // Handle wallpaper path: static image or animated GIF/Video.
    // Static images are NOT decoded here — decoding happens on the render
    // thread via SetWallpaper after spawn, so a large 4K/8K decode never
    // blocks daemon startup or subsequent monitor context creation.
    let (initial_worker, initial_frame_rx) = if let Some(path) = wallpaper_path {
        match detect_media_kind(path) {
            Some(MediaKind::Gif) => {
                let (tx, rx) = frame_channel();
                let handle = DecodeWorkerHandle::spawn_gif_worker(path.to_owned(), tx);
                (Some(handle), Some(rx))
            }
            Some(MediaKind::Video) => {
                let (tx, rx) = frame_channel();
                let handle = DecodeWorkerHandle::spawn_hw_video_worker(
                    path.to_owned(),
                    tx,
                    gpu_frame_tx,
                    gpu_ack_rx,
                    context.clone(),
                );
                (Some(handle), Some(rx))
            }
            _ => (None, None),
        }
    } else {
        (None, None)
    };
    let (assign_tx, assign_rx) = crossbeam_channel::unbounded::<RenderCommand>();
    let frame_counter = Arc::new(AtomicU64::new(0));

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let pause_flag = Arc::new(AtomicBool::new(false));
    let counter_clone = frame_counter.clone();

    let width = info.width;
    let height = info.height;
    let context_clone = context.clone();
    let shutdown_clone = shutdown_flag.clone();
    let pause_clone = pause_flag.clone();

    let handle = std::thread::Builder::new()
        .name(format!("render-{}", info.id))
        .spawn(move || {
            loop_runner::run_render_loop(loop_runner::RenderLoopParams {
                renderer,
                context: context_clone,
                initial_worker,
                initial_frame_rx,
                gpu_frame_rx: Some(gpu_frame_rx),
                gpu_ack_tx: Some(gpu_ack_tx),
                assign_rx,
                shutdown_flag: shutdown_clone,
                pause_flag: pause_clone,
                counter: counter_clone,
                width,
                height,
            });
        })
        .map_err(|_| DaemonError::ThreadSpawn)?;

    // Deferred static-image load: decode + upload runs on the render thread
    // (non-blocking for daemon startup). The 1x1 black fallback was already
    // uploaded above, so the desktop presents instantly.
    if let Some(path) = wallpaper_path
        && detect_media_kind(path) == Some(MediaKind::Image)
    {
        let _ = assign_tx.send(RenderCommand::SetWallpaper {
            path: path.to_owned(),
            fit_mode: Some(fit_mode),
        });
    }

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
