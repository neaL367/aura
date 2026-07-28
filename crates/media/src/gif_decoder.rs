use std::{io::BufReader, path::Path, path::PathBuf};

use gif::{DecodeOptions, DisposalMethod, Frame};

use crate::{
    decoder::{DecodedFrame, MediaDecoder},
    error::MediaError,
};

/// Streaming decoder for animated GIF files.
///
/// Keeps the GIF file open and decodes one frame at a time via
/// `read_next_frame()`, applying disposal compositing on the fly.
/// Two persistent canvases are maintained: a working canvas and a
/// snapshot for `RestoreToPrevious` disposal.
pub struct GifDecoder {
    decoder: Option<gif::Decoder<BufReader<std::fs::File>>>,
    canvas: Vec<u8>,
    before_frame: Vec<u8>,
    width: u32,
    height: u32,
    bg_color: [u8; 4],
    path: PathBuf,
}

impl GifDecoder {
    /// Open a GIF file and parse the header without reading any frames.
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        let file = std::fs::File::open(path)?;
        let mut options = DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);

        let decoder = options
            .read_info(BufReader::new(file))
            .map_err(|e| MediaError::Decode(e.to_string()))?;

        let width = decoder.width() as u32;
        let height = decoder.height() as u32;
        let bg_color = decoder
            .bg_color()
            .map(|c| {
                let palette = decoder.global_palette().unwrap_or(&[]);
                let idx = c * 3;
                if idx + 2 < palette.len() {
                    [palette[idx], palette[idx + 1], palette[idx + 2], 0xFF]
                } else {
                    [0, 0, 0, 0xFF]
                }
            })
            .unwrap_or([0, 0, 0, 0]);

        let total_pixels = (width * height * 4) as usize;
        let mut canvas = vec![0u8; total_pixels];
        for i in (0..canvas.len()).step_by(4) {
            canvas[i..i + 4].copy_from_slice(&bg_color);
        }

        Ok(Self {
            decoder: Some(decoder),
            canvas,
            before_frame: vec![0u8; total_pixels],
            width,
            height,
            bg_color,
            path: path.to_owned(),
        })
    }

    fn recreate_decoder(&self) -> Result<gif::Decoder<BufReader<std::fs::File>>, MediaError> {
        let file = std::fs::File::open(&self.path)?;
        let mut options = DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        options
            .read_info(BufReader::new(file))
            .map_err(|e| MediaError::Decode(e.to_string()))
    }
}

impl MediaDecoder for GifDecoder {
    fn next_frame(&mut self) -> Result<Option<DecodedFrame>, MediaError> {
        let decoder = match self.decoder.as_mut() {
            Some(d) => d,
            None => return Ok(None),
        };

        let frame = match decoder
            .read_next_frame()
            .map_err(|e| MediaError::Decode(e.to_string()))?
        {
            Some(f) => f,
            None => return Ok(None),
        };

        // Snapshot canvas before compositing (for RestoreToPrevious).
        self.before_frame.copy_from_slice(&self.canvas);

        // Composite frame pixels onto working canvas.
        composite_frame(&mut self.canvas, frame, self.width, self.height);

        let delay_ms = (frame.delay as u64) * 10;

        // Capture the composed canvas as the output frame BEFORE applying
        // disposal. Disposal prepares the canvas for the *next* frame, not
        // the current one. Previously, disposal was applied before cloning,
        // which caused Background/Previous frames to arrive at the renderer
        // already cleared or restored — an incorrect pre-mutation.
        let output_data = self.canvas.clone();

        // Apply disposal method to prepare canvas for the *next* frame.
        match frame.dispose {
            DisposalMethod::Keep | DisposalMethod::Any => {}
            DisposalMethod::Background => {
                clear_region(
                    &mut self.canvas,
                    frame.left as u32,
                    frame.top as u32,
                    frame.width as u32,
                    frame.height as u32,
                    self.width,
                    self.bg_color,
                );
            }
            DisposalMethod::Previous => {
                self.canvas.copy_from_slice(&self.before_frame);
            }
        }

        Ok(Some(DecodedFrame {
            width: self.width,
            height: self.height,
            data: output_data,
            timestamp_ms: 0,
            duration_ms: delay_ms.max(20),
        }))
    }

