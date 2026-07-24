use std::path::PathBuf;

use aura_core::playback::PlaybackCommand;
use aura_media::{FrameSender, GifDecoder, MediaDecoder};
use crossbeam_channel::{Receiver, Sender};

/// Handle to a background decode worker thread.
pub struct DecodeWorkerHandle {
    pub command_sender: Sender<PlaybackCommand>,
}

impl DecodeWorkerHandle {
    /// Signal the decode worker thread to stop execution.
    pub fn stop(&self) {
        let _ = self.command_sender.send(PlaybackCommand::Stop);
    }

    /// Spawn a dedicated background thread for GIF decoding.
    pub fn spawn_gif_worker(path: PathBuf, frame_sender: FrameSender) -> Self {
        spawn_gif_worker(path, frame_sender)
    }

    /// Spawn a dedicated background thread for Media Foundation video decoding.
    pub fn spawn_video_worker(path: PathBuf, frame_sender: FrameSender) -> Self {
        spawn_video_worker(path, frame_sender)
    }

    /// Spawn a hardware-accelerated video decode worker. Tier 2 (Vulkan Video)
    /// is not yet wired up; this always delegates to the Tier 1 Media
    /// Foundation CPU path today.
    pub fn spawn_hw_video_worker(
        path: PathBuf,
        frame_sender: FrameSender,
        context: std::sync::Arc<aura_renderer_vulkan::VulkanContext>,
    ) -> Self {
        spawn_hw_video_worker(path, frame_sender, context)
    }
}

/// Ensures a worker thread is always told to stop when its handle goes out of
/// scope, even if the caller forgets to call `.stop()` explicitly (e.g. on a
/// panic-unwind path, or when a handle is simply replaced/overwritten).
///
/// This is defense-in-depth: it does not by itself fix a paused worker
/// swallowing `Stop` (see `handle_command`), but combined with that fix it
/// guarantees a dropped handle always results in thread termination rather
/// than an orphaned, silently-resumed decode loop.
impl Drop for DecodeWorkerHandle {
    fn drop(&mut self) {
        let _ = self.command_sender.send(PlaybackCommand::Stop);
    }
}

/// Outcome of processing one command received by a decode worker.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ControlFlow {
    /// Keep decoding.
    Continue,
    /// Terminate the worker loop.
    Stopped,
}

/// Central command-handling logic, shared by every decode worker variant.
///
/// Fixes the state-machine bug: while paused, only `Play` used to be treated as
/// an exit condition, silently discarding `Stop` (and any other command) and
/// leaving the thread blocked forever. `Stop` — and a closed channel, which
/// means the handle was dropped without an explicit `.stop()` — now both
/// correctly terminate the worker instead of leaving it paused or resuming
/// playback unexpectedly.
pub fn handle_command(cmd: PlaybackCommand, cmd_rx: &Receiver<PlaybackCommand>) -> ControlFlow {
    match cmd {
        PlaybackCommand::Play => ControlFlow::Continue,
        PlaybackCommand::Stop => ControlFlow::Stopped,
        PlaybackCommand::Pause => {
            while let Ok(c) = cmd_rx.recv() {
                match c {
                    PlaybackCommand::Play => return ControlFlow::Continue,
                    PlaybackCommand::Stop => return ControlFlow::Stopped,
                    _ => {}
                }
            }
            // Sender dropped while paused (handle went away without .stop()):
            // terminate rather than silently falling through to resume decoding.
            ControlFlow::Stopped
        }
        _ => ControlFlow::Continue,
    }
}

/// Spawn a dedicated background thread for GIF decoding.
pub fn spawn_gif_worker(path: PathBuf, frame_sender: FrameSender) -> DecodeWorkerHandle {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();

    std::thread::Builder::new()
        .name("aura-decode-worker".into())
        .spawn(move || {
            let mut decoder = match GifDecoder::open(&path) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("Failed to open GIF wallpaper {}: {}", path.display(), e);
                    return;
                }
            };

            tracing::info!("DecodeWorker started for {}", path.display());

            'outer: loop {
                if let Ok(cmd) = cmd_rx.try_recv()
                    && handle_command(cmd, &cmd_rx) == ControlFlow::Stopped
                {
                    break 'outer;
                }

                match decoder.next_frame() {
                    Ok(Some(frame)) => {
                        if !frame_sender.send_blocking(frame) {
                            break 'outer;
                        }
                    }
                    Ok(None) => {
                        if let Err(e) = decoder.loop_reset() {
                            tracing::error!("Failed to reset GIF loop: {}", e);
                            break 'outer;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Decoder error: {}", e);
                        break 'outer;
                    }
                }
            }

            tracing::info!("DecodeWorker finished for {}", path.display());
        })
        .expect("failed to spawn decode worker thread");

    DecodeWorkerHandle {
        command_sender: cmd_tx,
    }
}

/// Spawn a dedicated background thread for Media Foundation video decoding.
pub fn spawn_video_worker(path: PathBuf, frame_sender: FrameSender) -> DecodeWorkerHandle {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();

    std::thread::Builder::new()
        .name("aura-video-worker".into())
        .spawn(move || {
            let mut decoder = match aura_platform_windows::MfVideoDecoder::open(&path) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("Failed to open video wallpaper {}: {}", path.display(), e);
                    return;
                }
            };

            tracing::info!("Video DecodeWorker started for {}", path.display());

            'outer: loop {
                if let Ok(cmd) = cmd_rx.try_recv()
                    && handle_command(cmd, &cmd_rx) == ControlFlow::Stopped
                {
                    break 'outer;
                }

                match decoder.next_frame() {
                    Ok(Some(frame)) => {
                        // Clamp to avoid an effective busy-loop on malformed
                        // (zero-duration) frame metadata.
                        let duration = std::time::Duration::from_millis(frame.duration_ms.max(1));
                        if !frame_sender.send_blocking(frame) {
                            break 'outer;
                        }

                        // Sleep in small increments so a Stop sent mid-frame
                        // is honored promptly instead of only being checked
                        // once per full frame duration.
                        const CHUNK: std::time::Duration = std::time::Duration::from_millis(25);
                        let mut remaining = duration;
                        while remaining > std::time::Duration::ZERO {
                            let step = remaining.min(CHUNK);
                            if let Ok(cmd) = cmd_rx.recv_timeout(step) {
                                if handle_command(cmd, &cmd_rx) == ControlFlow::Stopped {
                                    break 'outer;
                                }
                                break; // command handled (e.g. resumed from a pause); move on
                            }
                            remaining = remaining.saturating_sub(step);
                        }
                    }
                    Ok(None) => {
                        if let Err(e) = decoder.loop_reset() {
                            tracing::error!("Failed to reset video loop: {}", e);
                            break 'outer;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Video decoder error: {}", e);
                        break 'outer;
                    }
                }
            }

            tracing::info!("Video DecodeWorker finished for {}", path.display());
        })
        .expect("failed to spawn video decode worker thread");

    DecodeWorkerHandle {
        command_sender: cmd_tx,
    }
}

/// Spawn a hardware-accelerated video decode worker. Tier 2 (Vulkan Video)
/// is not yet wired up; this always delegates to the Tier 1 Media
/// Foundation CPU path today.
pub fn spawn_hw_video_worker(
    path: PathBuf,
    frame_sender: FrameSender,
    context: std::sync::Arc<aura_renderer_vulkan::VulkanContext>,
) -> DecodeWorkerHandle {
    let _ = context;
    tracing::info!(
        "Vulkan Video hardware pipeline routing to Media Foundation decoder until Tier 2 frame delivery is completed"
    );
    spawn_video_worker(path, frame_sender)
}
