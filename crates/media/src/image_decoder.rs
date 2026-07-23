use std::path::{Path, PathBuf};

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
    path: PathBuf,
    frame: Option<DecodedFrame>,
    width: u32,
    height: u32,
}

impl ImageDecoder {
    /// Load and decode a static image from disk.
    ///
    /// Downsamples images larger than 4K (3840px) to conserve RAM and converts to RGBA8.
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        let (frame, width, height) = Self::decode_frame(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            frame: Some(frame),
            width,
            height,
        })
    }

    fn decode_frame(path: &Path) -> Result<(DecodedFrame, u32, u32), MediaError> {
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

        Ok((frame, width, height))
    }
}

impl MediaDecoder for ImageDecoder {
    fn next_frame(&mut self) -> Result<Option<DecodedFrame>, MediaError> {
        Ok(self.frame.take())
    }

    fn loop_reset(&mut self) -> Result<(), MediaError> {
        if self.frame.is_none() {
            let (frame, _, _) = Self::decode_frame(&self.path)?;
            self.frame = Some(frame);
        }
        Ok(())
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}
