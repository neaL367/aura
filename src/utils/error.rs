use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Win32 API error: {0}")]
    Win32(#[from] windows::core::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Media decoding error: {0}")]
    Media(String),

    #[error("Renderer error: {0}")]
    Renderer(String),

    #[error("Platform error: {0}")]
    Platform(String),

    #[error("Monitor management error: {0}")]
    Monitor(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
