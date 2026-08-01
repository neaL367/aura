use std::path::PathBuf;

use aura_core::playback::PlaybackCommand;
use aura_media::{FrameSender, GifDecoder, MediaDecoder};
use aura_vulkan::video_decode_pipeline::{GpuAck, GpuVideoMessage};
use crossbeam_channel::{Receiver, Sender};

mod ack;
mod process;
mod run;
mod session;

use run::run_vulkan_video_loop;
use session::VulkanVideoDecoder;

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

    /// Spawn a hardware-accelerated video decode worker. Frames are presented
    /// by direct DPB sampling: the worker sends `GpuVideoMessage` messages to
    /// `gpu_frame_tx` and the render thread acknowledges slot reuse via
    /// `ack_rx` (the worker never reuses a DPB slot the renderer still
    /// displays). Falls back to the Media Foundation CPU path when Vulkan
    /// Video is unavailable.
    pub fn spawn_hw_video_worker(
        path: PathBuf,
        frame_sender: FrameSender,
        gpu_frame_tx: crossbeam_channel::Sender<GpuVideoMessage>,
        ack_rx: crossbeam_channel::Receiver<GpuAck>,
        context: std::sync::Arc<aura_vulkan::VulkanContext>,
    ) -> Self {
        spawn_hw_video_worker(path, frame_sender, gpu_frame_tx, ack_rx, context)
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
            let decoder = match aura_win::MfVideoDecoder::open(&path) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("Failed to open video wallpaper {}: {}", path.display(), e);
                    return;
                }
            };

            run_cpu_video_loop(decoder, frame_sender, cmd_rx, path);
        })
        .expect("failed to spawn video decode worker thread");

    DecodeWorkerHandle {
        command_sender: cmd_tx,
        join_handle: Some(handle),
    }
}

/// Media Foundation CPU decode loop (shared by the CPU worker and as the
/// fallback path of the Vulkan Video worker).
fn run_cpu_video_loop(
    mut decoder: aura_win::MfVideoDecoder,
    frame_sender: FrameSender,
    cmd_rx: Receiver<PlaybackCommand>,
    path: PathBuf,
) {
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
}
/// Spawn a hardware-accelerated video decode worker: Vulkan Video when the
/// GPU pipeline is available, Media Foundation CPU decode as fallback. The
/// caller owns the matching `gpu_frame_rx` / `ack_tx` channel ends.
pub fn spawn_hw_video_worker(
    path: PathBuf,
    frame_sender: FrameSender,
    gpu_frame_tx: crossbeam_channel::Sender<GpuVideoMessage>,
    ack_rx: crossbeam_channel::Receiver<GpuAck>,
    context: std::sync::Arc<aura_vulkan::VulkanContext>,
) -> DecodeWorkerHandle {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();

    let handle = std::thread::Builder::new()
        .name("aura-hw-video-worker".into())
        .spawn(move || match VulkanVideoDecoder::setup(&path, &context, &gpu_frame_tx, ack_rx) {
            Ok(decoder) => {
                run_vulkan_video_loop(decoder, &context, gpu_frame_tx, frame_sender, cmd_rx, path);
            }
            Err(e) => {
                tracing::warn!(
                    "Vulkan Video unavailable ({}); falling back to Media Foundation CPU decode",
                    e
                );
                match aura_win::MfVideoDecoder::open(&path) {
                    Ok(decoder) => run_cpu_video_loop(decoder, frame_sender, cmd_rx, path),
                    Err(e2) => {
                        tracing::error!(
                            "Failed to open video wallpaper {}: {}",
                            path.display(),
                            e2
                        );
                    }
                }
            }
        })
        .expect("failed to spawn hardware video decode worker thread");

    DecodeWorkerHandle {
        command_sender: cmd_tx,
        join_handle: Some(handle),
    }
}
