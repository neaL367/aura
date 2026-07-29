use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// PlaybackState
// ---------------------------------------------------------------------------

/// Current playback state for an animated wallpaper (GIF or Video).
///
/// For static images this is always `Playing` once loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlaybackState {
    Playing,
    Paused,
    /// Decoder has not produced any frames yet.
    #[default]
    Buffering,
}

// ---------------------------------------------------------------------------
// PlaybackCommand — sent from orchestrator to decode workers
// ---------------------------------------------------------------------------

/// Commands sent to a decode worker thread to control playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackCommand {
    /// Start or resume decoding.
    Play,
    /// Suspend decoding; hold the current frame.
    Pause,
    /// Seek to the beginning and restart decoding (for looping).
    ///
    /// # Note
    ///
    /// This variant is **reserved for future IPC-driven loop control** and is
    /// not currently sent from any production code path.  Render threads handle
    /// looping internally by calling `loop_reset()` when the decoder returns
    /// `Ok(None)` (end-of-stream), making an explicit `Loop` command redundant.
    /// It is retained so the protocol type is forward-compatible and test
    /// coverage of the variant remains meaningful.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PerformanceProfile {
    /// Render at full rate; no restrictions.
    Maximum,
    /// Reduce frame rate to conserve power.
    #[default]
    Balanced,
    /// Pause all rendering.
    Paused,
}
