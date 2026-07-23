use ash::vk;

use crate::{context::VulkanContext, error::VulkanError, surface::Surface};

/// Vulkan Swapchain wrapper for a single monitor.
pub struct Swapchain {
    pub swapchain_loader: ash::khr::swapchain::Device,
    pub swapchain: vk::SwapchainKHR,
    pub images: Vec<vk::Image>,
    pub image_views: Vec<vk::ImageView>,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
}

impl Swapchain {
    /// Create a new `Swapchain` for the specified `Surface` and extent.
    ///
    /// Selects vsync presentation mode (`VK_PRESENT_MODE_FIFO_KHR`).
    /// Prefers `B8G8R8A8_UNORM` or `R8G8B8A8_UNORM` color formats.
    pub fn create(
        context: &VulkanContext,
        surface: &Surface,
        width: u32,
        height: u32,
        old_swapchain: vk::SwapchainKHR,
    ) -> Result<Self, VulkanError> {
        let swapchain_loader = ash::khr::swapchain::Device::new(&context.instance, &context.device);

        let caps = surface.get_capabilities(context.physical_device)?;
        let formats = surface.get_formats(context.physical_device)?;
        let present_modes = surface.get_present_modes(context.physical_device)?;

        // Choose surface format (prefer BGRA8 UNORM or RGBA8 UNORM)
        let format = formats
            .iter()
            .cloned()
            .find(|f| {
                f.format == vk::Format::B8G8R8A8_UNORM || f.format == vk::Format::R8G8B8A8_UNORM
            })
            .unwrap_or(formats[0]);

        // Choose present mode (prefer vsync FIFO)
        let present_mode = present_modes
            .iter()
            .cloned()
            .find(|&m| m == vk::PresentModeKHR::FIFO)
            .unwrap_or(vk::PresentModeKHR::FIFO);

        // Compute extent matching requested monitor dimensions and surface capabilities
        let extent = if width > 0 && height > 0 {
            vk::Extent2D {
                width: width.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                height: height.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            }
        } else if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: 1920,
                height: 1080,
            }
        };

        // Determine image count (min_image_count + 1 to avoid waiting on presentation driver)
        let mut image_count = caps.min_image_count + 1;
        if caps.max_image_count > 0 && image_count > caps.max_image_count {
            image_count = caps.max_image_count;
        }

        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface.surface)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(caps.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(old_swapchain);

        let swapchain = unsafe {
            swapchain_loader
                .create_swapchain(&create_info, None)
                .map_err(|e| VulkanError::Swapchain(e.to_string()))?
        };

        let images = unsafe {
            swapchain_loader
                .get_swapchain_images(swapchain)
                .map_err(|e| VulkanError::Swapchain(e.to_string()))?
        };

        // Create image views for each swapchain image
        let mut image_views = Vec::with_capacity(images.len());
        for &image in &images {
            let view_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(format.format)
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
                    .map_err(|e| VulkanError::Swapchain(e.to_string()))?
            };
            image_views.push(view);
        }

        Ok(Self {
            swapchain_loader,
            swapchain,
            images,
            image_views,
            format: format.format,
            extent,
        })
    }

    /// Clean up image views and destroy the swapchain handle.
    ///
    /// # Safety
    /// Must be called when GPU execution on this swapchain has completed.
    pub unsafe fn destroy(&mut self, device: &ash::Device) {
        unsafe {
            for view in self.image_views.drain(..) {
                device.destroy_image_view(view, None);
            }
            if self.swapchain != vk::SwapchainKHR::null() {
                self.swapchain_loader
                    .destroy_swapchain(self.swapchain, None);
                self.swapchain = vk::SwapchainKHR::null();
            }
        }
    }
}
