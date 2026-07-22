use ash::vk;
use gpu_allocator::vulkan::AllocationCreateDesc;

use crate::{context::VulkanContext, error::VulkanError, texture::GpuTexture};

/// Helper for uploading CPU-decoded frame buffers to GPU textures via staging memory.
pub struct TextureUploader;

impl TextureUploader {
    /// Upload CPU-decoded RGBA pixel data to a `GpuTexture`.
    pub fn upload_pixels(
        context: &mut VulkanContext,
        command_pool: vk::CommandPool,
        texture: &mut GpuTexture,
        pixels: &[u8],
    ) -> Result<(), VulkanError> {
        let buffer_size = pixels.len() as u64;
        if buffer_size == 0 {
            return Ok(());
        }

        // 1. Create staging buffer
        let buffer_info = vk::BufferCreateInfo::default()
            .size(buffer_size)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let staging_buffer = unsafe {
            context
                .device
                .create_buffer(&buffer_info, None)
                .map_err(|e| VulkanError::Upload(e.to_string()))?
        };

        let reqs = unsafe {
            context
                .device
                .get_buffer_memory_requirements(staging_buffer)
        };

        let staging_allocation = context
            .allocator
            .lock()
            .unwrap()
            .allocate(&AllocationCreateDesc {
                name: "Texture Staging Buffer",
                requirements: reqs,
                location: gpu_allocator::MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .map_err(|e| VulkanError::Allocation(e.to_string()))?;

        unsafe {
            context
                .device
                .bind_buffer_memory(
                    staging_buffer,
                    staging_allocation.memory(),
                    staging_allocation.offset(),
                )
                .map_err(|e| VulkanError::Upload(e.to_string()))?;

            // Copy pixel data into mapped staging memory
            if let Some(mapped_ptr) = staging_allocation.mapped_ptr() {
                std::ptr::copy_nonoverlapping(
                    pixels.as_ptr(),
                    mapped_ptr.as_ptr() as *mut u8,
                    pixels.len(),
                );
            }
        }

        // 2. Allocate command buffer for layout transitions and transfer
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let command_buffer = unsafe {
            context
                .device
                .allocate_command_buffers(&alloc_info)
                .map_err(|e| VulkanError::Upload(e.to_string()))?[0]
        };

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
                .begin_command_buffer(command_buffer, &begin_info)
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
                command_buffer,
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

            context.device.cmd_copy_buffer_to_image(
                command_buffer,
                staging_buffer,
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
                command_buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier_to_shader],
            );

            context
                .device
                .end_command_buffer(command_buffer)
                .map_err(|e| VulkanError::Upload(e.to_string()))?;
        }

        texture.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        // Submit and wait synchronously for transfer execution
        let submit_info =
            vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&command_buffer));
        unsafe {
            context
                .device
                .queue_submit(context.graphics_queue, &[submit_info], vk::Fence::null())
                .map_err(|e| VulkanError::Upload(e.to_string()))?;
            context
                .device
                .queue_wait_idle(context.graphics_queue)
                .map_err(|e| VulkanError::Upload(e.to_string()))?;

            // Cleanup command buffer and staging allocation
            context
                .device
                .free_command_buffers(command_pool, &[command_buffer]);
            context.device.destroy_buffer(staging_buffer, None);
            let _ = context.allocator.lock().unwrap().free(staging_allocation);
        }

        Ok(())
    }
}
