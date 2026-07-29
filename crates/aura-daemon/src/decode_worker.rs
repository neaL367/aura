use std::path::PathBuf;

use aura_core::playback::PlaybackCommand;
use aura_media::{FrameSender, GifDecoder, MediaDecoder};
use crossbeam_channel::{Receiver, Sender};

/// Handle to a background decode worker thread.
pub struct DecodeWorkerHandle {
    pub command_sender: Sender<PlaybackCommand>,
    /// JoinHandle for the worker thread.
    ///
    /// Stored so `Drop` can join the thread after signalling stop, making
    /// shutdown deterministic. Without joining, the thread may outlive the
    /// resources it holds (e.g. the frame channel receiver).
    join_handle: Option<std::thread::JoinHandle<()>>,
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
        context: std::sync::Arc<aura_vulkan::VulkanContext>,
    ) -> Self {
        spawn_hw_video_worker(path, frame_sender, context)
    }
}

/// Ensures a worker thread is always told to stop when its handle goes out of
/// scope, even if the caller forgets to call `.stop()` explicitly (e.g. on a
/// panic-unwind path, or when a handle is simply replaced/overwritten).
///
/// Sends `Stop` and then joins the thread so shutdown is deterministic:
/// after `Drop` returns, the worker is guaranteed to have finished executing.
impl Drop for DecodeWorkerHandle {
    fn drop(&mut self) {
        let _ = self.command_sender.send(PlaybackCommand::Stop);
        if let Some(handle) = self.join_handle.take()
            && let Err(e) = handle.join()
        {
            tracing::error!("Decode worker thread panicked on shutdown: {:?}", e);
        }
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

    let handle = std::thread::Builder::new()
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
                        // Use an interruptible send loop instead of send_blocking.
                        // send_blocking would block forever when the render loop is
                        // paused and the bounded frame queue is full, causing the
                        // worker to ignore Stop commands until the renderer resumes.
                        loop {
                            match frame_sender.try_send_checked(frame.clone()) {
                                Ok(()) => break, // frame delivered
                                Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                                    // Renderer gone — exit worker.
                                    break 'outer;
                                }
                                Err(crossbeam_channel::TrySendError::Full(_)) => {
                                    // Queue full — check for stop before retrying.
                                    if let Ok(cmd) = cmd_rx.try_recv()
                                        && handle_command(cmd, &cmd_rx) == ControlFlow::Stopped
                                    {
                                        break 'outer;
                                    }
                                    std::thread::sleep(std::time::Duration::from_millis(5));
                                }
                            }
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
        join_handle: Some(handle),
    }
}

/// Spawn a dedicated background thread for Media Foundation video decoding.
pub fn spawn_video_worker(path: PathBuf, frame_sender: FrameSender) -> DecodeWorkerHandle {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();

    let handle = std::thread::Builder::new()
        .name("aura-video-worker".into())
        .spawn(move || {
            let mut decoder = match aura_win::MfVideoDecoder::open(&path) {
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
        join_handle: Some(handle),
    }
}

/// Spawn a hardware-accelerated video decode worker. Tier 2 (Vulkan Video)
/// is not yet wired up; this always delegates to the Tier 1 Media
/// Foundation CPU path today.
pub fn spawn_hw_video_worker(
    path: PathBuf,
    frame_sender: FrameSender,
    context: std::sync::Arc<aura_vulkan::VulkanContext>,
) -> DecodeWorkerHandle {
    let _ = context;
    tracing::info!(
        "Vulkan Video hardware pipeline routing to Media Foundation decoder until Tier 2 frame delivery is completed"
    );
    spawn_video_worker(path, frame_sender)
}
