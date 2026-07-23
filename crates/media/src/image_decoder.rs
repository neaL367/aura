use std::path::Path;

use image::GenericImageView;

use crate::{
    decoder::{DecodedFrame, MediaDecoder},
    error::MediaError,
};

/// Maximum dimension for static image decoding (3840x2160 4K UHD).
/// Images larger than 4K are downsampled to prevent massive uncompressed RAM bloat (e.g. 6K/8K images).
const MAX_DECODE_DIM: u32 = 3840;

/// Decoder for single-frame static images (PNG, JPEG, BMP, WebP, TIFF, …).
pub struct ImageDecoder {
    frame: DecodedFrame,
    consumed: bool,
    width: u32,
    height: u32,
}

impl ImageDecoder {
    /// Load and decode a static image from disk.
    ///
    /// Downsamples images larger than 4K (3840px) to conserve RAM and converts to RGBA8.
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        let img = image::open(path)?;
        let (orig_w, orig_h) = img.dimensions();

        let img = if orig_w > MAX_DECODE_DIM || orig_h > MAX_DECODE_DIM {
            tracing::info!(
                "Downsampling high-resolution wallpaper {:?} ({}x{}) to max {}px to save RAM",
                path,
                orig_w,
                orig_h,
                MAX_DECODE_DIM
            );
            img.thumbnail(MAX_DECODE_DIM, MAX_DECODE_DIM)
        } else {
            img
        };

        let (width, height) = img.dimensions();
        let rgba = img.into_rgba8();
        let data = rgba.into_raw();

        let frame = DecodedFrame {
            width,
            height,
            data,
            timestamp_ms: 0,
            duration_ms: 0,
        };
        frame.validate()?;

        Ok(Self {
            frame,
            consumed: false,
            width,
            height,
        })
    }
}

impl MediaDecoder for ImageDecoder {
    fn next_frame(&mut self) -> Result<Option<DecodedFrame>, MediaError> {
        if self.consumed {
            Ok(None)
        } else {
            self.consumed = true;
            Ok(Some(self.frame.clone()))
        }
    }

    fn loop_reset(&mut self) -> Result<(), MediaError> {
        self.consumed = false;
        Ok(())
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}
