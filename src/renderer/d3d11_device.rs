use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11_CREATE_DEVICE_FLAG,
};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL,
    D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0,
    D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_10_0,
};
use tracing::info;
use crate::utils::error::{AppError, Result};

pub struct D3d11Device {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
}

impl D3d11Device {
    /// Creates a shared D3D11 hardware device with BGRA context support.
    pub fn new() -> Result<Self> {
        let feature_levels = [
            D3D_FEATURE_LEVEL_11_1,
            D3D_FEATURE_LEVEL_11_0,
            D3D_FEATURE_LEVEL_10_1,
            D3D_FEATURE_LEVEL_10_0,
        ];

        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        let mut feature_level = D3D_FEATURE_LEVEL::default();

        let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;

        unsafe {
            D3D11CreateDevice(
                None, // Default adapter
                D3D_DRIVER_TYPE_HARDWARE,
                windows::Win32::Foundation::HMODULE::default(), // No software rasterizer
                D3D11_CREATE_DEVICE_FLAG(flags.0),
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut feature_level),
                Some(&mut context),
            )?;
        }

        let device = device.ok_or_else(|| AppError::Renderer("Failed to obtain D3D11 device".to_string()))?;
        let context = context.ok_or_else(|| AppError::Renderer("Failed to obtain D3D11 device context".to_string()))?;

        info!("D3D11 Device initialized. Feature level: {:?}", feature_level);

        Ok(Self { device, context })
    }
}
