use ash::vk;
use aura_core::{monitor::MonitorId, wallpaper::FitMode};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;

use std::sync::Arc;

use super::{MonitorRenderer, resources};
use crate::{
    context::VulkanContext, error::VulkanError, frame::FrameSync, pipeline::GraphicsPipeline,
    staging::StagingUploader, surface::Surface, swapchain::Swapchain,
};

impl MonitorRenderer {
    pub fn create_win32(
        context: &Arc<VulkanContext>,
        monitor_id: MonitorId,
        hwnd: HWND,
        width: u32,
        height: u32,
    ) -> Result<Self, VulkanError> {
        let surface = Surface::create_win32(context, hwnd)?;

        if !surface.support(context.physical_device, context.graphics_queue_family)? {
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

        let border_sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
            .border_color(vk::BorderColor::FLOAT_TRANSPARENT_BLACK)
            .max_anisotropy(1.0);

        let border_sampler = unsafe {
            context
                .device
                .create_sampler(&border_sampler_info, None)
                .map_err(|e| VulkanError::Texture(e.to_string()))?
        };

        let video_sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .max_anisotropy(1.0);

        let video_sampler = unsafe {
            context
                .device
                .create_sampler(&video_sampler_info, None)
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
            border_sampler,
            video_sampler,
            active_video_frame: None,
            gfx_timeline_value: 0,
            current_gfx_timeline: vk::Semaphore::null(),
            uploader,
            virtual_x: 0,
            virtual_y: 0,
            virtual_desktop_width: width,
            virtual_desktop_height: height,
        })
    }
}
