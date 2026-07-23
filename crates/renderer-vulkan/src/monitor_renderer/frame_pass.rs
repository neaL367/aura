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

    let (image_index, _) = unsafe {
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

    let framebuffer = renderer.framebuffers[image_index as usize];

    unsafe {
        context
            .device
            .reset_command_buffer(
                renderer.command_buffer,
                vk::CommandBufferResetFlags::empty(),
            )
            .ok();
    }

    let begin_info =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

    unsafe {
        context
            .device
            .begin_command_buffer(renderer.command_buffer, &begin_info)
            .ok();
    }

    let clear_value = vk::ClearValue {
        color: vk::ClearColorValue {
            float32: clear_color,
        },
    };

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

    if let Some(ref texture) = renderer.active_texture {
        let pc = if renderer.active_fit_mode == FitMode::Span {
            crate::transform::calculate_span_uv_transform(
                texture.width,
                texture.height,
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
                texture.width,
                texture.height,
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
            .ok();
    }

    let wait_semaphores = [renderer.frame_sync.image_available_semaphore];
    let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
    let signal_semaphores = [renderer.frame_sync.render_finished_semaphore];

    let submit_info = vk::SubmitInfo::default()
        .wait_semaphores(&wait_semaphores)
        .wait_dst_stage_mask(&wait_stages)
        .command_buffers(std::slice::from_ref(&renderer.command_buffer))
        .signal_semaphores(&signal_semaphores);

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
