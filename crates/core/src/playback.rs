use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// PlaybackState
// ---------------------------------------------------------------------------

/// Current playback state for an animated wallpaper (GIF or Video).
///
/// For static images this is always `Playing` once loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    Playing,
    Paused,
    /// Decoder has not produced any frames yet.
    Buffering,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self::Buffering
    }
}

// ---------------------------------------------------------------------------
// PlaybackCommand — sent from orchestrator to decode workers
// ---------------------------------------------------------------------------

/// Commands sent to a decode worker thread to control playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackCommand {
    /// Start or resume decoding.
    Play,
    /// Suspend decoding; hold the current frame.
    Pause,
    /// Seek to the beginning and restart decoding (for looping).
    Loop,
    /// Stop decoding and release all resources.
    Stop,
}

// ---------------------------------------------------------------------------
// PerformanceProfile — controls daemon behaviour under power/session events
// ---------------------------------------------------------------------------

/// Performance behaviour profile for the wallpaper daemon.
///
/// Applied when the system enters specific states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformanceProfile {
    /// Render at full rate; no restrictions.
    Maximum,
    /// Reduce frame rate to conserve power.
    Balanced,
    /// Pause all rendering.
    Paused,
}

impl Default for PerformanceProfile {
    fn default() -> Self {
        Self::Balanced
    }
}
