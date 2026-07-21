use crate::decoder::MediaDecoder;

/// Placeholder trait for video decoders.
///
/// The concrete implementation (`MfVideoDecoder`) lives in `aura-platform-windows`
/// and uses Media Foundation.  This module provides the trait object interface
/// used by the media crate's frame pipeline.
///
/// Tier 1 path: Media Foundation → CPU-decoded RGBA8 frame → bounded channel
/// → render thread → GPU upload via staging buffer.
///
/// Zero-copy D3D11↔Vulkan interop is deferred until the Tier 1 path is
/// verified on the target hardware.
pub trait VideoDecoder: MediaDecoder {
    /// Total duration of the video in milliseconds (0 if unknown).
    fn duration_ms(&self) -> u64;
}

/// Detect whether a file is a video by inspecting its extension.
///
/// Production code should use Media Foundation's `MFCreateSourceResolver`
/// for content-based detection.  This is a lightweight fallback.
pub fn is_video_by_extension(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm")
    )
}
