use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tracing::{error, info, warn};

use crate::{
    codec::{read_message, write_message},
    error::IpcError,
    protocol::{IpcMessage, PIPE_NAME, PROTOCOL_VERSION, Request, Response},
};

/// Callback type invoked by the server to handle each request.
pub type RequestHandler = Box<dyn Fn(Request) -> Response + Send + Sync + 'static>;

/// Async IPC server — listens on the named pipe and dispatches requests.
pub struct IpcServer {
    handler: std::sync::Arc<RequestHandler>,
    pipe_name: String,
}

impl IpcServer {
    pub fn new(handler: RequestHandler) -> Self {
        Self {
            handler: std::sync::Arc::new(handler),
            pipe_name: PIPE_NAME.to_owned(),
        }
    }

    /// Create a server on a custom pipe name (for testing).
    pub fn on_pipe(handler: RequestHandler, pipe_name: impl Into<String>) -> Self {
        Self {
            handler: std::sync::Arc::new(handler),
            pipe_name: pipe_name.into(),
        }
    }

    /// Accept connections and dispatch requests until `shutdown` is signalled.
    pub async fn serve(
        self,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), IpcError> {
        info!("IPC server listening on {}", self.pipe_name);

        loop {
            let server = match ServerOptions::new()
                .first_pipe_instance(false)
                .create(&self.pipe_name)
            {
                Ok(s) => s,
                Err(e) => {
                    warn!(
                        "Failed to create named pipe instance: {}; retrying in 100ms",
                        e
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
            };

            // Wait for a connection or shutdown.
            tokio::select! {
                result = server.connect() => {
                    match result {
                        Ok(()) => {
                            let handler = self.handler.clone();
                            tokio::spawn(handle_client(server, handler));
                        }
                        Err(e) => {
                            // On Windows, if a client connects between pipe creation and connect(),
                            // ConnectNamedPipe returns ERROR_PIPE_CONNECTED (535 / 0x217) or std::io::ErrorKind::AlreadyExists.
                            // This indicates the client has already connected and the pipe instance is valid.
                            if e.raw_os_error() == Some(535) || e.kind() == std::io::ErrorKind::AlreadyExists {
                                let handler = self.handler.clone();
                                tokio::spawn(handle_client(server, handler));
                            } else {
                                warn!("IPC pipe connect error: {}; retrying in 100ms", e);
                                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            }
                        }
                    }
                }
                _ = shutdown.changed() => {
                    info!("IPC server shutting down");
                    break;
                }
            }
        }

        Ok(())
    }
}

async fn handle_client(mut pipe: NamedPipeServer, handler: std::sync::Arc<RequestHandler>) {
    loop {
        let msg: IpcMessage<Request> = match read_message(&mut pipe).await {
            Ok(m) => m,
            Err(IpcError::ConnectionClosed) => break,
            Err(e) => {
                warn!("IPC read error: {}", e);
                break;
            }
        };

        if msg.version != PROTOCOL_VERSION {
            warn!(
                got = msg.version,
                daemon = PROTOCOL_VERSION,
                "IPC version mismatch — rejecting request"
            );
            let err_response = Response::Error {
                reason: format!(
                    "protocol version mismatch (client: {}, daemon: {})",
                    msg.version, PROTOCOL_VERSION
                ),
            };
            let reply = IpcMessage::new(err_response);
            let _ = write_message(&mut pipe, &reply).await;
            break;
        }

        let handler_clone = handler.clone();
        let payload = msg.payload;
        let response = match tokio::task::spawn_blocking(move || handler_clone(payload)).await {
            Ok(resp) => resp,
            Err(e) => Response::Error {
                reason: format!("request execution failed: {}", e),
            },
        };

        let reply = IpcMessage::new(response);

        if let Err(e) = write_message(&mut pipe, &reply).await {
            error!("IPC write error: {}", e);
            break;
        }
    }
}
