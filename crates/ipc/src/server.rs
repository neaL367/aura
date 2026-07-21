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
}

impl IpcServer {
    pub fn new(handler: RequestHandler) -> Self {
        Self {
            handler: std::sync::Arc::new(handler),
        }
    }

    /// Accept connections and dispatch requests until `shutdown` is signalled.
    pub async fn serve(
        self,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), IpcError> {
        info!("IPC server listening on {}", PIPE_NAME);

        loop {
            // Create a new pipe instance for the next client.
            let server = ServerOptions::new()
                .first_pipe_instance(false)
                .create(PIPE_NAME)
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
            warn!(got = msg.version, "IPC version mismatch");
        }

        let response = handler(msg.payload);
        let reply = IpcMessage::new(response);

        if let Err(e) = write_message(&mut pipe, &reply).await {
            error!("IPC write error: {}", e);
            break;
        }
    }
}
