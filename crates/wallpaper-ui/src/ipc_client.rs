use aura_ipc::client::IpcClient;
use aura_ipc::protocol::{DaemonStatus, Request, Response};
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
    cmd_tx: tokio::sync::mpsc::UnboundedSender<(
        Request,
        tokio::sync::oneshot::Sender<Result<Response, String>>,
    )>,
}

impl UiIpcClient {
    pub fn new() -> Self {
        let status = Arc::new(Mutex::new(ConnectionStatus::Connecting));
        let status_clone = status.clone();
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
                        return;
                    }
                };

                rt.block_on(async move {
                    loop {
                        *status_clone.lock().unwrap() = ConnectionStatus::Connecting;
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
                                        });
                                    }
                                    Err(e) => {
                                        *status_clone.lock().unwrap() = ConnectionStatus::Error(e.to_string());
                                        tokio::time::sleep(Duration::from_secs(2)).await;
                                        continue;
                                    }
                                }

                                loop {
                                    tokio::select! {
                                        cmd = cmd_rx.recv() => {
                                            match cmd {
                                                Some((req, resp_tx)) => {
                                                    let res = client.send(req).await.map_err(|e| e.to_string());
                                                    let _ = resp_tx.send(res);
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
                                                    break;
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                *status_clone.lock().unwrap() = ConnectionStatus::Disconnected;
                                tokio::time::sleep(Duration::from_secs(2)).await;
                            }
                        }
                    }
                });
            })
            .expect("Failed to spawn UI IPC worker thread");

        Self { status, cmd_tx }
    }

    pub fn status(&self) -> ConnectionStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn send(&self, req: Request) {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let _ = self.cmd_tx.send((req, tx));
    }
}
