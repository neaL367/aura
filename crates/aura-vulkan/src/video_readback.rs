//! NV12 -> RGBA8 software conversion (reference implementation).
//!
//! The interim M2a host readback path (`VideoReadback`) was superseded by
//! graphics-side NV12 sampling: the renderer samples the DPB image views
//! directly through the session's `VkSamplerYcbcrConversion`, so no readback
//! or CPU conversion happens in the playback path. This software converter is
//! kept as a bit-exact reference for the GPU conversion and for unit tests.

use crate::error::VulkanError;

/// Convert an NV12 (Y plane + interleaved U/V plane) buffer to RGBA8.
///
/// BT.709 limited-range coefficients, 4:2:0 subsampling (the 2x2 chroma block
/// is duplicated across the 2x2 luma block). Intentionally plain and fast
/// (`i32` math, no SIMD).
pub fn nv12_to_rgba(nv12: &[u8], width: u32, height: u32) -> Result<Vec<u8>, VulkanError> {
    let w = width as usize;
    let h = height as usize;
    let expected = w * h * 3 / 2;
    if nv12.len() < expected {
        return Err(VulkanError::Video(format!(
            "NV12 buffer too small: {} < {}",
            nv12.len(),
            expected
        )));
    }

    let y_plane = &nv12[..w * h];
    let uv_plane = &nv12[w * h..];
    let mut out = vec![0u8; w * h * 4];

    for y in 0..h {
        let uv_row = (y / 2) * w;
        for x in 0..w {
            let uv = uv_plane
                .get(uv_row + (x / 2) * 2 + 1)
                .copied()
                .unwrap_or(128) as i32
                - 128;
            let cb = uv_plane.get(uv_row + (x / 2) * 2).copied().unwrap_or(128) as i32 - 128;
            let yv = y_plane[y * w + x] as i32;
            let y_adj = ((1164 * (yv - 16)).max(0) + 500) / 1000;

            let r = (y_adj + (1793 * uv + 500) / 1000).clamp(0, 255);
            let g = (y_adj - (213 * cb + 500) / 1000 - (533 * uv + 500) / 1000).clamp(0, 255);
            let b = (y_adj + (2112 * cb + 500) / 1000).clamp(0, 255);

            let o = (y * w + x) * 4;
            out[o] = r as u8;
            out[o + 1] = g as u8;
            out[o + 2] = b as u8;
            out[o + 3] = 255;
        }
    }
    Ok(out)
}
