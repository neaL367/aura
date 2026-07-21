use ash::vk;
use aura_core::monitor::MonitorId;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;

use crate::{
    context::VulkanContext, error::VulkanError, frame::FrameSync, surface::Surface,
    swapchain::Swapchain, texture::GpuTexture, upload::TextureUploader,
};

/// Per-monitor Vulkan renderer.
///
/// Owns the HWND `Surface`, vsync-paced `Swapchain`, synchronization primitives (`FrameSync`),
/// command pool, and active wallpaper `GpuTexture`.
pub struct MonitorRenderer {
    pub monitor_id: MonitorId,
    pub surface: Surface,
    pub swapchain: Swapchain,
    pub frame_sync: FrameSync,
    pub command_pool: vk::CommandPool,
    pub active_texture: Option<GpuTexture>,
}

impl MonitorRenderer {
    /// Create a new `MonitorRenderer` attached to a host window `HWND`.
    #[cfg(target_os = "windows")]
    pub fn create_win32(
        context: &mut VulkanContext,
        monitor_id: MonitorId,
        hwnd: HWND,
        width: u32,
        height: u32,
    ) -> Result<Self, VulkanError> {
        let surface = Surface::create_win32(context, hwnd)?;

        // Verify queue support for this surface
        if !surface.get_support(context.physical_device, context.graphics_queue_family)? {
            return Err(VulkanError::Surface(
                "Graphics queue family does not support presentation on this surface".into(),
            ));
        }

        let swapchain =
            Swapchain::create(context, &surface, width, height, vk::SwapchainKHR::null())?;
        let frame_sync = FrameSync::new(context)?;

        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(context.graphics_queue_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

        let command_pool = unsafe {
            context
                .device
                .create_command_pool(&pool_info, None)
                .map_err(|e| VulkanError::Render(e.to_string()))?
        };

        Ok(Self {
            monitor_id,
            surface,
            swapchain,
            frame_sync,
            command_pool,
            active_texture: None,
        })
    }

    /// Upload new RGBA pixel data to the active wallpaper texture.
    ///
    /// Reuses the existing `GpuTexture` if dimensions match; otherwise recreates the allocation.
    pub fn set_wallpaper_pixels(
        &mut self,
        context: &mut VulkanContext,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<(), VulkanError> {
        let needs_recreate = match &self.active_texture {
            Some(t) => t.width != width || t.height != height,
            None => true,
        };

        if needs_recreate {
            if let Some(mut old_t) = self.active_texture.take() {
                unsafe { old_t.destroy(context) };
            }
            let new_t = GpuTexture::create_2d(context, width, height, vk::Format::R8G8B8A8_UNORM)?;
            self.active_texture = Some(new_t);
        }

        if let Some(texture) = &self.active_texture {
            TextureUploader::upload_pixels(context, self.command_pool, texture, pixels)?;
        }

        Ok(())
    }

    /// Recreate the swapchain after display resolution or DPI change.
    pub fn resize(
        &mut self,
        context: &VulkanContext,
        width: u32,
        height: u32,
    ) -> Result<(), VulkanError> {
        unsafe {
            context.device.device_wait_idle().ok();
        }

        let old_swapchain = self.swapchain.swapchain;
        let new_swapchain =
            Swapchain::create(context, &self.surface, width, height, old_swapchain)?;

        unsafe {
            self.swapchain.destroy(&context.device);
        }
        self.swapchain = new_swapchain;

        Ok(())
    }

    /// Clean up all GPU resources.
    ///
    /// # Safety
    /// Must be called when the GPU is idle before destroying `VulkanContext`.
    pub unsafe fn destroy(&mut self, context: &mut VulkanContext) {
        unsafe {
            context.device.device_wait_idle().ok();

            if let Some(mut texture) = self.active_texture.take() {
                texture.destroy(context);
            }

            context.device.destroy_command_pool(self.command_pool, None);
            self.frame_sync.destroy(&context.device);
            self.swapchain.destroy(&context.device);
        }
    }
}
