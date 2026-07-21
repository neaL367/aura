//! `aura-platform-windows` — Windows 11 platform integration.
//!
//! # Module responsibilities
//!
//! - `workerw`: WorkerW discovery and `ensure_attached()`.
//! - `host_window`: Per-monitor HWND lifecycle (create, destroy, recreate).
//! - `monitor_enum`: Stable monitor enumeration with device-path IDs.
//! - `singleton`: Named-mutex process singleton.
//! - `event_pump`: Win32 message loop and `HostEvent` enum.
//! - `power`: Power / session change notifications.
//! - `mf_video`: Media Foundation video decoder.

// Only compile on Windows.
#![cfg(target_os = "windows")]

pub mod error;
pub mod event_pump;
pub mod host_window;
pub mod mf_video;
pub mod monitor_enum;
pub mod power;
pub mod singleton;
pub mod workerw;

pub use error::PlatformError;
