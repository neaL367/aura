use ash::vk;
use aura_core::monitor::MonitorId;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;

use crate::{
    context::VulkanContext, error::VulkanError, frame::FrameSync, pipeline::GraphicsPipeline,
    surface::Surface, swapchain::Swapchain, texture::GpuTexture,
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
    pub upload_command_buffer: vk::CommandBuffer,
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptor_set: vk::DescriptorSet,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub active_texture: Option<GpuTexture>,
    // Persistent staging buffer for CPU→GPU texture uploads.
    pub upload_staging_buffer: Option<vk::Buffer>,
    pub upload_staging_allocation: Option<gpu_allocator::vulkan::Allocation>,
    pub upload_staging_size: u64,
    pub upload_fence: vk::Fence,
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

        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        let upload_fence = unsafe {
            context
                .device
                .create_fence(&fence_info, None)
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?
        };

        Ok(Self {
            monitor_id,
            context: context.clone(),
            surface,
            swapchain,
            pipeline,
            frame_sync,
            command_pool,
            command_buffer,
            upload_command_buffer,
            descriptor_pool,
            descriptor_set,
            framebuffers,
            active_texture: None,
            upload_staging_buffer: None,
            upload_staging_allocation: None,
            upload_staging_size: 0,
            upload_fence,
        })
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

        if self.active_texture.is_some() {
            unsafe {
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

    /// Upload new RGBA pixel data to the active wallpaper texture using a
    /// persistent staging buffer + fence, without queue_wait_idle.
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

        // 2. Wait for previous upload to complete (protects staging buffer reuse).
        unsafe {
            context
                .device
                .wait_for_fences(std::slice::from_ref(&self.upload_fence), true, u64::MAX)
                .map_err(|e| VulkanError::Upload(e.to_string()))?;
            context
                .device
                .reset_fences(std::slice::from_ref(&self.upload_fence))
                .map_err(|e| VulkanError::Upload(e.to_string()))?;
        }

        // 3. Create / resize staging buffer if needed.
        if self.upload_staging_size < buffer_size {
            if let Some(buf) = self.upload_staging_buffer.take() {
                unsafe { context.device.destroy_buffer(buf, None) };
            }
            if let Some(alloc) = self.upload_staging_allocation.take() {
                let _ = context.allocator.lock().unwrap().free(alloc);
            }

            let buffer_info = vk::BufferCreateInfo::default()
                .size(buffer_size)
                .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let new_buffer = unsafe {
                context
                    .device
                    .create_buffer(&buffer_info, None)
                    .map_err(|e| VulkanError::Upload(e.to_string()))?
            };

            let reqs = unsafe { context.device.get_buffer_memory_requirements(new_buffer) };

            let new_alloc = context
                .allocator
                .lock()
                .unwrap()
                .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                    name: "Staging Buffer",
                    requirements: reqs,
                    location: gpu_allocator::MemoryLocation::CpuToGpu,
                    linear: true,
                    allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
                })
                .map_err(|e| VulkanError::Allocation(e.to_string()))?;

            unsafe {
                context
                    .device
                    .bind_buffer_memory(new_buffer, new_alloc.memory(), new_alloc.offset())
                    .map_err(|e| VulkanError::Upload(e.to_string()))?;
            }

            self.upload_staging_buffer = Some(new_buffer);
            self.upload_staging_allocation = Some(new_alloc);
            self.upload_staging_size = buffer_size;
        }

        // 4. Map and copy pixel data into staging buffer.
        if let Some(ref alloc) = self.upload_staging_allocation {
            if let Some(mapped_ptr) = alloc.mapped_ptr() {
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        pixels.as_ptr(),
                        mapped_ptr.as_ptr() as *mut u8,
                        pixels.len(),
                    );
                }
            } else {
                unsafe {
                    let ptr = context
                        .device
                        .map_memory(
                            alloc.memory(),
                            alloc.offset(),
                            buffer_size,
                            vk::MemoryMapFlags::empty(),
                        )
                        .map_err(|e| VulkanError::Upload(e.to_string()))?;
                    std::ptr::copy_nonoverlapping(pixels.as_ptr(), ptr as *mut u8, pixels.len());
                    context.device.unmap_memory(alloc.memory());
                }
            }

            // Flush mapped memory range to ensure CPU writes are visible to GPU DMA transfer reads
            unsafe {
                let range = vk::MappedMemoryRange::default()
                    .memory(alloc.memory())
                    .offset(alloc.offset())
                    .size(vk::WHOLE_SIZE);
                let _ = context.device.flush_mapped_memory_ranges(&[range]);
            }
        }

        // 5. Record upload command buffer.
        let texture = self
            .active_texture
            .as_ref()
            .ok_or_else(|| VulkanError::Upload("no active texture".into()))?;

        unsafe {
            context
                .device
                .reset_command_buffer(
                    self.upload_command_buffer,
                    vk::CommandBufferResetFlags::empty(),
                )
                .ok();
        }

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        let current_layout = texture.layout;
        let (src_stage, src_access) = if current_layout == vk::ImageLayout::UNDEFINED {
            (
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::AccessFlags::empty(),
            )
        } else {
            (
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::AccessFlags::SHADER_READ,
            )
        };

        unsafe {
            context
                .device
                .begin_command_buffer(self.upload_command_buffer, &begin_info)
                .map_err(|e| VulkanError::Upload(e.to_string()))?;

            // Transition image: current_layout -> TRANSFER_DST_OPTIMAL
            let barrier_to_transfer = vk::ImageMemoryBarrier::default()
                .old_layout(current_layout)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(texture.image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .src_access_mask(src_access)
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);

            context.device.cmd_pipeline_barrier(
                self.upload_command_buffer,
                src_stage,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier_to_transfer],
            );

            // Copy buffer to image
            let copy_region = vk::BufferImageCopy::default()
                .buffer_offset(0)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                .image_extent(vk::Extent3D {
                    width: texture.width,
                    height: texture.height,
                    depth: 1,
                });

            let staging_buf = self
                .upload_staging_buffer
                .ok_or_else(|| VulkanError::Upload("No staging buffer available".to_string()))?;

            context.device.cmd_copy_buffer_to_image(
                self.upload_command_buffer,
                staging_buf,
                texture.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[copy_region],
            );

            // Transition image: TRANSFER_DST_OPTIMAL -> SHADER_READ_ONLY_OPTIMAL
            let barrier_to_shader = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(texture.image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ);

            context.device.cmd_pipeline_barrier(
                self.upload_command_buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier_to_shader],
            );

            context
                .device
                .end_command_buffer(self.upload_command_buffer)
                .map_err(|e| VulkanError::Upload(e.to_string()))?;
        }

        if let Some(t) = self.active_texture.as_mut() {
            t.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        }

        // 6. Submit with upload_fence (no wait_idle).
        let submit_info = vk::SubmitInfo::default()
            .command_buffers(std::slice::from_ref(&self.upload_command_buffer));

        let _lock = context.queue_lock();

        unsafe {
            context
                .device
                .queue_submit(context.graphics_queue, &[submit_info], self.upload_fence)
                .map_err(|e| VulkanError::Upload(e.to_string()))?;
            context
                .device
                .wait_for_fences(std::slice::from_ref(&self.upload_fence), true, u64::MAX)
                .map_err(|e| VulkanError::Upload(e.to_string()))?;
        }

        // 7. Update descriptor set to point at the (now-populated) texture.
        if let Some(some_texture) = self.active_texture.as_ref() {
            update_descriptor_set(context, self.descriptor_set, some_texture);
        }

        Ok(())
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

            // Destroy persistent staging buffer.
            if let Some(buf) = self.upload_staging_buffer.take() {
                context.device.destroy_buffer(buf, None);
            }
            if let Some(alloc) = self.upload_staging_allocation.take() {
                let _ = context.allocator.lock().unwrap().free(alloc);
            }
            if self.upload_fence != vk::Fence::null() {
                context.device.destroy_fence(self.upload_fence, None);
                self.upload_fence = vk::Fence::null();
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
) {
    let image_info = vk::DescriptorImageInfo::default()
        .sampler(texture.sampler)
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
