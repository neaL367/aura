//! `aura-storage` — Configuration and wallpaper library metadata persistence.

pub mod config_store;
pub mod error;
pub mod library_store;
pub mod migration;
pub mod scanner;
pub mod thumbnail;
pub mod watcher;

pub use error::StorageError;
pub use scanner::LibraryScanner;
pub use thumbnail::ThumbnailStore;
pub use watcher::LibraryWatcher;
