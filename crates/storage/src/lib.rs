//! `aura-storage` — Configuration and wallpaper library metadata persistence.

pub mod config_store;
pub mod error;
pub mod library_store;
pub mod migration;

pub use error::StorageError;
