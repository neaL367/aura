use std::path::PathBuf;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D};
use crate::domain::fit_mode::FitMode;
use crate::renderer::swapchain::Swapchain;
use crate::renderer::texture::TextureRenderer;
use crate::media::image::wic_decoder;
use crate::utils::error::Result;

/// Orchestrates rendering a static image wallpaper on a specific HWND.
pub struct ImageWallpaper {
    swapchain: Swapchain,
    texture: ID3D11Texture2D,
    fit_mode: FitMode,
    width: u32,
    height: u32,
}

impl ImageWallpaper {
    /// Creates a swapchain for the window, decodes the image, uploads it, and prepares for rendering.
    pub fn new(
        device: &ID3D11Device,
        hwnd: HWND,
        width: u32,
        height: u32,
        path: PathBuf,
        fit_mode: FitMode,
    ) -> Result<Self> {
        let swapchain = Swapchain::create(device, hwnd, width, height)?;
        let texture = wic_decoder::load_texture_from_file(device, &path)?;

        Ok(Self {
            swapchain,
            texture,
            fit_mode,
            width,
            height,
        })
    }

    /// Renders the static image to the swapchain backbuffer and presents it to the screen.
    pub fn render(
        &self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        renderer: &TextureRenderer,
    ) -> Result<()> {
        if let Some(ref rtv) = self.swapchain.rtv {
            renderer.render(
                device,
                context,
                rtv,
                &self.texture,
                self.fit_mode,
                self.width,
                self.height,
            )?;
            self.swapchain.present()?;
        }
        Ok(())
    }

    /// Resizes the target viewport and swapchain buffers.
    pub fn resize(&mut self, device: &ID3D11Device, width: u32, height: u32) -> Result<()> {
        self.width = width;
        self.height = height;
        self.swapchain.resize(device, width, height)?;
        Ok(())
    }

    /// Replaces the active wallpaper texture and updates the fit mode.
    pub fn replace_texture(&mut self, texture: ID3D11Texture2D, fit_mode: FitMode) {
        self.texture = texture;
        self.fit_mode = fit_mode;
    }

    /// Reconfigures the active viewport fit mode layout.
    pub fn set_fit_mode(&mut self, fit_mode: FitMode) {
        self.fit_mode = fit_mode;
    }
}
