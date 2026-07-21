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
    /// Connect to the wallpaperd named pipe.
    ///
    /// Retries once if the server is busy (ERROR_PIPE_BUSY).
    pub async fn connect() -> Result<Self, IpcError> {
        let pipe = loop {
            match ClientOptions::new().open(PIPE_NAME) {
                Ok(p) => break p,
                Err(e) if e.raw_os_error() == Some(231) => {
                    // ERROR_PIPE_BUSY — server has no free instances; wait briefly.
                    debug!("Pipe busy, waiting…");
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                Err(e) => return Err(IpcError::Io(e)),
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
        }
        Ok(resp.payload)
    }
}
