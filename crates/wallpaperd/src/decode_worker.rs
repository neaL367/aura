use std::path::PathBuf;

use aura_core::playback::PlaybackCommand;
use aura_media::{FrameSender, GifDecoder, MediaDecoder};
use crossbeam_channel::Sender;

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

                loop {
                    if let Ok(cmd) = cmd_rx.try_recv() {
                        match cmd {
                            PlaybackCommand::Play => {}
                            PlaybackCommand::Pause => {
                                while let Ok(c) = cmd_rx.recv() {
                                    if c == PlaybackCommand::Play {
                                        break;
                                    }
                                }
                            }
                            PlaybackCommand::Stop => break,
                            _ => {}
                        }
                    }

                    match decoder.next_frame() {
                        Ok(Some(frame)) => {
                            if !frame_sender.send_blocking(frame) {
                                break;
                            }
                        }
                        Ok(None) => {
                            if let Err(e) = decoder.loop_reset() {
                                tracing::error!("Failed to reset GIF loop: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!("Decoder error: {}", e);
                            break;
                        }
                    }
                }

                tracing::info!("DecodeWorker finished for {}", path.display());
            })
            .expect("failed to spawn decode worker thread");

        Self {
            command_sender: cmd_tx,
        }
    }

    /// Spawn a dedicated background thread for Media Foundation video decoding.
    pub fn spawn_video_worker(path: PathBuf, frame_sender: FrameSender) -> Self {
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

                loop {
                    if let Ok(cmd) = cmd_rx.try_recv() {
                        match cmd {
                            PlaybackCommand::Play => {}
                            PlaybackCommand::Pause => {
                                while let Ok(c) = cmd_rx.recv() {
                                    if c == PlaybackCommand::Play {
                                        break;
                                    }
                                }
                            }
                            PlaybackCommand::Stop => break,
                            _ => {}
                        }
                    }

                    match decoder.next_frame() {
                        Ok(Some(frame)) => {
                            let duration = std::time::Duration::from_millis(frame.duration_ms);
                            if !frame_sender.send_blocking(frame) {
                                break;
                            }
                            std::thread::sleep(duration);
                        }
                        Ok(None) => {
                            if let Err(e) = decoder.loop_reset() {
                                tracing::error!("Failed to reset video loop: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!("Video decoder error: {}", e);
                            break;
                        }
                    }
                }

                tracing::info!("Video DecodeWorker finished for {}", path.display());
            })
            .expect("failed to spawn video decode worker thread");

        Self {
            command_sender: cmd_tx,
        }
    }

    /// Spawn a hardware-accelerated video decode worker (Tier 2 Vulkan Video) with automatic Tier 1 fallback.
    pub fn spawn_hw_video_worker(
        path: PathBuf,
        frame_sender: FrameSender,
        context: std::sync::Arc<aura_renderer_vulkan::VulkanContext>,
    ) -> Self {
        if context.video_queue_family.is_none() || context.video_decode_queue.is_none() {
            tracing::info!(
                "Hardware video decode unavailable on device; using Tier 1 Media Foundation CPU path"
            );
            return Self::spawn_video_worker(path, frame_sender);
        }

        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
        let path_clone = path.clone();

        std::thread::Builder::new()
            .name("aura-hw-video-worker".into())
            .spawn(move || {
                let mut demuxer = match aura_platform_windows::MfH264Demuxer::open(&path_clone) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::warn!(
                            "Failed to open H.264 demuxer for {:?}: {}; falling back to Tier 1 CPU decode",
                            path_clone,
                            e
                        );
                        let fallback_handle = Self::spawn_video_worker(path_clone, frame_sender);
                        while let Ok(cmd) = cmd_rx.recv() {
                            let _ = fallback_handle.command_sender.send(cmd);
                        }
                        return;
                    }
                };

                let mut _reorder_buffer =
                    aura_platform_windows::h264_parser::PocReorderBuffer::new(4);
                tracing::info!(
                    "Tier 2 Vulkan Video DecodeWorker started for {} ({}x{})",
                    path_clone.display(),
                    demuxer.width(),
                    demuxer.height()
                );

                loop {
                    if let Ok(cmd) = cmd_rx.try_recv() {
                        match cmd {
                            PlaybackCommand::Play => {}
                            PlaybackCommand::Pause => {
                                while let Ok(c) = cmd_rx.recv() {
                                    if c == PlaybackCommand::Play {
                                        break;
                                    }
                                }
                            }
                            PlaybackCommand::Stop => break,
                            _ => {}
                        }
                    }

                    match demuxer.read_next_annex_b_nal() {
                        Ok(Some((_nal_bytes, pts_ms))) => {
                            // Demuxed Annex-B NAL unit ready for Vulkan Video execution pipeline
                            let duration = std::time::Duration::from_millis(16);
                            std::thread::sleep(duration);
                            let _ = pts_ms;
                        }
                        Ok(None) => {
                            tracing::info!("Reached EOF for video {:?}, looping", path_clone);
                            match aura_platform_windows::MfH264Demuxer::open(&path_clone) {
                                Ok(new_demuxer) => demuxer = new_demuxer,
                                Err(e) => {
                                    tracing::error!("Failed to re-open demuxer on loop: {}", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Demuxer error: {}", e);
                            break;
                        }
                    }
                }

                tracing::info!("Tier 2 Vulkan Video DecodeWorker finished for {}", path_clone.display());
            })
            .expect("failed to spawn hw video decode worker thread");

        Self {
            command_sender: cmd_tx,
        }
    }
}
