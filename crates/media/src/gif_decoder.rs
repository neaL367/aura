use std::{io::BufReader, path::Path};

use gif::{DecodeOptions, DisposalMethod, Frame};

use crate::{
    decoder::{DecodedFrame, MediaDecoder},
    error::MediaError,
};

/// Decoder for animated GIF files.
///
/// Implements correct GIF disposal-method compositing:
/// - `DoNotDispose`: overlay next frame on top of the current canvas.
/// - `RestoreToBackground`: clear the frame region to the background colour.
/// - `RestoreToPrevious`: restore the frame region to what it was before this frame.
/// - `Any` (unspecified): treat as `DoNotDispose`.
pub struct GifDecoder {
    frames: Vec<GifFrame>,
    cursor: usize,
    width: u32,
    height: u32,
}

struct GifFrame {
    /// RGBA8 pixels (after disposal compositing).
    canvas: Vec<u8>,
    width: u32,
    height: u32,
    delay_ms: u64,
}

impl GifDecoder {
    /// Load and decode all frames of a GIF from disk.
    ///
    /// Pre-decodes the full sequence with disposal compositing applied so that
    /// the render thread never does more than a channel receive.
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        let file = std::fs::File::open(path)?;
        let mut options = DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);

        let mut decoder = options
            .read_info(BufReader::new(file))
            .map_err(|e| MediaError::Decode(e.to_string()))?;

        let canvas_w = decoder.width() as u32;
        let canvas_h = decoder.height() as u32;
        let bg_color = decoder.bg_color().map(|c| {
            let palette = decoder.global_palette().unwrap_or(&[]);
            let idx = (c as usize) * 3;
            if idx + 2 < palette.len() {
                [palette[idx], palette[idx + 1], palette[idx + 2], 0xFF]
            } else {
                [0, 0, 0, 0xFF]
            }
        }).unwrap_or([0, 0, 0, 0]);

        let mut canvas = vec![0u8; (canvas_w * canvas_h * 4) as usize];
        // Fill with background colour.
        for i in (0..canvas.len()).step_by(4) {
            canvas[i..i + 4].copy_from_slice(&bg_color);
        }

        let mut frames = Vec::new();

        while let Some(frame) = decoder
            .read_next_frame()
            .map_err(|e| MediaError::Decode(e.to_string()))?
        {
            // Save canvas before this frame for RestoreToPrevious.
            let before_frame = canvas.clone();

            // Composite frame pixels onto canvas.
            composite_frame(&mut canvas, frame, canvas_w, canvas_h);

            // Record the fully-composited canvas as this frame's output.
            let delay_ms = (frame.delay as u64) * 10; // GIF delay is in 10ms units
            frames.push(GifFrame {
                canvas: canvas.clone(),
                width: canvas_w,
                height: canvas_h,
                delay_ms: delay_ms.max(20), // minimum 20ms to avoid 0-delay spin
            });

            // Apply disposal method to prepare canvas for next frame.
            match frame.dispose {
                DisposalMethod::Keep | DisposalMethod::Any => {
                    // Leave canvas as-is.
                }
                DisposalMethod::Background => {
                    // Clear frame region to background colour.
                    clear_region(
                        &mut canvas,
                        frame.left as u32,
                        frame.top as u32,
                        frame.width as u32,
                        frame.height as u32,
                        canvas_w,
                        bg_color,
                    );
                }
                DisposalMethod::Previous => {
                    // Restore to canvas state before this frame.
                    canvas = before_frame;
                }
            }
        }

        if frames.is_empty() {
            return Err(MediaError::Decode("GIF contains no frames".into()));
        }

        Ok(Self {
            frames,
            cursor: 0,
            width: canvas_w,
            height: canvas_h,
        })
    }
}

impl MediaDecoder for GifDecoder {
    fn next_frame(&mut self) -> Result<Option<DecodedFrame>, MediaError> {
        if self.cursor >= self.frames.len() {
            return Ok(None); // end of stream; caller should call loop_reset
        }
        let f = &self.frames[self.cursor];
        self.cursor += 1;
        Ok(Some(DecodedFrame {
            width: f.width,
            height: f.height,
            data: f.canvas.clone(),
            timestamp_ms: 0, // relative timing managed by decode worker
            duration_ms: f.delay_ms,
        }))
    }

    fn loop_reset(&mut self) -> Result<(), MediaError> {
        self.cursor = 0;
        Ok(())
    }

    fn width(&self) -> u32 { self.width }
    fn height(&self) -> u32 { self.height }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Blit a GIF frame's pixels into the canvas using the frame's palette.
fn composite_frame(canvas: &mut [u8], frame: &Frame<'_>, cw: u32, ch: u32) {
    let fx = frame.left as u32;
    let fy = frame.top as u32;
    let fw = frame.width as u32;
    let fh = frame.height as u32;

    for row in 0..fh {
        for col in 0..fw {
            let src_idx = ((row * fw + col) * 4) as usize;
            let cx = fx + col;
            let cy = fy + row;
            if cx >= cw || cy >= ch {
                continue;
            }
            let dst_idx = ((cy * cw + cx) * 4) as usize;
            // GIF frames have a transparent index; alpha == 0 means transparent.
            let alpha = frame.buffer[src_idx + 3];
            if alpha != 0 {
                canvas[dst_idx..dst_idx + 4]
                    .copy_from_slice(&frame.buffer[src_idx..src_idx + 4]);
            }
        }
    }
}

/// Fill a rectangular region of the canvas with a solid colour.
fn clear_region(canvas: &mut [u8], x: u32, y: u32, w: u32, h: u32, cw: u32, color: [u8; 4]) {
    for row in 0..h {
        for col in 0..w {
            let idx = (((y + row) * cw + (x + col)) * 4) as usize;
            if idx + 3 < canvas.len() {
                canvas[idx..idx + 4].copy_from_slice(&color);
            }
        }
    }
}
