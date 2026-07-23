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
    last_error: Arc<Mutex<Option<String>>>,
    cmd_tx: tokio::sync::mpsc::UnboundedSender<(
        Request,
        tokio::sync::oneshot::Sender<Result<Response, String>>,
    )>,
}

impl UiIpcClient {
    pub fn new(ctx: egui::Context) -> Self {
        let status = Arc::new(Mutex::new(ConnectionStatus::Connecting));
        let wallpapers = Arc::new(Mutex::new(Vec::new()));
        let last_error = Arc::new(Mutex::new(None));
        let status_clone = status.clone();
        let wallpapers_clone = wallpapers.clone();
        let last_error_clone = last_error.clone();

        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<(
            Request,
            tokio::sync::oneshot::Sender<Result<Response, String>>,
        )>();

        std::thread::Builder::new()
            .name("ipc-ui-worker".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        *status_clone.lock().unwrap() = ConnectionStatus::Error(e.to_string());
                        ctx.request_repaint();
                        return;
                    }
                };

                rt.block_on(async move {
                    loop {
                        *status_clone.lock().unwrap() = ConnectionStatus::Connecting;
                        ctx.request_repaint();
                        match IpcClient::connect().await {
                            Ok(mut client) => {
                                match client.send(Request::GetStatus).await {
                                    Ok(Response::Status(s)) => {
                                        *status_clone.lock().unwrap() = ConnectionStatus::Connected(s);
                                    }
                                    Ok(_) => {
                                        *status_clone.lock().unwrap() = ConnectionStatus::Connected(DaemonStatus {
                                            protocol_version: 1,
                                            active_monitors: 0,
                                            assigned_wallpapers: 0,
                                            is_paused: false,
                                            monitors: vec![],
                                        });
                                    }
                                    Err(e) => {
                                        *status_clone.lock().unwrap() = ConnectionStatus::Error(e.to_string());
                                        ctx.request_repaint();
                                        tokio::time::sleep(Duration::from_secs(2)).await;
                                        continue;
                                    }
                                }
                                ctx.request_repaint();

                                // Initial wallpaper list fetch
                                if let Ok(Response::WallpaperList(list)) = client.send(Request::ListWallpapers).await {
                                    tracing::info!("UI initial fetch received {} wallpaper(s) over IPC", list.len());
                                    *wallpapers_clone.lock().unwrap() = list;
                                    ctx.request_repaint();
                                }

                                loop {
                                    tokio::select! {
                                        cmd = cmd_rx.recv() => {
                                            match cmd {
                                                 Some((req, resp_tx)) => {
                                                     tracing::info!("UI sending IPC request: {:?}", req);
                                                     let res = client.send(req).await;
                                                     match &res {
                                                         Ok(Response::Status(s)) => {
                                                             tracing::info!("UI received Status update: {} monitor(s)", s.active_monitors);
                                                             *status_clone.lock().unwrap() = ConnectionStatus::Connected(s.clone());
                                                             *last_error_clone.lock().unwrap() = None;
                                                         }
                                                         Ok(Response::WallpaperList(list)) => {
                                                             tracing::info!("UI received WallpaperList with {} wallpaper(s)", list.len());
                                                             *wallpapers_clone.lock().unwrap() = list.clone();
                                                             *last_error_clone.lock().unwrap() = None;
                                                         }
                                                         Ok(Response::Error { reason }) => {
                                                             tracing::warn!("Daemon returned error: {}", reason);
                                                             *last_error_clone.lock().unwrap() = Some(reason.clone());
                                                         }
                                                         Ok(_) => {
                                                             *last_error_clone.lock().unwrap() = None;
                                                         }
                                                         Err(e) => {
                                                             tracing::warn!("IPC transport error: {}", e);
                                                             *last_error_clone.lock().unwrap() = Some(e.to_string());
                                                         }
                                                     }
                                                     ctx.request_repaint();
                                                     let _ = resp_tx.send(res.map_err(|e| e.to_string()));
                                                     ctx.request_repaint();
                                                 }
                                                None => return,
                                            }
                                        }
                                        _ = tokio::time::sleep(Duration::from_secs(3)) => {
                                            match client.send(Request::GetStatus).await {
                                                Ok(Response::Status(s)) => {
                                                    *status_clone.lock().unwrap() = ConnectionStatus::Connected(s);
                                                }
                                                Err(_e) => {
                                                    *status_clone.lock().unwrap() = ConnectionStatus::Disconnected;
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
                                *status_clone.lock().unwrap() = ConnectionStatus::Disconnected;
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

    /// Reason the most recent command failed, if any. Cleared on the next
    /// successful command.
    pub fn last_error(&self) -> Option<String> {
        self.last_error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn fetch_wallpapers(&self) {
        self.send(Request::ListWallpapers);
    }

    pub fn send(&self, req: Request) {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let _ = self.cmd_tx.send((req, tx));
    }
}
