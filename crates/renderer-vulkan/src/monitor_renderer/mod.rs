pub mod frame_pass;
pub mod resources;

use ash::vk;
use aura_core::{monitor::MonitorId, wallpaper::FitMode};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;

use crate::{
    context::VulkanContext, error::VulkanError, frame::FrameSync, pipeline::GraphicsPipeline,
    staging::StagingUploader, surface::Surface, swapchain::Swapchain, texture::GpuTexture,
};

use std::sync::Arc;

pub struct MonitorRenderer {
    pub monitor_id: MonitorId,
    pub surface: Surface,
    pub swapchain: Swapchain,
    pub pipeline: GraphicsPipeline,
    pub frame_sync: FrameSync,
    pub command_pool: vk::CommandPool,
    pub command_buffer: vk::CommandBuffer,
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptor_set: vk::DescriptorSet,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub active_texture: Option<GpuTexture>,
    pub active_fit_mode: FitMode,
    pub repeat_sampler: vk::Sampler,
    pub uploader: StagingUploader,
    pub virtual_x: i32,
    pub virtual_y: i32,
    pub virtual_desktop_width: u32,
    pub virtual_desktop_height: u32,
    pub context: Arc<VulkanContext>,
}

impl MonitorRenderer {
    #[cfg(target_os = "windows")]
    pub fn create_win32(
        context: &Arc<VulkanContext>,
        monitor_id: MonitorId,
        hwnd: HWND,
        width: u32,
        height: u32,
    ) -> Result<Self, VulkanError> {
        let surface = Surface::create_win32(context, hwnd)?;

        if !surface.get_support(context.physical_device, context.graphics_queue_family)? {
            return Err(VulkanError::Surface(
                "Graphics queue family does not support presentation on this surface".into(),
            ));
        }

        let swapchain =
            Swapchain::create(context, &surface, width, height, vk::SwapchainKHR::null())?;
        let frame_sync = FrameSync::new(context)?;
        let pipeline = GraphicsPipeline::create(context, swapchain.format)?;

        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(context.graphics_queue_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = unsafe {
            context
                .device
                .create_command_pool(&pool_info, None)
                .map_err(|e| VulkanError::Render(e.to_string()))?
        };

        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(2);
        let bufs = unsafe {
            context
                .device
                .allocate_command_buffers(&alloc_info)
                .map_err(|e| VulkanError::Render(e.to_string()))?
        };
        let command_buffer = bufs[0];
        let upload_command_buffer = bufs[1];

        let descriptor_pool = resources::create_descriptor_pool(context, 1)?;
        let descriptor_set =
            resources::allocate_descriptor_set(context, &pipeline, descriptor_pool)?;

        let framebuffers = resources::create_framebuffers(context, &pipeline, &swapchain)?;

        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT)
            .max_anisotropy(1.0);

        let repeat_sampler = unsafe {
            context
                .device
                .create_sampler(&sampler_info, None)
                .map_err(|e| VulkanError::Texture(e.to_string()))?
        };

        let uploader = StagingUploader::create(context, upload_command_buffer)?;

        Ok(Self {
            monitor_id,
            context: context.clone(),
            surface,
            swapchain,
            pipeline,
            frame_sync,
            command_pool,
            command_buffer,
            descriptor_pool,
            descriptor_set,
            framebuffers,
            active_texture: None,
            active_fit_mode: FitMode::Fill,
            repeat_sampler,
            uploader,
            virtual_x: 0,
            virtual_y: 0,
            virtual_desktop_width: width,
            virtual_desktop_height: height,
        })
    }

    pub fn set_virtual_geometry(&mut self, mon_x: i32, mon_y: i32, total_w: u32, total_h: u32) {
        self.virtual_x = mon_x;
        self.virtual_y = mon_y;
        self.virtual_desktop_width = total_w;
        self.virtual_desktop_height = total_h;
    }

    /// Acquire, draw, and present one frame.
    pub fn frame(
        &mut self,
        context: &VulkanContext,
        clear_color: [f32; 4],
    ) -> Result<(), VulkanError> {
        frame_pass::execute_frame(self, context, clear_color)
    }

    pub fn set_wallpaper_pixels(
        &mut self,
        context: &VulkanContext,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<(), VulkanError> {
        let buffer_size = pixels.len() as u64;
        if buffer_size == 0 {
            return Ok(());
        }

        // 1. Ensure texture exists with correct dimensions.
        let needs_recreate = match &self.active_texture {
            Some(t) => t.width != width || t.height != height,
            None => true,
        };

        if needs_recreate {
            unsafe {
                context.device.device_wait_idle().ok();
            }
            if let Some(mut old_t) = self.active_texture.take() {
                unsafe { old_t.destroy(context) };
            }
            let new_t = GpuTexture::create_2d(context, width, height, vk::Format::R8G8B8A8_UNORM)?;
            self.active_texture = Some(new_t);
        }

        if let Some(ref mut texture) = self.active_texture {
            self.uploader.upload_pixels(context, texture, pixels)?;
            resources::update_descriptor_set(
                context,
                self.descriptor_set,
                texture,
                self.active_fit_mode,
                self.repeat_sampler,
            );
        }

        Ok(())
    }

    /// Free CPU-to-GPU staging buffer allocation to reclaim host RAM.
    pub fn trim_staging(&mut self, context: &VulkanContext) {
        self.uploader.trim(context);
    }

    /// Update active fit mode and update descriptor set sampler if needed.
    pub fn set_fit_mode(&mut self, fit_mode: FitMode, context: &VulkanContext) {
        self.active_fit_mode = fit_mode;
        if let Some(ref texture) = self.active_texture {
            resources::update_descriptor_set(
                context,
                self.descriptor_set,
                texture,
                fit_mode,
                self.repeat_sampler,
            );
        }
    }

    /// Recreate the swapchain and framebuffers after resolution change.
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
            resources::destroy_framebuffers(&context.device, &mut self.framebuffers);
            self.swapchain.destroy(&context.device);
        }
        self.swapchain = new_swapchain;
        self.framebuffers =
            resources::create_framebuffers(context, &self.pipeline, &self.swapchain)?;

        Ok(())
    }

    /// Clean up all GPU resources.
    ///
    /// # Safety
    /// Must be called when the GPU is idle before destroying `VulkanContext`.
    pub unsafe fn destroy(&mut self, context: &VulkanContext) {
        unsafe {
            if self.command_pool == vk::CommandPool::null() {
                return;
            }
            context.device.device_wait_idle().ok();

            if let Some(mut texture) = self.active_texture.take() {
                texture.destroy(context);
            }

            self.uploader.destroy(context);

            if self.repeat_sampler != vk::Sampler::null() {
                context.device.destroy_sampler(self.repeat_sampler, None);
                self.repeat_sampler = vk::Sampler::null();
            }

            resources::destroy_framebuffers(&context.device, &mut self.framebuffers);
            if self.descriptor_pool != vk::DescriptorPool::null() {
                context
                    .device
                    .destroy_descriptor_pool(self.descriptor_pool, None);
                self.descriptor_pool = vk::DescriptorPool::null();
            }
            if self.command_pool != vk::CommandPool::null() {
                context.device.destroy_command_pool(self.command_pool, None);
                self.command_pool = vk::CommandPool::null();
            }
            self.pipeline.destroy(&context.device);
            self.frame_sync.destroy(&context.device);
            self.swapchain.destroy(&context.device);
        }
    }
}

impl Drop for MonitorRenderer {
    fn drop(&mut self) {
        let context = self.context.clone();
        unsafe {
            self.destroy(&context);
        }
    }
}
