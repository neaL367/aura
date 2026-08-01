use crate::video_session::{DpbSlot, NV12};
use crate::{context::VulkanContext, error::VulkanError};
use ash::vk;

/// Create the DPB image array (`max_num_ref_frames + 1` slots).
/// DPB images are multi-planar (`G8_B8R8_2PLANE_420_UNORM`) and
/// CONCURRENT-shared between the video decode and graphics queue
/// families when they differ.
pub(super) fn create_dpb_slots(
    context: &VulkanContext,
    width: u32,
    height: u32,
    queue_family: u32,
    dpb_capacity: u32,
    ycbcr_conversion: vk::SamplerYcbcrConversion,
) -> Result<Vec<DpbSlot>, VulkanError> {
    // -- DPB images -----------------------------------------------------
    let sharing_mode = if queue_family == context.graphics_queue_family {
        vk::SharingMode::EXCLUSIVE
    } else {
        vk::SharingMode::CONCURRENT
    };
    let shared_families = [queue_family, context.graphics_queue_family];
    let queue_family_indices: &[u32] = if sharing_mode == vk::SharingMode::CONCURRENT {
        &shared_families
    } else {
        &[]
    };

    let mut dpb_slots = Vec::with_capacity(dpb_capacity as usize);
    for i in 0..dpb_capacity {
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(NV12)
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
                vk::ImageUsageFlags::VIDEO_DECODE_DPB_KHR
                    | vk::ImageUsageFlags::VIDEO_DECODE_DST_KHR
                    | vk::ImageUsageFlags::SAMPLED,
            )
            .sharing_mode(sharing_mode)
            .queue_family_indices(queue_family_indices)
            .flags(vk::ImageCreateFlags::MUTABLE_FORMAT)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let image = unsafe {
            context
                .device
                .create_image(&image_info, None)
                .map_err(|e| VulkanError::Allocation(format!("DPB Image {i} failed: {e}")))?
        };

        let reqs = unsafe { context.device.get_image_memory_requirements(image) };
        let allocation = {
            let mut guard = context.allocator.lock().unwrap();
            guard.as_mut().and_then(|allocator| {
                allocator
                    .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                        name: "DPB Image Allocation",
                        requirements: reqs,
                        location: gpu_allocator::MemoryLocation::GpuOnly,
                        linear: false,
                        allocation_scheme:
                            gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
                    })
                    .ok()
            })
        };
        let Some(allocation) = allocation else {
            return Err(VulkanError::Allocation(format!(
                "DPB Image {i} memory allocation failed"
            )));
        };
        unsafe {
            context
                .device
                .bind_image_memory(image, allocation.memory(), allocation.offset())
                .map_err(|e| VulkanError::Allocation(format!("DPB Image {i} bind: {e}")))?
        };

        let mut ycbcr_info = vk::SamplerYcbcrConversionInfo::default().conversion(ycbcr_conversion);

        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(NV12)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })
            .push_next(&mut ycbcr_info);

        let view = unsafe {
            context
                .device
                .create_image_view(&view_info, None)
                .map_err(|e| VulkanError::Allocation(format!("DPB Image {i} view: {e}")))?
        };

        dpb_slots.push(DpbSlot {
            image,
            view,
            allocation: Some(allocation),
        });
    }

    Ok(dpb_slots)
}
