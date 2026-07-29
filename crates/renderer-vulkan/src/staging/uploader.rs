use ash::vk;

use crate::{context::VulkanContext, error::VulkanError, texture::GpuTexture};

pub fn record_and_submit_upload(
    context: &VulkanContext,
    cmd_buffer: vk::CommandBuffer,
    staging_buffer: vk::Buffer,
    texture: &mut GpuTexture,
    upload_fence: vk::Fence,
) -> Result<(), VulkanError> {
    unsafe {
        context
            .device
            .reset_command_buffer(cmd_buffer, vk::CommandBufferResetFlags::empty())
            .ok();
    }

    let begin_info =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

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
            .begin_command_buffer(cmd_buffer, &begin_info)
            .map_err(|e| VulkanError::Upload(e.to_string()))?;

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
            cmd_buffer,
            src_stage,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier_to_transfer],
        );

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
            cmd_buffer,
            staging_buffer,
            texture.image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[copy_region],
        );

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
            cmd_buffer,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier_to_shader],
        );

        context
            .device
            .end_command_buffer(cmd_buffer)
            .map_err(|e| VulkanError::Upload(e.to_string()))?;
    }

    texture.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

    let submit_info = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&cmd_buffer));

    let _lock = context.queue_lock();

    unsafe {
        context
            .device
            .queue_submit(context.graphics_queue, &[submit_info], upload_fence)
            .map_err(|e| VulkanError::Upload(e.to_string()))?;

        context
            .device
            .wait_for_fences(std::slice::from_ref(&upload_fence), true, u64::MAX)
            .map_err(|e| VulkanError::Upload(e.to_string()))?;
    }

    Ok(())
}
