use windows::core::Interface;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIAdapter, IDXGIFactory2, IDXGISwapChain1,
    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, DXGI_PRESENT, DXGI_SCALING_STRETCH,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC, DXGI_ALPHA_MODE_UNSPECIFIED,
    DXGI_FORMAT_UNKNOWN,
};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11RenderTargetView, ID3D11Texture2D,
};
use tracing::{info, debug};
use crate::utils::error::{AppError, Result};

pub struct Swapchain {
    pub swapchain: IDXGISwapChain1,
    pub rtv: Option<ID3D11RenderTargetView>,
}

impl Swapchain {
    /// Creates a new DXGI Swapchain and its corresponding Render Target View (RTV) for the given window.
    pub fn create(device: &ID3D11Device, hwnd: HWND, width: u32, height: u32) -> Result<Self> {
        // Query IDXGIDevice interface from D3D11 device
        let dxgi_device: IDXGIDevice = device.cast()?;

        // Get IDXGIAdapter
        let adapter: IDXGIAdapter = unsafe { dxgi_device.GetAdapter()? };

        // Get the DXGI factory
        let factory: IDXGIFactory2 = unsafe { adapter.GetParent()? };

        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            Stereo: false.into(),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2, // Double buffered
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_UNSPECIFIED,
            Flags: 0,
        };

        let swapchain = unsafe {
            factory.CreateSwapChainForHwnd(
                device,
                hwnd,
                &desc,
                None, // Fullscreen description
                None, // Restrict to output
            )?
        };

        let mut s = Self {
            swapchain,
            rtv: None,
        };

        // Create initial render target view
        s.create_rtv(device)?;

        info!("Created DXGI swapchain ({}x{}) for HWND: {:?}", width, height, hwnd);
        Ok(s)
    }

    /// Internal helper to create the Render Target View (RTV) from the back buffer.
    fn create_rtv(&mut self, device: &ID3D11Device) -> Result<()> {
        let back_buffer: ID3D11Texture2D = unsafe { self.swapchain.GetBuffer(0)? };
        let mut rtv = None;
        unsafe {
            device.CreateRenderTargetView(&back_buffer, None, Some(&mut rtv))?;
        }
        let rtv = rtv.ok_or_else(|| {
            AppError::Renderer("Failed to create Render Target View".to_string())
        })?;
        self.rtv = Some(rtv);
        Ok(())
    }

    /// Resizes the swapchain buffers to match a window size change.
    /// Safely releases the RTV first so D3D doesn't fail the operation.
    pub fn resize(&mut self, device: &ID3D11Device, width: u32, height: u32) -> Result<()> {
        debug!("Resizing swapchain buffers to {}x{}", width, height);

        // Crucial: Release references to the backbuffer before resizing
        self.rtv = None;

        unsafe {
            self.swapchain.ResizeBuffers(
                0, // Preserve buffer count
                width,
                height,
                DXGI_FORMAT_UNKNOWN, // Preserve format
                windows::Win32::Graphics::Dxgi::DXGI_SWAP_CHAIN_FLAG(0),
            )?;
        }

        // Recreate the RTV on the new backbuffer
        self.create_rtv(device)?;
        Ok(())
    }

    /// Presents the frame to the display, synchronized to the screen VSync.
    pub fn present(&self) -> Result<()> {
        unsafe {
            // Present with sync interval 1 (VSync)
            crate::utils::hresult::check(self.swapchain.Present(1, DXGI_PRESENT(0)))?;
        }
        Ok(())
    }
}
