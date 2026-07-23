pub mod loop_runner;
pub mod placement;

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    },
};

use aura_core::playback::PlaybackCommand;
use aura_core::wallpaper::MediaKind;
use aura_core::wallpaper::detect_media_kind;
use aura_media::{ImageDecoder, MediaDecoder, frame_channel};
use aura_renderer_vulkan::{VulkanContext, monitor_renderer::MonitorRenderer};

use crate::daemon::DaemonError;
use crate::decode_worker::DecodeWorkerHandle;
use crate::render_coordinator::MonitorContext;

pub use loop_runner::load_and_upload_static_image;

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
    SetPerformanceProfile(aura_core::playback::PerformanceProfile),
    SetTargetFps(u8),
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
    let host_window = placement::setup_host_window_placement(info, workerw)?;

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

    // Handle wallpaper path: static image or animated GIF/Video.
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
                assign_rx,
                shutdown_flag: shutdown_clone,
                pause_flag: pause_clone,
                counter: counter_clone,
                width,
                height,
            });
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
