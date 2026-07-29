use crate::error::MediaError;

// ---------------------------------------------------------------------------
// DecodedFrame
// ---------------------------------------------------------------------------

/// A single decoded video/GIF frame ready for GPU upload.
///
/// Pixel format is always RGBA8 (4 bytes per pixel, row-major, no padding).
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    /// RGBA8 pixel data.  Length must equal `width * height * 4`.
    pub data: Vec<u8>,
    /// Display timestamp in milliseconds from media start (0 for static images).
    pub timestamp_ms: u64,
    /// Requested display duration in milliseconds (0 = display until next frame).
    pub duration_ms: u64,
}

impl DecodedFrame {
    /// Returns the expected byte length for the given dimensions.
    pub fn expected_len(width: u32, height: u32) -> usize {
        (width as usize) * (height as usize) * 4
    }

    /// Assert that the pixel buffer has the correct length.
    pub fn validate(&self) -> Result<(), MediaError> {
        let expected = Self::expected_len(self.width, self.height);
        if self.data.len() != expected {
            return Err(MediaError::Decode(format!(
                "frame buffer size mismatch: expected {} bytes, got {}",
                expected,
                self.data.len()
            )));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MediaDecoder trait
// ---------------------------------------------------------------------------

/// Common interface for all media decoders.
///
/// Decoders are owned by dedicated worker threads and called only from
/// that thread.  The trait is `Send` so the worker can be moved across
/// thread boundaries.
pub trait MediaDecoder: Send {
    /// Decode and return the next frame.
    ///
    /// Returns `Ok(None)` at end of stream (for non-looping media).
    fn next_frame(&mut self) -> Result<Option<DecodedFrame>, MediaError>;

    /// Reset the decoder to the beginning of the media (for looping).
    fn loop_reset(&mut self) -> Result<(), MediaError>;

    /// Width of the decoded frames in pixels.
    fn width(&self) -> u32;

    /// Height of the decoded frames in pixels.
    fn height(&self) -> u32;
}
