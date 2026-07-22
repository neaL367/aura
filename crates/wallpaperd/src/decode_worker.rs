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
}
