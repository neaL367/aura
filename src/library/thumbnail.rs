use std::path::Path;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use windows::core::{Interface, PCWSTR};
use windows::Win32::Graphics::Imaging::{
    IWICImagingFactory, CLSID_WICImagingFactory,
    GUID_ContainerFormatPng, GUID_WICPixelFormat24bppBGR,
    WICDecodeMetadataCacheOnDemand, WICBitmapInterpolationModeFant,
    IWICBitmapSource, WICBitmapEncoderNoCache, WICBitmapDitherTypeNone,
    WICBitmapPaletteTypeCustom,
};
use crate::utils::error::{AppError, Result};
use tracing::info;

/// Computes a fast metadata-based hash of a file path, size, and modification time.
/// Avoids reading the file contents.
pub fn get_metadata_hash(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    if let Ok(meta) = std::fs::metadata(path) {
        if let Ok(modified) = meta.modified() {
            modified.hash(&mut hasher);
        }
        meta.len().hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

/// Generates a thumbnail image from `src_path` and saves it to `dest_path`
/// with its long edge scaled down to at most `max_dim` pixels.
pub fn generate_thumbnail(src_path: &Path, dest_path: &Path, max_dim: u32) -> Result<()> {
    // Return early if the file is a video since video thumbnails are deferred to M8.
    let ext = src_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if matches!(ext.as_str(), "mp4" | "webm" | "mov") {
        info!("Skipping thumbnail generation for video: {:?}", src_path);
        return Ok(());
    }

    let src_wide = to_wide(src_path);

    unsafe {
        let factory: IWICImagingFactory = windows::Win32::System::Com::CoCreateInstance(
            &CLSID_WICImagingFactory,
            None,
            windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
        )?;

        // 1. Decode source image
        let decoder = factory.CreateDecoderFromFilename(
            PCWSTR(src_wide.as_ptr()),
            None,
            windows::Win32::Foundation::GENERIC_ACCESS_RIGHTS(0x80000000), // GENERIC_READ
            WICDecodeMetadataCacheOnDemand,
        )?;

        let frame = decoder.GetFrame(0)?;
        let mut src_w = 0;
        let mut src_h = 0;
        frame.GetSize(&mut src_w, &mut src_h)?;

        if src_w == 0 || src_h == 0 {
            return Err(AppError::Platform("Source image size is zero".into()));
        }

        // 2. Compute target dimensions preserving aspect ratio
        let (target_w, target_h) = if src_w > src_h {
            let ratio = src_h as f32 / src_w as f32;
            let tw = max_dim.min(src_w);
            let th = (tw as f32 * ratio) as u32;
            (tw, th.max(1))
        } else {
            let ratio = src_w as f32 / src_h as f32;
            let th = max_dim.min(src_h);
            let tw = (th as f32 * ratio) as u32;
            (tw.max(1), th)
        };

        // 3. Scale frame
        let scaler = factory.CreateBitmapScaler()?;
        let frame_source: IWICBitmapSource = frame.cast()?;
        scaler.Initialize(
            &frame_source,
            target_w,
            target_h,
            WICBitmapInterpolationModeFant,
        )?;

        // 4. Format convert scaler output to 24bpp BGR (standard for JPEG encoding)
        let converter = factory.CreateFormatConverter()?;
        let scaler_source: IWICBitmapSource = scaler.cast()?;
        converter.Initialize(
            &scaler_source,
            &GUID_WICPixelFormat24bppBGR,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;

        // Ensure parent directories for destination exist
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 5. Initialize WIC encoder for PNG (lossless, high-quality thumbnails)
        let encoder = factory.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
        let stream = factory.CreateStream()?;
        let dest_wide = to_wide(dest_path);
        
        stream.InitializeFromFilename(
            PCWSTR(dest_wide.as_ptr()),
            windows::Win32::Foundation::GENERIC_WRITE.0,
        )?;

        encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;

        let mut frame_encode = None;
        let mut property_bag = None;
        encoder.CreateNewFrame(&mut frame_encode, &mut property_bag)?;
        
        let frame_encode = frame_encode.ok_or_else(|| {
            AppError::Platform("Failed to create encoder frame".into())
        })?;

        frame_encode.Initialize(property_bag.as_ref())?;

        let mut pixel_format = GUID_WICPixelFormat24bppBGR;
        frame_encode.SetPixelFormat(&mut pixel_format)?;
        frame_encode.SetSize(target_w, target_h)?;

        let converter_source: IWICBitmapSource = converter.cast()?;
        frame_encode.WriteSource(&converter_source, std::ptr::null())?;
        frame_encode.Commit()?;
        encoder.Commit()?;

        info!("Successfully generated thumbnail for {:?} at {:?}", src_path, dest_path);
        Ok(())
    }
}

fn to_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}
