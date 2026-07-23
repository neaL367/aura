use ash::vk;
use aura_core::{monitor::MonitorId, wallpaper::FitMode};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;

use crate::{
    context::VulkanContext, error::VulkanError, frame::FrameSync, pipeline::GraphicsPipeline,
    staging::StagingUploader, surface::Surface, swapchain::Swapchain, texture::GpuTexture,
    transform::calculate_uv_transform,
};

use std::sync::Arc;

pub struct MonitorRenderer {
    pub monitor_id: MonitorId,
    pub context: Arc<VulkanContext>,
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

        let descriptor_pool = create_descriptor_pool(context, 1)?;
        let descriptor_set = allocate_descriptor_set(context, &pipeline, descriptor_pool)?;

        let framebuffers = create_framebuffers(context, &pipeline, &swapchain)?;

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

    pub fn set_virtual_geometry(
        &mut self,
        mon_x: i32,
        mon_y: i32,
        total_w: u32,
        total_h: u32,
    ) {
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
        self.frame_sync.wait_and_reset(&context.device)?;

        let (image_index, _) = unsafe {
            self.swapchain
                .swapchain_loader
                .acquire_next_image(
                    self.swapchain.swapchain,
                    u64::MAX,
                    self.frame_sync.image_available_semaphore,
                    vk::Fence::null(),
                )
                .map_err(|e| {
                    if e == vk::Result::ERROR_OUT_OF_DATE_KHR {
                        VulkanError::SwapchainOutOfDate
                    } else {
                        VulkanError::Swapchain(e.to_string())
                    }
                })?
        };

        let framebuffer = self.framebuffers[image_index as usize];

        unsafe {
            context
                .device
                .reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
                .ok();
        }

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            context
                .device
                .begin_command_buffer(self.command_buffer, &begin_info)
                .ok();
        }

        let clear_value = vk::ClearValue {
            color: vk::ClearColorValue {
                float32: clear_color,
            },
        };

