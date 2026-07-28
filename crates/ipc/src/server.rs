use std::sync::Arc;
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
    handler: Arc<RequestHandler>,
    pipe_name: String,
    first_instance: bool,
    skip_client_validation: bool,
    client_validator: Arc<aura_security::ClientValidator>,
    ready_callback: Option<Box<dyn FnOnce() + Send>>,
}

impl IpcServer {
    pub fn new(handler: RequestHandler) -> Self {
        Self {
            handler: Arc::new(handler),
            pipe_name: PIPE_NAME.to_owned(),
            first_instance: true,
            skip_client_validation: false,
            client_validator: Arc::new(aura_security::ClientValidator::new()),
            ready_callback: None,
        }
    }

    /// Create a server on a custom pipe name (for testing).
    pub fn on_pipe(handler: RequestHandler, pipe_name: impl Into<String>) -> Self {
        Self {
            handler: Arc::new(handler),
            pipe_name: pipe_name.into(),
            first_instance: true,
            skip_client_validation: true,
            client_validator: Arc::new(aura_security::ClientValidator::new()),
            ready_callback: None,
        }
    }

    /// Register a callback fired after the first named pipe instance is created
    /// and listening, signalling that the IPC server is ready for connections.
    pub fn on_ready(mut self, f: impl FnOnce() + Send + 'static) -> Self {
        self.ready_callback = Some(Box::new(f));
        self
    }

    /// Set the client PID validator for connection filtering.
    pub fn with_client_validator(mut self, validator: aura_security::ClientValidator) -> Self {
        self.client_validator = Arc::new(validator);
        self
    }

    /// Enable strict client PID validation for production use.
    pub fn with_client_validation(mut self) -> Self {
        self.skip_client_validation = false;
        self
    }

    /// Accept connections and dispatch requests until `shutdown` is signalled.
    pub async fn serve(
        mut self,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), IpcError> {
        info!("IPC server listening on {}", self.pipe_name);

        let mut retry_delay = std::time::Duration::from_millis(100);
        loop {
            let mut opts = ServerOptions::new();
            opts.first_pipe_instance(self.first_instance);
            opts.max_instances(32);

            let server = match opts.create(&self.pipe_name) {
                Ok(s) => s,
                Err(e) => {
                    warn!(
                        "Failed to create named pipe instance: {}; retrying in {:?}",
                        e, retry_delay
                    );
                    tokio::time::sleep(retry_delay).await;
                    retry_delay = (retry_delay * 2).min(std::time::Duration::from_secs(2));
                    continue;
                }
            };
            apply_pipe_dacl(&server);
            retry_delay = std::time::Duration::from_millis(100);
            // Fire ready callback on first successful pipe creation.
            if self.first_instance
                && let Some(f) = self.ready_callback.take()
            {
                f();
            }
            self.first_instance = false;

            tokio::select! {
                result = tokio::time::timeout(std::time::Duration::from_secs(30), server.connect()) => {
                    let result = match result {
                        Ok(r) => r,
                        Err(_) => {
                            warn!("IPC pipe connect timed out after 30s");
                            continue;
                        }
                    };
                    match result {
                        Ok(()) => {
                            let client_pid = get_server_client_pid(&server);
                            if let Some(pid) = client_pid {
                                if !self.skip_client_validation && !validate_client_pid(pid) {
                                    warn!("IPC connection rejected: unauthorized PID {}", pid);
                                    continue;
                                }
                                info!("IPC connection accepted from PID {}", pid);
                            }
                            let handler = self.handler.clone();
                            tokio::spawn(handle_client(server, handler));
                        }
                        Err(e) => {
                            if e.raw_os_error() == Some(535) || e.kind() == std::io::ErrorKind::AlreadyExists {
                                let handler = self.handler.clone();
                                tokio::spawn(handle_client(server, handler));
                            } else {
                                warn!("IPC pipe connect error: {}; retrying in {:?}", e, retry_delay);
                                tokio::time::sleep(retry_delay).await;
                                retry_delay = (retry_delay * 2).min(std::time::Duration::from_secs(2));
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

fn get_server_client_pid(_server: &NamedPipeServer) -> Option<u32> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::io::AsRawHandle;
        let handle = _server.as_raw_handle();
        aura_security::pipe_security::get_named_pipe_client_pid(handle as isize).ok()
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

#[cfg(target_os = "windows")]
fn apply_pipe_dacl(server: &NamedPipeServer) {
    use std::os::windows::io::AsRawHandle;
    let access = aura_security::FILE_GENERIC_READ | aura_security::FILE_GENERIC_WRITE;
    let Ok(sd) = aura_security::SecurityDescriptor::for_current_user_with_access(access) else {
        return;
    };
    aura_security::pipe_security::apply_pipe_dacl(server.as_raw_handle() as isize, &sd);
}

#[cfg(not(target_os = "windows"))]
fn apply_pipe_dacl(_server: &NamedPipeServer) {}

fn validate_client_pid(pid: u32) -> bool {
    aura_security::validate_client_pid(pid)
}

async fn handle_client(mut pipe: NamedPipeServer, handler: Arc<RequestHandler>) {
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
