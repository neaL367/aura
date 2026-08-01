use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use aura_core::playback::PlaybackCommand;
use aura_core::wallpaper::MediaKind;
use aura_core::wallpaper::detect_media_kind;
use aura_media::frame_channel;
use aura_vulkan::VulkanContext;
use aura_vulkan::video_decode_pipeline::GpuAck;

use super::{RenderCommand, RenderLoopState};
use crate::decode_worker::DecodeWorkerHandle;

impl RenderLoopState {
    /// Ack the DPB slot of the currently displayed video frame (if any) so
    /// the worker may reuse it after a wallpaper switch. The graphics
    /// timeline value signaled after sampling the slot is included, so the
    /// worker waits for it before overwriting the slot.
    pub(super) fn ack_active_video_slot(&self) {
        if let (Some(tx), Some(slot)) =
            (self.gpu_ack_tx.as_ref(), self.renderer.active_video_slot())
        {
            let gfx_value = self.renderer.active_video_sampled_gfx_value();
            let _ = tx.send(GpuAck::SlotReused { slot, gfx_value });
        }
    }

    /// Drain queued render commands (max 32 per wake) and apply them.
    pub(super) fn drain_commands(
        &mut self,
        assign_rx: &crossbeam_channel::Receiver<super::RenderCommand>,
        counter: &Arc<AtomicU64>,
        context: &Arc<VulkanContext>,
    ) {
        for _ in 0..32 {
            if let Ok(cmd) = assign_rx.try_recv() {
                self.is_dirty = true;
                match cmd {
                    RenderCommand::SetFitMode(new_mode) => {
                        tracing::info!("Render thread received new fit mode: {:?}", new_mode);
                        self.renderer.set_fit_mode(new_mode, context);
                    }
                    RenderCommand::Resize {
                        width: new_w,
                        height: new_h,
                    } => {
                        tracing::info!("Render thread received resize: {}x{}", new_w, new_h);
                        self.width = new_w;
                        self.height = new_h;
                        if let Err(e) = self.renderer.resize(context, self.width, self.height) {
                            tracing::warn!("Resize failed: {}", e);
                        }
                    }
                    RenderCommand::SetWallpaper {
                        path: new_path,
                        fit_mode,
                    } => {
                        tracing::info!("Render thread received new wallpaper path: {:?}", new_path);
                        if let Some(mode) = fit_mode {
                            self.renderer.set_fit_mode(mode, context);
                        }
                        // Release the displayed video frame's DPB slot before
                        // tearing down its worker and channels.
                        self.ack_active_video_slot();
                        if let Some(worker) = self.active_worker.take() {
                            worker.stop();
                        }
                        self.current_frame_rx = None;
                        self.gpu_frame_rx = None;
                        self.gpu_ack_tx = None;

                        match detect_media_kind(&new_path) {
                            Some(MediaKind::Gif) => {
                                let (tx, rx) = frame_channel();
                                let handle = DecodeWorkerHandle::spawn_gif_worker(new_path, tx);
                                self.active_worker = Some(handle);
                                self.current_frame_rx = Some(rx);
                            }
                            Some(MediaKind::Video) => {
                                let (tx, rx) = frame_channel();
                                let (gpu_tx, gpu_rx) = crossbeam_channel::bounded(2);
                                let (ack_tx, ack_rx) = crossbeam_channel::unbounded();
                                let handle = DecodeWorkerHandle::spawn_hw_video_worker(
                                    new_path,
                                    tx,
                                    gpu_tx,
                                    ack_rx,
                                    context.clone(),
                                );
                                self.active_worker = Some(handle);
                                self.current_frame_rx = Some(rx);
                                self.gpu_frame_rx = Some(gpu_rx);
                                self.gpu_ack_tx = Some(ack_tx);
                            }
                            Some(MediaKind::Image) => {
                                crate::render_thread::load_and_upload_static_image(
                                    &new_path,
                                    &mut self.renderer,
                                    context,
                                );
                                if self.renderer.frame(context, [0.0, 0.0, 0.0, 1.0]).is_ok() {
                                    counter.fetch_add(1, Ordering::Relaxed);
                                    self.renderer.trim_staging(context);
                                    aura_win::trim_working_set();
                                    self.is_dirty = false;
                                }
                            }
                            _ => {
                                tracing::warn!(
                                    "Unsupported or unhandled media path: {:?}",
                                    new_path
                                );
                            }
                        }
                    }
                    RenderCommand::SetWallpaperPredecoded {
                        path,
                        fit_mode,
                        frame,
                    } => {
                        tracing::info!("Render thread received pre-decoded wallpaper: {:?}", path);
                        if let Some(mode) = fit_mode {
                            self.renderer.set_fit_mode(mode, context);
                        }
                        // Release the displayed video frame's DPB slot before
                        // tearing down its worker and channels.
                        self.ack_active_video_slot();
                        if let Some(worker) = self.active_worker.take() {
                            worker.stop();
                        }
                        self.current_frame_rx = None;
                        self.gpu_frame_rx = None;
                        self.gpu_ack_tx = None;

                        if let Err(e) = self.renderer.set_wallpaper_pixels(
                            context,
                            frame.width,
                            frame.height,
                            &frame.data,
                        ) {
                            tracing::warn!(
                                "Pre-decoded upload failed ({}), falling back to decode for {:?}",
                                e,
                                path
                            );
                            crate::render_thread::load_and_upload_static_image(
                                &path,
                                &mut self.renderer,
                                context,
                            );
                        }
                        if self.renderer.frame(context, [0.0, 0.0, 0.0, 1.0]).is_ok() {
                            counter.fetch_add(1, Ordering::Relaxed);
                            self.renderer.trim_staging(context);
                            aura_win::trim_working_set();
                            self.is_dirty = false;
                        }
                    }
                    RenderCommand::Playback(cmd) => {
                        if cmd == PlaybackCommand::Play
                            && self.current_profile
                                == aura_core::playback::PerformanceProfile::Paused
                        {
                            tracing::info!(
                                "Playback(Play) received while paused; unpausing performance profile to Maximum"
                            );
                            self.current_profile = aura_core::playback::PerformanceProfile::Maximum;
                        }
                        if let Some(ref worker) = self.active_worker {
                            let _ = worker.command_sender.send(cmd);
                        }
                    }
                    RenderCommand::SetPerformanceProfile(profile) => {
                        tracing::info!(
                            profile = ?profile,
                            "Render thread performance profile updated"
                        );
                        self.current_profile = profile;
                    }
                    RenderCommand::SetTargetFps(fps) => {
                        let valid_fps = fps.clamp(1, 240);
                        tracing::info!(fps = valid_fps, "Render thread target FPS updated");
                        self.target_fps = valid_fps;
                    }
                }
            } else {
                break;
            }
        }
    }
}
