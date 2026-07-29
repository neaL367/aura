use ash::vk;
use gpu_allocator::vulkan::AllocationCreateDesc;

use crate::{context::VulkanContext, error::VulkanError};

pub struct GpuTexture {
    pub image: vk::Image,
    pub allocation: Option<gpu_allocator::vulkan::Allocation>,
    pub view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub width: u32,
    pub height: u32,
    pub layout: vk::ImageLayout,
}

impl GpuTexture {
    pub fn create_2d(
        context: &VulkanContext,
        width: u32,
        height: u32,
        format: vk::Format,
    ) -> Result<Self, VulkanError> {
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(
                vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_DST
                    | vk::ImageUsageFlags::COLOR_ATTACHMENT,
            )
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let image = unsafe {
            context
                .device
                .create_image(&image_info, None)
                .map_err(|e| VulkanError::Texture(e.to_string()))?
        };

        let reqs = unsafe { context.device.get_image_memory_requirements(image) };

        let allocation = {
            let mut guard = context.allocator.lock().unwrap();
            let alloc = guard.as_mut().ok_or_else(|| {
                VulkanError::Allocation("Allocator missing during texture creation".to_string())
            })?;
            alloc
                .allocate(&AllocationCreateDesc {
                    name: "GpuTexture Allocation",
                    requirements: reqs,
                    location: gpu_allocator::MemoryLocation::GpuOnly,
                    linear: false,
                    allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
                })
                .map_err(|e| VulkanError::Allocation(e.to_string()))?
        };

        unsafe {
            context
                .device
                .bind_image_memory(image, allocation.memory(), allocation.offset())
                .map_err(|e| VulkanError::Texture(e.to_string()))?;
        }

        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .components(vk::ComponentMapping {
                r: vk::ComponentSwizzle::IDENTITY,
                g: vk::ComponentSwizzle::IDENTITY,
                b: vk::ComponentSwizzle::IDENTITY,
                a: vk::ComponentSwizzle::IDENTITY,
            })
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        let view = unsafe {
            context
                .device
                .create_image_view(&view_info, None)
                .map_err(|e| VulkanError::Texture(e.to_string()))?
        };

        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .max_anisotropy(1.0);

        let sampler = unsafe {
            context
                .device
                .create_sampler(&sampler_info, None)
                .map_err(|e| VulkanError::Texture(e.to_string()))?
        };

        Ok(Self {
            image,
            allocation: Some(allocation),
            view,
            sampler,
            width,
            height,
            layout: vk::ImageLayout::UNDEFINED,
        })
    }

    /// # Safety
    /// Must be called when GPU execution using this texture has completed.
    pub unsafe fn destroy(&mut self, context: &VulkanContext) {
        unsafe {
            context.device.destroy_sampler(self.sampler, None);
            context.device.destroy_image_view(self.view, None);
            context.device.destroy_image(self.image, None);
            if let Some(alloc) = self.allocation.take()
                && let Ok(mut guard) = context.allocator.lock()
                && let Some(ref mut allocator) = *guard
            {
                let _ = allocator.free(alloc);
            }
        }
    }
}
