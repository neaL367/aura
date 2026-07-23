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
            let server = ServerOptions::new()
                .first_pipe_instance(false)
                .create(&self.pipe_name)
                .map_err(IpcError::Io)?;

            // Wait for a connection or shutdown.
            tokio::select! {
                result = server.connect() => {
                    result.map_err(IpcError::Io)?;
                    let handler = self.handler.clone();
                    tokio::spawn(handle_client(server, handler));
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