    fn loop_reset(&mut self) -> Result<(), MediaError> {
        let new_decoder = self.recreate_decoder()?;
        self.decoder = Some(new_decoder);

        // Reset canvas to background colour.
        for i in (0..self.canvas.len()).step_by(4) {
            self.canvas[i..i + 4].copy_from_slice(&self.bg_color);
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
            let alpha = frame.buffer[src_idx + 3];
            if alpha != 0 {
                canvas[dst_idx..dst_idx + 4].copy_from_slice(&frame.buffer[src_idx..src_idx + 4]);
            }
        }
    }
}

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::MediaDecoder;

    /// Build a minimal single-frame in-memory GIF (2×2, one opaque color).
    /// Returns the raw bytes of the GIF file.
    fn make_gif(width: u16, height: u16, color: [u8; 3], dispose: gif::DisposalMethod) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut buf, width, height, &[]).expect("encoder");
            encoder.set_repeat(gif::Repeat::Infinite).expect("repeat");
            let palette = color.to_vec();
            let pixel_count = (width as usize) * (height as usize);
            // Frame 1: all pixels map to palette index 0 (our color).
            let mut frame = gif::Frame {
                width,
                height,
                palette: Some(palette.clone()),
                buffer: std::borrow::Cow::Owned(vec![0u8; pixel_count]),
                dispose,
                transparent: None,
                delay: 10, // 100 ms
                ..Default::default()
            };
            encoder.write_frame(&frame).expect("frame 1");
            // Frame 2: all pixels map to palette index 0 (same color).
            frame.dispose = gif::DisposalMethod::Keep;
            encoder.write_frame(&frame).expect("frame 2");
        }
        buf
    }

    /// Open a GIF from in-memory bytes by writing to a temp file.
    fn open_gif_bytes(bytes: &[u8]) -> GifDecoder {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "aura_test_{}.gif",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::write(&path, bytes).expect("write temp gif");
        let decoder = GifDecoder::open(&path).expect("open gif");
        let _ = std::fs::remove_file(&path);
        decoder
    }

    /// Helper: extract RGBA pixel at (x, y) from a flat RGBA buffer.
    fn pixel(data: &[u8], x: u32, y: u32, width: u32) -> [u8; 4] {
        let idx = ((y * width + x) * 4) as usize;
        [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
    }

    #[test]
    fn keep_disposal_accumulates_frames() {
        // DisposalMethod::Keep — canvas is NOT cleared between frames;
        // each frame builds on top of the previous.
        let color = [0xFF_u8, 0x00, 0x00]; // red
        let bytes = make_gif(2, 2, color, gif::DisposalMethod::Keep);
        let mut dec = open_gif_bytes(&bytes);

        let f1 = dec.next_frame().expect("ok").expect("frame1");
        // Frame 1 must be the red color.
        assert_eq!(pixel(&f1.data, 0, 0, 2), [0xFF, 0x00, 0x00, 0xFF]);

        let f2 = dec.next_frame().expect("ok").expect("frame2");
        // Frame 2 is also red (Keep — canvas not cleared).
        assert_eq!(pixel(&f2.data, 0, 0, 2), [0xFF, 0x00, 0x00, 0xFF]);
    }

    #[test]
    fn background_disposal_output_is_not_premutated() {
        // DisposalMethod::Background — the frame region should be cleared on
        // the canvas AFTER the frame output is returned, not before.
        // Regression test: the bug caused the output to be cleared (bg color)
        // before the renderer received it.
        let color = [0x00_u8, 0xFF, 0x00]; // green
        let bytes = make_gif(2, 2, color, gif::DisposalMethod::Background);
        let mut dec = open_gif_bytes(&bytes);

        let f1 = dec.next_frame().expect("ok").expect("frame1");
        // The output frame must show the GREEN pixels, not the background.
        // Before the fix, Background disposal would clear the canvas before
        // cloning, so f1.data would be all-zeroes (transparent bg).
        assert_eq!(
            pixel(&f1.data, 0, 0, 2),
            [0x00, 0xFF, 0x00, 0xFF],
            "Background disposal must NOT pre-clear the output frame"
        );
    }

    #[test]
    fn previous_disposal_output_is_not_premutated() {
        // DisposalMethod::Previous — canvas is restored to the pre-frame
        // snapshot AFTER the frame output is captured.
        // Regression test: the bug caused the canvas to be restored to
        // `before_frame` (empty at frame 1) before cloning, so the output
        // would be the blank pre-frame state rather than the composed frame.
        let color = [0x00_u8, 0x00, 0xFF]; // blue
        let bytes = make_gif(2, 2, color, gif::DisposalMethod::Previous);
        let mut dec = open_gif_bytes(&bytes);

        let f1 = dec.next_frame().expect("ok").expect("frame1");
        // Output must be blue pixels, not blank/restored pre-frame.
        assert_eq!(
            pixel(&f1.data, 0, 0, 2),
            [0x00, 0x00, 0xFF, 0xFF],
            "Previous disposal must NOT restore canvas before outputting frame"
        );
    }

    #[test]
    fn any_disposal_behaves_like_keep() {
        // DisposalMethod::Any is treated identically to Keep.
        let color = [0xFF_u8, 0xFF, 0x00]; // yellow
        let bytes = make_gif(2, 2, color, gif::DisposalMethod::Any);
        let mut dec = open_gif_bytes(&bytes);

        let f1 = dec.next_frame().expect("ok").expect("frame1");
        assert_eq!(pixel(&f1.data, 0, 0, 2), [0xFF, 0xFF, 0x00, 0xFF]);
    }

    #[test]
    fn frame_duration_minimum_20ms() {
        // Frames with delay=0 (0 ms) must be clamped to 20 ms.
        let color = [0x80_u8, 0x80, 0x80];
        let mut buf = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut buf, 1, 1, &[]).expect("enc");
            encoder.set_repeat(gif::Repeat::Finite(0)).expect("repeat");
            let frame = gif::Frame {
                width: 1,
                height: 1,
                palette: Some(color.to_vec()),
                buffer: std::borrow::Cow::Owned(vec![0u8]),
                delay: 0, // 0 ms — should be clamped
                dispose: gif::DisposalMethod::Keep,
                transparent: None,
                ..Default::default()
            };
            encoder.write_frame(&frame).expect("frame");
        }
        let mut dec = open_gif_bytes(&buf);
        let f = dec.next_frame().expect("ok").expect("frame");
        assert!(
            f.duration_ms >= 20,
            "zero-delay frames must be clamped to ≥20ms, got {}",
            f.duration_ms
        );
    }
}
