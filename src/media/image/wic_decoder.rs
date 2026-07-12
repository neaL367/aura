use std::path::Path;
use windows::core::{PCWSTR, Interface};
use windows::Win32::Graphics::Imaging::{
    IWICImagingFactory, CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA,
    WICDecodeMetadataCacheOnDemand, WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom,
    IWICBitmapSource,
};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11Texture2D, D3D11_TEXTURE2D_DESC, D3D11_SUBRESOURCE_DATA,
    D3D11_USAGE_IMMUTABLE, D3D11_BIND_SHADER_RESOURCE,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use tracing::{info, warn};
use crate::utils::error::{AppError, Result};
use crate::media::image::webp_fallback;

/// Decodes an image file and uploads it to an immutable D3D11 GPU texture.
/// First attempts to use native Windows WIC. If WIC fails (e.g. WebP format on older Win10 builds),
/// falls back to the pure-Rust WebP decoder.
pub fn load_texture_from_file(device: &ID3D11Device, path: &Path) -> Result<ID3D11Texture2D> {
    let is_webp = path.extension()
        .map_or(false, |ext| ext.to_string_lossy().eq_ignore_ascii_case("webp"));

    let decode_result = if is_webp {
        match decode_with_wic(path) {
            Ok(res) => Ok(res),
            Err(e) => {
                warn!("WIC failed to decode WebP, falling back to Rust image crate. Error: {:?}", e);
                webp_fallback::decode_webp(path)
            }
        }
    } else {
        decode_with_wic(path)
    };

    let (width, height, pixels) = decode_result?;

    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_IMMUTABLE,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };

    let init_data = D3D11_SUBRESOURCE_DATA {
        pSysMem: pixels.as_ptr() as *const _,
        SysMemPitch: width * 4,
        SysMemSlicePitch: 0,
    };

    let mut texture: Option<ID3D11Texture2D> = None;
    unsafe {
        device.CreateTexture2D(&desc, Some(&init_data), Some(&mut texture))?;
    }

    let texture = texture.ok_or_else(|| {
        AppError::Renderer("D3D11 CreateTexture2D returned success but texture was null".to_string())
    })?;

    info!("Successfully loaded texture from {:?} ({}x{})", path, width, height);
    Ok(texture)
}

fn decode_with_wic(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let path_wide = to_wide(path);

    unsafe {
        let factory: IWICImagingFactory = windows::Win32::System::Com::CoCreateInstance(
            &CLSID_WICImagingFactory,
            None,
            windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
        )?;

        let decoder = factory.CreateDecoderFromFilename(
            PCWSTR(path_wide.as_ptr()),
            None,
            windows::Win32::Foundation::GENERIC_ACCESS_RIGHTS(0x80000000), // GENERIC_READ
            WICDecodeMetadataCacheOnDemand,
        )?;

        let frame = decoder.GetFrame(0)?;
        let converter = factory.CreateFormatConverter()?;

        let source: IWICBitmapSource = frame.cast()?;

        converter.Initialize(
            &source,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;

        let mut w = 0;
        let mut h = 0;
        converter.GetSize(&mut w, &mut h)?;

        let stride = w * 4;
        let mut pixels = vec![0u8; (stride * h) as usize];
        
        converter.CopyPixels(
            std::ptr::null(),
            stride,
            &mut pixels,
        )?;

        Ok((w, h, pixels))
    }
}

fn to_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}
