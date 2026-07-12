use std::path::Path;
use crate::utils::error::{AppError, Result};

/// Decodes an image (typically WebP) using the pure-Rust `image` crate.
/// Returns (width, height, raw_premultiplied_bgra_bytes).
pub fn decode_webp(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let img = image::ImageReader::open(path)
        .map_err(|e| AppError::Io(e))?
        .decode()
        .map_err(|e| AppError::Media(format!("WebP decode failed: {}", e)))?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    
    // WebP images from the image crate are decoded as standard RGBA.
    // For D3D11 compatibility and proper blending, we convert to BGRA and premultiply the alpha channel.
    let mut raw_bytes = rgba.into_raw();
    for pixel in raw_bytes.chunks_exact_mut(4) {
        // Swap Red and Blue to get BGRA
        pixel.swap(0, 2);
        
        let alpha = pixel[3] as f32 / 255.0;
        pixel[0] = (pixel[0] as f32 * alpha) as u8; // Blue
        pixel[1] = (pixel[1] as f32 * alpha) as u8; // Green
        pixel[2] = (pixel[2] as f32 * alpha) as u8; // Red
    }

    Ok((width, height, raw_bytes))
}
