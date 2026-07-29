use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML serialisation error: {0}")]
    Toml(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("configuration migration failed: {0}")]
    Migration(String),
    #[error("corrupt configuration: {0}")]
    Corrupt(String),
}

impl From<toml::de::Error> for StorageError {
    fn from(e: toml::de::Error) -> Self {
        Self::Toml(e.to_string())
    }
}

impl From<toml::ser::Error> for StorageError {
    fn from(e: toml::ser::Error) -> Self {
        Self::Toml(e.to_string())
    }
}
