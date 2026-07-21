//! `aura-core` — Platform-independent domain model.
//!
//! This crate defines the core types shared across the entire Aura platform.
//! It has no dependency on Win32, Vulkan, Media Foundation, or any UI framework.

pub mod config;
pub mod error;
pub mod monitor;
pub mod playback;
pub mod wallpaper;

pub use error::CoreError;
