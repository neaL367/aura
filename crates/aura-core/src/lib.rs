//! `aura-core` — Platform-independent domain model.
//!
//! This crate defines the core types shared across the entire Aura platform.
//! It has no dependency on Win32, Vulkan, Media Foundation, or any UI framework.

pub mod config;
pub mod error;
pub mod monitor;
pub mod playback;
pub mod slideshow_state;
pub mod wallpaper;

pub use error::CoreError;

// ---------------------------------------------------------------------------
// Shared constants
// ---------------------------------------------------------------------------

/// The window title used by both the eframe UI window and the tray icon /
/// foreground-bringing `FindWindowW` calls. Defining it here ensures that
/// all consumers always agree on the string.
pub const WINDOW_TITLE: &str = "Aura Wallpaper";
