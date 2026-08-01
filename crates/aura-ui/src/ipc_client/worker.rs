use std::sync::{Arc, Mutex};
use std::time::Duration;

use aura_ipc::client::IpcClient;
use aura_ipc::protocol::{DaemonStatus, Request, Response, WallpaperEntry};

use super::types::ConnectionStatus;
use crate::toast::{ToastEvent, ToastKind};

pub fn spawn_ipc_worker(
    ctx: egui::Context,
    status: Arc<Mutex<ConnectionStatus>>,
    wallpapers: Arc<Mutex<Vec<WallpaperEntry>>>,
    config: Arc<Mutex<Option<aura_core::config::AppConfig>>>,
    last_error: Arc<Mutex<Option<String>>>,
    mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
    toast_tx: std::sync::mpsc::Sender<ToastEvent>,
) {
    std::thread::Builder::new()
        .name("ipc-ui-worker".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    *status.lock().unwrap_or_else(|err| err.into_inner()) =
                        ConnectionStatus::Error(e.to_string());
                    ctx.request_repaint();
                    return;
                }
            };

            rt.block_on(async move {
                loop {
                    *status.lock().unwrap_or_else(|err| err.into_inner()) =
                        ConnectionStatus::Connecting;
                    ctx.request_repaint();
                    match IpcClient::connect().await {
                        Ok(mut client) => {
                            let connected = match client.send(Request::GetStatus).await {
                                Ok(Response::Status(s)) => {
                                    *status.lock().unwrap_or_else(|err| err.into_inner()) =
                                        ConnectionStatus::Connected(s.clone());
                                    true
                                }
                                Ok(_) => {
                                    *status.lock().unwrap_or_else(|err| err.into_inner()) =
                                        ConnectionStatus::Connected(DaemonStatus {
                                            protocol_version: 1,
                                            active_monitors: 0,
                                            assigned_wallpapers: 0,
                                            is_paused: false,
                                            monitors: vec![],
                                        });
                                    true
                                }
                                Err(e) => {
                                    *status.lock().unwrap_or_else(|err| err.into_inner()) =
                                        ConnectionStatus::Error(e.to_string());
                                    ctx.request_repaint();
                                    tokio::time::sleep(Duration::from_secs(2)).await;
                                    continue;
                                }
                            };
                            if connected {
                                let _ = toast_tx
                                    .send(("Connected to Aura daemon".into(), ToastKind::Success));
                            }
                            ctx.request_repaint();

                            // Initial wallpaper + config fetch
                            if let Ok(Response::WallpaperList(list)) =
                                client.send(Request::ListWallpapers).await
                            {
                                tracing::info!(
                                    "UI initial fetch received {} wallpaper(s) over IPC",
                                    list.len()
                                );
                                *wallpapers.lock().unwrap_or_else(|err| err.into_inner()) = list;
                                ctx.request_repaint();
                            }
                            if let Ok(Response::Config(cfg)) = client.send(Request::GetConfig).await
                            {
                                *config.lock().unwrap_or_else(|err| err.into_inner()) = Some(cfg);
                                ctx.request_repaint();
                            }

                            let mut health_check = tokio::time::interval(Duration::from_secs(3));
                            loop {
                                tokio::select! {
                                    cmd = cmd_rx.recv() => {
                                        match cmd {
                                            Some(req) => {
                                                tracing::info!("UI sending IPC request: {:?}", req);
                                                let res = client.send(req).await;
                                                match &res {
                                                    Ok(Response::Status(s)) => {
                                                        tracing::info!("UI received Status update: {} monitor(s)", s.active_monitors);
                                                        *status.lock().unwrap_or_else(|err| err.into_inner()) = ConnectionStatus::Connected(s.clone());
                                                        *last_error.lock().unwrap_or_else(|err| err.into_inner()) = None;
                                                    }
                                                    Ok(Response::WallpaperList(list)) => {
                                                        tracing::info!("UI received WallpaperList with {} wallpaper(s)", list.len());
                                                        *wallpapers.lock().unwrap_or_else(|err| err.into_inner()) = list.clone();
                                                        *last_error.lock().unwrap_or_else(|err| err.into_inner()) = None;
                                                        let _ = toast_tx.send((
                                                            format!("Library updated — {} wallpaper(s)", list.len()),
                                                            ToastKind::Success,
                                                        ));
                                                        ctx.request_repaint();
                                                    }
                                                    Ok(Response::Config(c)) => {
                                                        tracing::info!("UI received Config update");
                                                        *config.lock().unwrap_or_else(|err| err.into_inner()) = Some(c.clone());
                                                        *last_error.lock().unwrap_or_else(|err| err.into_inner()) = None;
                                                    }
                                                    Ok(Response::Error { reason }) => {
                                                        tracing::warn!("Daemon returned error: {}", reason);
                                                        *last_error.lock().unwrap_or_else(|err| err.into_inner()) = Some(reason.clone());
                                                        let _ = toast_tx
                                                            .send((reason.clone(), ToastKind::Error));
                                                        ctx.request_repaint();
                                                    }
                                                    Ok(_) => {
                                                        *last_error.lock().unwrap_or_else(|err| err.into_inner()) = None;
                                                    }
                                                    Err(e) => {
                                                        tracing::warn!("IPC transport error: {}", e);
                                                        *last_error.lock().unwrap_or_else(|err| err.into_inner()) = Some(e.to_string());
                                                    }
                                                }
                                                ctx.request_repaint();
                                            }
                                            None => return,
                                        }
                                    }
                                    _ = health_check.tick() => {
                                        match client.send(Request::GetStatus).await {
                                            Ok(Response::Status(s)) => {
                                                *status.lock().unwrap_or_else(|err| err.into_inner()) = ConnectionStatus::Connected(s);
                                            }
                                            Err(_e) => {
                                                *status.lock().unwrap_or_else(|err| err.into_inner()) = ConnectionStatus::Disconnected;
                                                let _ = toast_tx
                                                    .send(("Lost connection to Aura daemon".into(), ToastKind::Error));
                                                ctx.request_repaint();
                                                break;
                                            }
                                            _ => {}
                                        }
                                        ctx.request_repaint();
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            *status.lock().unwrap_or_else(|err| err.into_inner()) =
                                ConnectionStatus::Disconnected;
                            ctx.request_repaint();
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                    }
                }
            });
        })
        .expect("Failed to spawn UI IPC worker thread");
}
