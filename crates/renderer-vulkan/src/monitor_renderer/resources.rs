use ash::vk;
use aura_core::wallpaper::FitMode;

use crate::{
    context::VulkanContext, error::VulkanError, pipeline::GraphicsPipeline, swapchain::Swapchain,
    texture::GpuTexture,
};

pub fn create_descriptor_pool(
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

pub fn allocate_descriptor_set(
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

pub fn update_descriptor_set(
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

pub fn create_framebuffers(
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

/// Destroy framebuffers.
///
/// # Safety
/// The caller must ensure that the device and framebuffers are valid and no pending render commands reference them.
pub unsafe fn destroy_framebuffers(device: &ash::Device, framebuffers: &mut Vec<vk::Framebuffer>) {
    unsafe {
        for fb in framebuffers.drain(..) {
            device.destroy_framebuffer(fb, None);
        }
    }
}
