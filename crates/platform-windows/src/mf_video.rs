use std::path::Path;

use aura_media::{
    decoder::{DecodedFrame, MediaDecoder},
    error::MediaError,
};

use crate::error::PlatformError;

/// Media Foundation video decoder (stub — full implementation in Phase 7).
///
/// # Full implementation plan
///
/// Tier 1 path (CPU decode):
/// - `MFStartup` / `MFCreateSourceResolver` / `IMFMediaSource`
/// - `IMFSourceReader` for per-frame reads
/// - Convert to RGBA8 via `MFVideoFormat_ARGB32` or manual transform
/// - Push frames into `FrameSender` channel
///
/// Tier 2 (D3D11 shared texture — future):
/// - `MFCreateDXGIDeviceManager` + D3D11↔Vulkan interop
/// - Zero-copy via external memory extension
pub struct MfVideoDecoder {
    #[allow(dead_code)]
    path: std::path::PathBuf,
    width: u32,
    height: u32,
}

impl MfVideoDecoder {
    /// Open a video file for decoding.
    ///
    /// Currently a stub that stores the path without initialising Media Foundation.
    pub fn open(path: &Path) -> Result<Self, PlatformError> {
        // TODO: call MFStartup, MFCreateSourceReader, etc.
        tracing::warn!(
            path = %path.display(),
            "MfVideoDecoder: stub — Media Foundation not yet initialised"
        );
        Ok(Self {
            path: path.to_path_buf(),
            width: 1920,
            height: 1080,
        })
    }
}

impl MediaDecoder for MfVideoDecoder {
    fn next_frame(&mut self) -> Result<Option<DecodedFrame>, MediaError> {
        // Stub: return a black frame.
        let data = vec![0u8; (self.width * self.height * 4) as usize];
        Ok(Some(DecodedFrame {
            width: self.width,
            height: self.height,
            data,
            timestamp_ms: 0,
            duration_ms: 33, // ~30fps placeholder
        }))
    }

    fn loop_reset(&mut self) -> Result<(), MediaError> {
        Ok(()) // TODO: seek to 0
    }

    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
}
