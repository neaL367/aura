use aura_ipc::client::IpcClient;
use aura_ipc::protocol::{DaemonStatus, Request, Response, WallpaperEntry};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected(DaemonStatus),
    Error(String),
}

pub struct UiIpcClient {
    status: Arc<Mutex<ConnectionStatus>>,
    wallpapers: Arc<Mutex<Vec<WallpaperEntry>>>,
    config: Arc<Mutex<Option<aura_core::config::AppConfig>>>,
    last_error: Arc<Mutex<Option<String>>>,
    cmd_tx: tokio::sync::mpsc::UnboundedSender<Request>,
}

impl UiIpcClient {
    pub fn new(ctx: egui::Context) -> Self {
        let status = Arc::new(Mutex::new(ConnectionStatus::Connecting));
        let wallpapers = Arc::new(Mutex::new(Vec::new()));
        let config = Arc::new(Mutex::new(None));
        let last_error = Arc::new(Mutex::new(None));
        let status_clone = status.clone();
        let wallpapers_clone = wallpapers.clone();
        let config_clone = config.clone();
        let last_error_clone = last_error.clone();

        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<Request>();

        std::thread::Builder::new()
            .name("ipc-ui-worker".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        *status_clone.lock().unwrap_or_else(|err| err.into_inner()) =
                            ConnectionStatus::Error(e.to_string());
                        ctx.request_repaint();
                        return;
                    }
                };

                rt.block_on(async move {
                    loop {
                        *status_clone.lock().unwrap_or_else(|err| err.into_inner()) =
                            ConnectionStatus::Connecting;
                        ctx.request_repaint();
                        match IpcClient::connect().await {
                            Ok(mut client) => {
                                match client.send(Request::GetStatus).await {
                                    Ok(Response::Status(s)) => {
                                        *status_clone
                                            .lock()
                                            .unwrap_or_else(|err| err.into_inner()) =
                                            ConnectionStatus::Connected(s);
                                    }
                                    Ok(_) => {
                                        *status_clone
                                            .lock()
                                            .unwrap_or_else(|err| err.into_inner()) =
                                            ConnectionStatus::Connected(DaemonStatus {
                                                protocol_version: 1,
                                                active_monitors: 0,
                                                assigned_wallpapers: 0,
                                                is_paused: false,
                                                monitors: vec![],
                                            });
                                    }
                                    Err(e) => {
                                        *status_clone
                                            .lock()
                                            .unwrap_or_else(|err| err.into_inner()) =
                                            ConnectionStatus::Error(e.to_string());
                                        ctx.request_repaint();
                                        tokio::time::sleep(Duration::from_secs(2)).await;
                                        continue;
                                    }
                                }
                                ctx.request_repaint();

                                // Initial wallpaper list fetch
                                if let Ok(Response::WallpaperList(list)) =
                                    client.send(Request::ListWallpapers).await
                                {
                                    tracing::info!(
                                        "UI initial fetch received {} wallpaper(s) over IPC",
                                        list.len()
                                    );
                                    *wallpapers_clone
                                        .lock()
                                        .unwrap_or_else(|err| err.into_inner()) = list;
                                    ctx.request_repaint();
                                }

                                let mut health_check =
                                    tokio::time::interval(Duration::from_secs(3));
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
                                                            *status_clone.lock().unwrap_or_else(|err| err.into_inner()) = ConnectionStatus::Connected(s.clone());
                                                            *last_error_clone.lock().unwrap_or_else(|err| err.into_inner()) = None;
                                                        }
                                                        Ok(Response::WallpaperList(list)) => {
                                                            tracing::info!("UI received WallpaperList with {} wallpaper(s)", list.len());
                                                            *wallpapers_clone.lock().unwrap_or_else(|err| err.into_inner()) = list.clone();
                                                            *last_error_clone.lock().unwrap_or_else(|err| err.into_inner()) = None;
                                                        }
                                                        Ok(Response::Config(c)) => {
                                                            tracing::info!("UI received Config update");
                                                            *config_clone.lock().unwrap_or_else(|err| err.into_inner()) = Some(c.clone());
                                                            *last_error_clone.lock().unwrap_or_else(|err| err.into_inner()) = None;
                                                        }
                                                        Ok(Response::Error { reason }) => {
                                                            tracing::warn!("Daemon returned error: {}", reason);
                                                            *last_error_clone.lock().unwrap_or_else(|err| err.into_inner()) = Some(reason.clone());
                                                        }
                                                        Ok(_) => {
                                                            *last_error_clone.lock().unwrap_or_else(|err| err.into_inner()) = None;
                                                        }
                                                        Err(e) => {
                                                            tracing::warn!("IPC transport error: {}", e);
                                                            *last_error_clone.lock().unwrap_or_else(|err| err.into_inner()) = Some(e.to_string());
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
                                                    *status_clone.lock().unwrap_or_else(|err| err.into_inner()) = ConnectionStatus::Connected(s);
                                                }
                                                Err(_e) => {
                                                    *status_clone.lock().unwrap_or_else(|err| err.into_inner()) = ConnectionStatus::Disconnected;
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
                                *status_clone.lock().unwrap_or_else(|err| err.into_inner()) =
                                    ConnectionStatus::Disconnected;
                                ctx.request_repaint();
                                tokio::time::sleep(Duration::from_secs(2)).await;
                            }
                        }
                    }
                });
            })
            .expect("Failed to spawn UI IPC worker thread");

        Self {
            status,
            wallpapers,
            config,
            last_error,
            cmd_tx,
        }
    }

    pub fn status(&self) -> ConnectionStatus {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn wallpapers(&self) -> Vec<WallpaperEntry> {
        self.wallpapers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn config(&self) -> Option<aura_core::config::AppConfig> {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Reason the most recent command failed, if any. Cleared on the next
    /// successful command.
    pub fn last_error(&self) -> Option<String> {
        self.last_error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    #[expect(dead_code, reason = "convenience wrapper for manual refresh")]
    pub fn fetch_wallpapers(&self) {
        self.send(Request::ListWallpapers);
    }

    pub fn send(&self, req: Request) {
        let _ = self.cmd_tx.send(req);
    }
}
