use thiserror::Error;

/// Top-level error type for the `aura-core` crate.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid wallpaper state transition: {0}")]
    StateTransition(#[from] crate::wallpaper::StateTransitionError),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