        let render_pass_begin = vk::RenderPassBeginInfo::default()
            .render_pass(self.pipeline.render_pass)
            .framebuffer(framebuffer)
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.swapchain.extent,
            })
            .clear_values(std::slice::from_ref(&clear_value));

        unsafe {
            context.device.cmd_begin_render_pass(
                self.command_buffer,
                &render_pass_begin,
                vk::SubpassContents::INLINE,
            );
        }

        let viewport = vk::Viewport::default()
            .x(0.0)
            .y(0.0)
            .width(self.swapchain.extent.width as f32)
            .height(self.swapchain.extent.height as f32)
            .min_depth(0.0)
            .max_depth(1.0);

        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: self.swapchain.extent,
        };

        unsafe {
            context.device.cmd_set_viewport(
                self.command_buffer,
                0,
                std::slice::from_ref(&viewport),
            );
            context
                .device
                .cmd_set_scissor(self.command_buffer, 0, std::slice::from_ref(&scissor));
            context.device.cmd_bind_pipeline(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline.pipeline,
            );
        }

        if let Some(ref texture) = self.active_texture {
            let pc = if self.active_fit_mode == FitMode::Span {
                crate::transform::calculate_span_uv_transform(
                    texture.width,
                    texture.height,
                    self.virtual_x,
                    self.virtual_y,
                    self.swapchain.extent.width,
                    self.swapchain.extent.height,
                    self.virtual_desktop_width,
                    self.virtual_desktop_height,
                )
            } else {
                calculate_uv_transform(
                    self.active_fit_mode,
                    texture.width,
                    texture.height,
                    self.swapchain.extent.width,
                    self.swapchain.extent.height,
                )
            };
            let mut pc_bytes = [0u8; 16];
            pc_bytes[0..4].copy_from_slice(&pc.uv_scale[0].to_ne_bytes());
            pc_bytes[4..8].copy_from_slice(&pc.uv_scale[1].to_ne_bytes());
            pc_bytes[8..12].copy_from_slice(&pc.uv_offset[0].to_ne_bytes());
            pc_bytes[12..16].copy_from_slice(&pc.uv_offset[1].to_ne_bytes());

            unsafe {
                context.device.cmd_push_constants(
                    self.command_buffer,
                    self.pipeline.pipeline_layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    &pc_bytes,
                );
                context.device.cmd_bind_descriptor_sets(
                    self.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline.pipeline_layout,
                    0,
                    std::slice::from_ref(&self.descriptor_set),
                    &[],
                );
            }
        }

        unsafe {
            context.device.cmd_draw(self.command_buffer, 6, 1, 0, 0);
        }

        unsafe {
            context.device.cmd_end_render_pass(self.command_buffer);
            context.device.end_command_buffer(self.command_buffer).ok();
        }

        let wait_semaphores = [self.frame_sync.image_available_semaphore];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let signal_semaphores = [self.frame_sync.render_finished_semaphore];

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(std::slice::from_ref(&self.command_buffer))
            .signal_semaphores(&signal_semaphores);

        let _lock = context.queue_lock();

        unsafe {
            context
                .device
                .queue_submit(
                    context.graphics_queue,
                    &[submit_info],
                    self.frame_sync.in_flight_fence,
                )
                .map_err(|e| VulkanError::Render(e.to_string()))?;
        }

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&signal_semaphores)
            .swapchains(std::slice::from_ref(&self.swapchain.swapchain))
            .image_indices(std::slice::from_ref(&image_index));

        unsafe {
            let suboptimal = self
                .swapchain
                .swapchain_loader
                .queue_present(context.graphics_queue, &present_info)
                .map_err(|e| {
                    if e == vk::Result::ERROR_OUT_OF_DATE_KHR {
                        VulkanError::SwapchainOutOfDate
                    } else {
                        VulkanError::Swapchain(e.to_string())
                    }
                })?;
            if suboptimal {
                Err(VulkanError::SwapchainOutOfDate)
            } else {
                Ok(())
            }
        }
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
            update_descriptor_set(
                context,
                self.descriptor_set,
                texture,
                self.active_fit_mode,
                self.repeat_sampler,
            );
        }

        Ok(())
    }

    /// Update active fit mode and update descriptor set sampler if needed.
    pub fn set_fit_mode(&mut self, fit_mode: FitMode, context: &VulkanContext) {
        self.active_fit_mode = fit_mode;
        if let Some(ref texture) = self.active_texture {
            update_descriptor_set(
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
            destroy_framebuffers(&context.device, &mut self.framebuffers);
            self.swapchain.destroy(&context.device);
        }
        self.swapchain = new_swapchain;
        self.framebuffers = create_framebuffers(context, &self.pipeline, &self.swapchain)?;

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

            destroy_framebuffers(&context.device, &mut self.framebuffers);
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

fn create_descriptor_pool(
    context: &VulkanContext,
    max_sets: u32,
) -> Result<vk::DescriptorPool, VulkanError> {
    let pool_sizes = [vk::DescriptorPoolSize {
        ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        descriptor_count: max_sets,
    }];

    let pool_info = vk::DescriptorPoolCreateInfo::default()
        .pool_sizes(&pool_sizes)
        .max_sets(max_sets);

    unsafe {
        context
            .device
            .create_descriptor_pool(&pool_info, None)
            .map_err(|e| VulkanError::Render(e.to_string()))
    }
}

fn allocate_descriptor_set(
    context: &VulkanContext,
    pipeline: &GraphicsPipeline,
    pool: vk::DescriptorPool,
) -> Result<vk::DescriptorSet, VulkanError> {
    let layouts = [pipeline.descriptor_set_layout];
    let alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(pool)
        .set_layouts(&layouts);

    unsafe {
        context
            .device
            .allocate_descriptor_sets(&alloc_info)
            .map_err(|e| VulkanError::Render(e.to_string()))
            .map(|sets| sets[0])
    }
}

fn update_descriptor_set(
    context: &VulkanContext,
    descriptor_set: vk::DescriptorSet,
    texture: &GpuTexture,
    fit_mode: FitMode,
    repeat_sampler: vk::Sampler,
) {
    let sampler = if fit_mode == FitMode::Tile {
        repeat_sampler
    } else {
        texture.sampler
    };

    let image_info = vk::DescriptorImageInfo::default()
        .sampler(sampler)
        .image_view(texture.view)
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

    let write = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set)
        .dst_binding(0)
        .descriptor_count(1)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(std::slice::from_ref(&image_info));

    unsafe {
        context.device.update_descriptor_sets(&[write], &[]);
    }
}

fn create_framebuffers(
    context: &VulkanContext,
    pipeline: &GraphicsPipeline,
    swapchain: &Swapchain,
) -> Result<Vec<vk::Framebuffer>, VulkanError> {
    swapchain
        .image_views
        .iter()
        .map(|&view| {
            let attachments = [view];
            let fb_info = vk::FramebufferCreateInfo::default()
                .render_pass(pipeline.render_pass)
                .attachments(&attachments)
                .width(swapchain.extent.width)
                .height(swapchain.extent.height)
                .layers(1);

            unsafe {
                context
                    .device
                    .create_framebuffer(&fb_info, None)
                    .map_err(|e| VulkanError::Render(e.to_string()))
            }
        })
        .collect()
}

unsafe fn destroy_framebuffers(device: &ash::Device, framebuffers: &mut Vec<vk::Framebuffer>) {
    unsafe {
        for fb in framebuffers.drain(..) {
            device.destroy_framebuffer(fb, None);
        }
    }
}
