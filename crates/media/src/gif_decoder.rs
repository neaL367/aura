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

        // Apply disposal method to prepare canvas for next frame.
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
            data: self.canvas.clone(),
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
