use thiserror::Error;

/// Errors produced by the IPC layer.
#[derive(Debug, Error)]
pub enum IpcError {
    #[error("pipe I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialisation error: {0}")]
    Serialise(#[from] serde_json::Error),

    #[error("protocol version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u16, got: u16 },

    #[error("connection closed")]
    ConnectionClosed,

    #[error("message too large: {size} bytes (max {max})")]
    MessageTooLarge { size: usize, max: usize },
}
