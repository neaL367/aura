use std::path::Path;

use image::GenericImageView;

use crate::{
    decoder::{DecodedFrame, MediaDecoder},
    error::MediaError,
};

/// Decoder for single-frame static images (PNG, JPEG, BMP, WebP, TIFF, …).
///
/// Decodes once on construction; `next_frame` returns the cached frame.
/// After `loop_reset`, the frame is returned again (for wallpaper cycling).
pub struct ImageDecoder {
    frame: DecodedFrame,
    consumed: bool,
}

impl ImageDecoder {
    /// Load and decode a static image from disk.
    ///
    /// Converts to RGBA8 regardless of the source colour space.
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        let img = image::open(path)?;
        let (width, height) = img.dimensions();
        let rgba = img.into_rgba8();
        let data = rgba.into_raw();

        let frame = DecodedFrame {
            width,
            height,
            data,
            timestamp_ms: 0,
            duration_ms: 0, // static; renderer holds indefinitely
        };
        frame.validate()?;

        Ok(Self {
            frame,
            consumed: false,
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
        self.frame.width
    }

    fn height(&self) -> u32 {
        self.frame.height
    }
}
