//! `aura-ipc` — Typed named-pipe IPC protocol.
//!
//! Provides the request/response protocol, length-prefixed JSON codec,
//! and async client/server over `\\.\pipe\aura-wallpaperd`.

pub mod client;
pub mod codec;
pub mod error;
pub mod protocol;
pub mod server;

pub use error::IpcError;
pub use protocol::{Request, Response};
