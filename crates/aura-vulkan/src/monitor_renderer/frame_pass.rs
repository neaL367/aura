use ash::vk;
use aura_core::wallpaper::FitMode;

use crate::{context::VulkanContext, error::VulkanError, transform::calculate_uv_transform};

use super::MonitorRenderer;

pub fn execute_frame(
    renderer: &mut MonitorRenderer,
    context: &VulkanContext,
    clear_color: [f32; 4],
) -> Result<(), VulkanError> {
    renderer.frame_sync.wait_and_reset(&context.device)?;

    let (image_index, suboptimal) = unsafe {
        renderer
            .swapchain
            .swapchain_loader
            .acquire_next_image(
                renderer.swapchain.swapchain,
                u64::MAX,
                renderer.frame_sync.image_available_semaphore,
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

    if suboptimal {
        return Err(VulkanError::SwapchainOutOfDate);
    }

    let framebuffer = renderer.framebuffers[image_index as usize];

    unsafe {
        context
            .device
            .reset_command_buffer(
                renderer.command_buffer,
                vk::CommandBufferResetFlags::empty(),
            )
            .map_err(|e| VulkanError::Render(format!("reset_command_buffer failed: {}", e)))?;
    }

    let begin_info =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

    unsafe {
        context
            .device
            .begin_command_buffer(renderer.command_buffer, &begin_info)
            .map_err(|e| VulkanError::Render(format!("begin_command_buffer failed: {}", e)))?;
    }

    let clear_value = vk::ClearValue {
        color: vk::ClearColorValue {
            float32: clear_color,
        },
    };

    // A presented video frame must be transitioned out of the decode layout
    // before it is sampled (once per presented frame; CONCURRENT image
    // sharing with the decode queue, so no ownership transfer is required).
    if let Some(ref mut video) = renderer.active_video_frame
        && !video.layout_transitioned
    {
        let barrier = vk::ImageMemoryBarrier2::default()
            .old_layout(vk::ImageLayout::VIDEO_DECODE_DPB_KHR)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(video.image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })
            .src_stage_mask(vk::PipelineStageFlags2::VIDEO_DECODE_KHR)
            .src_access_mask(vk::AccessFlags2::VIDEO_DECODE_WRITE_KHR)
            .dst_stage_mask(vk::PipelineStageFlags2::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags2::SHADER_SAMPLED_READ);
        let dependencies =
            vk::DependencyInfo::default().image_memory_barriers(std::slice::from_ref(&barrier));
        unsafe {
            context
                .device
                .cmd_pipeline_barrier2(renderer.command_buffer, &dependencies);
        }
        video.layout_transitioned = true;
    }

    let render_pass_begin = vk::RenderPassBeginInfo::default()
        .render_pass(renderer.pipeline.render_pass)
        .framebuffer(framebuffer)
        .render_area(vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: renderer.swapchain.extent,
        })
        .clear_values(std::slice::from_ref(&clear_value));

    unsafe {
        context.device.cmd_begin_render_pass(
            renderer.command_buffer,
            &render_pass_begin,
            vk::SubpassContents::INLINE,
        );
    }

    let viewport = vk::Viewport::default()
        .x(0.0)
        .y(0.0)
        .width(renderer.swapchain.extent.width as f32)
        .height(renderer.swapchain.extent.height as f32)
        .min_depth(0.0)
        .max_depth(1.0);

    let scissor = vk::Rect2D {
        offset: vk::Offset2D { x: 0, y: 0 },
        extent: renderer.swapchain.extent,
    };

    unsafe {
        context.device.cmd_set_viewport(
            renderer.command_buffer,
            0,
            std::slice::from_ref(&viewport),
        );
        context
            .device
            .cmd_set_scissor(renderer.command_buffer, 0, std::slice::from_ref(&scissor));
        context.device.cmd_bind_pipeline(
            renderer.command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            renderer.pipeline.pipeline,
        );
    }

    let video_frame = renderer.active_video_frame.as_ref();
    let has_sampled_content = video_frame.is_some() || renderer.active_texture.is_some();

    if has_sampled_content {
        let (img_w, img_h) = match video_frame {
            Some(video) => (video.width, video.height),
            None => {
                let texture = renderer.active_texture.as_ref().expect("checked above");
                (texture.width, texture.height)
            }
        };
        let pc = if renderer.active_fit_mode == FitMode::Span {
            crate::transform::calculate_span_uv_transform(
                img_w,
                img_h,
                renderer.virtual_x,
                renderer.virtual_y,
                renderer.swapchain.extent.width,
                renderer.swapchain.extent.height,
                renderer.virtual_desktop_width,
                renderer.virtual_desktop_height,
            )
        } else {
            calculate_uv_transform(
                renderer.active_fit_mode,
                img_w,
                img_h,
                renderer.swapchain.extent.width,
                renderer.swapchain.extent.height,
            )
        };
        let mut pc_bytes = [0u8; 16];
        pc_bytes[0..4].copy_from_slice(&pc.uv_scale[0].to_ne_bytes());
        pc_bytes[4..8].copy_from_slice(&pc.uv_scale[1].to_ne_bytes());
        pc_bytes[8..12].copy_from_slice(&pc.uv_offset[0].to_ne_bytes());
        pc_bytes[12..16].copy_from_slice(&pc.uv_offset[1].to_ne_bytes());

        unsafe {
            context.device.cmd_push_constants(
                renderer.command_buffer,
                renderer.pipeline.pipeline_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                &pc_bytes,
            );
            context.device.cmd_bind_descriptor_sets(
                renderer.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                renderer.pipeline.pipeline_layout,
                0,
                std::slice::from_ref(&renderer.descriptor_set),
                &[],
            );
        }
    }

    unsafe {
        context.device.cmd_draw(renderer.command_buffer, 6, 1, 0, 0);
        context.device.cmd_end_render_pass(renderer.command_buffer);
        context
            .device
            .end_command_buffer(renderer.command_buffer)
            .map_err(|e| VulkanError::Render(format!("end_command_buffer failed: {}", e)))?;
    }

    // Wait on the decode timeline so the presented frame's DPB contents are
    // guaranteed complete before sampling. Values for binary semaphores must
    // be 0 when VkTimelineSemaphoreSubmitInfo is chained.
    let mut wait_semaphores = vec![renderer.frame_sync.image_available_semaphore];
    let mut wait_stages = vec![vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
    let mut wait_values = vec![0u64];
    let mut signal_semaphores = vec![renderer.frame_sync.render_finished_semaphore];
    let mut signal_values = vec![0u64];
    if let Some(ref mut video) = renderer.active_video_frame {
        wait_semaphores.push(video.timeline_semaphore);
        wait_stages.push(vk::PipelineStageFlags::FRAGMENT_SHADER);
        wait_values.push(video.timeline_value);
        // Signal the graphics->video timeline in this same submit: once the
        // sampling commands complete, the decode worker may overwrite the
        // slot. The value is reported back to the worker in the slot ack.
        renderer.gfx_timeline_value += 1;
        signal_semaphores.push(video.gfx_timeline);
        signal_values.push(renderer.gfx_timeline_value);
        video.sampled_gfx_value = renderer.gfx_timeline_value;
    }

    let mut timeline_info = vk::TimelineSemaphoreSubmitInfo::default()
        .wait_semaphore_values(&wait_values)
        .signal_semaphore_values(&signal_values);

    let submit_info = vk::SubmitInfo::default()
        .wait_semaphores(&wait_semaphores)
        .wait_dst_stage_mask(&wait_stages)
        .command_buffers(std::slice::from_ref(&renderer.command_buffer))
        .signal_semaphores(&signal_semaphores)
        .push_next(&mut timeline_info);

    let _lock = context.queue_lock();

    unsafe {
        context
            .device
            .queue_submit(
                context.graphics_queue,
                &[submit_info],
                renderer.frame_sync.in_flight_fence,
            )
            .map_err(|e| VulkanError::Render(e.to_string()))?;
    }

    let present_info = vk::PresentInfoKHR::default()
        .wait_semaphores(&signal_semaphores)
        .swapchains(std::slice::from_ref(&renderer.swapchain.swapchain))
        .image_indices(std::slice::from_ref(&image_index));

    unsafe {
        let suboptimal = renderer
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
