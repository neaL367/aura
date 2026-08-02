use tokio::net::windows::named_pipe::ClientOptions;
use tracing::{debug, warn};

use crate::{
    codec::{read_message, write_message},
    error::IpcError,
    protocol::{IpcMessage, PIPE_NAME, PROTOCOL_VERSION, Request, Response},
};

/// Async IPC client — connects to `wallpaperd` and sends typed requests.
pub struct IpcClient {
    pipe: tokio::net::windows::named_pipe::NamedPipeClient,
}

impl IpcClient {
    /// Connect to the wallpaperd named pipe (uses `PIPE_NAME`).
    pub async fn connect() -> Result<Self, IpcError> {
        Self::connect_to(PIPE_NAME).await
    }

    /// Connect to a custom named pipe (for testing).
    ///
    /// Retries up to `MAX_ATTEMPTS` times (every 50 ms) when the pipe is busy
    /// (`ERROR_PIPE_BUSY` / OS error 231), then returns [`IpcError::PipeBusy`].
    ///
    /// # Errors
    ///
    /// - [`IpcError::PipeBusy`] – pipe remained busy for all retry attempts.
    /// - [`IpcError::Io`] – any other OS-level I/O failure.
    pub async fn connect_to(pipe_name: &str) -> Result<Self, IpcError> {
        const MAX_ATTEMPTS: u32 = 40; // ~2 s at 50 ms cadence
        let mut attempts = 0u32;
        let pipe = loop {
            attempts += 1;
            match ClientOptions::new().open(pipe_name) {
                Ok(p) => break p,
                Err(e) => {
                    let is_busy = matches!(e.raw_os_error(), Some(231 | 170));
                    if is_busy && attempts < MAX_ATTEMPTS {
                        debug!(attempt = attempts, "Pipe busy, retrying…");
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    } else if is_busy {
                        return Err(IpcError::PipeBusy { attempts });
                    } else {
                        return Err(IpcError::Io(e));
                    }
                }
            }
        };
        Ok(Self { pipe })
    }

    /// Send a request and receive a response.
    pub async fn send(&mut self, request: Request) -> Result<Response, IpcError> {
        let msg = IpcMessage::new(request);
        write_message(&mut self.pipe, &msg).await?;

        let resp: IpcMessage<Response> = read_message(&mut self.pipe).await?;
        if resp.version != PROTOCOL_VERSION {
            warn!(
                got = resp.version,
                expected = PROTOCOL_VERSION,
                "IPC protocol version mismatch"
            );
            return Err(IpcError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                got: resp.version,
            });
        }
        Ok(resp.payload)
    }

    /// Send a pre-constructed IpcMessage (useful for testing custom headers/versions).
    pub async fn send_message(&mut self, msg: IpcMessage<Request>) -> Result<Response, IpcError> {
        write_message(&mut self.pipe, &msg).await?;
        let resp: IpcMessage<Response> = read_message(&mut self.pipe).await?;
        if resp.version != PROTOCOL_VERSION {
            warn!(
                got = resp.version,
                expected = PROTOCOL_VERSION,
                "IPC protocol version mismatch"
            );
            return Err(IpcError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                got: resp.version,
            });
        }
        Ok(resp.payload)
    }
}
