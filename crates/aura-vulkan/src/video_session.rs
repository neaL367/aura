//! Vulkan Video H.264 Decode Session and DPB Image Allocation (Stage 3).

use ash::vk;

use crate::{context::VulkanContext, error::VulkanError};

/// Decoded Picture Buffer (DPB) image slot container.
pub struct DpbSlot {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub allocation: Option<gpu_allocator::vulkan::Allocation>,
}

/// Manages a `VkVideoSessionKHR`, parameters, and DPB array sized dynamically from `max_num_ref_frames + 1`.
pub struct VulkanVideoSession {
    pub session: vk::VideoSessionKHR,
    pub session_parameters: vk::VideoSessionParametersKHR,
    pub dpb_slots: Vec<DpbSlot>,
    pub width: u32,
    pub height: u32,
    pub max_ref_frames: u32,
}

impl VulkanVideoSession {
    /// Create a new video decode session for H.264.
    pub fn create(
        context: &VulkanContext,
        width: u32,
        height: u32,
        max_ref_frames: u32,
    ) -> Result<Self, VulkanError> {
        let dpb_capacity = (max_ref_frames + 1) as usize;
        let mut dpb_slots = Vec::with_capacity(dpb_capacity);

        // Allocate DPB images
        for i in 0..dpb_capacity {
            let image_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::G8_B8R8_2PLANE_420_UNORM) // Standard NV12 format for H.264 decode
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
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED);

            let image = unsafe {
                context
                    .device
                    .create_image(&image_info, None)
                    .map_err(|e| VulkanError::Allocation(format!("DPB Image {i} failed: {e}")))?
            };

            let reqs = unsafe { context.device.get_image_memory_requirements(image) };
            let alloc = {
                let mut guard = context.allocator.lock().unwrap();
                if let Some(allocator) = guard.as_mut() {
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
                } else {
                    None
                }
            };

            dpb_slots.push(DpbSlot {
                image,
                view: vk::ImageView::null(),
                allocation: alloc,
            });
        }

        tracing::info!(
            "VulkanVideoSession initialized: {}x{}, DPB slots: {}",
            width,
            height,
            dpb_capacity
        );

        Ok(Self {
            session: vk::VideoSessionKHR::null(),
            session_parameters: vk::VideoSessionParametersKHR::null(),
            dpb_slots,
            width,
            height,
            max_ref_frames,
        })
    }

    /// Clean up Vulkan Video Session and DPB image allocations.
    ///
    /// # Safety
    /// Must be called when GPU execution using this session has completed.
    pub unsafe fn destroy(&mut self, context: &VulkanContext) {
        let mut allocator_lock = context.allocator.lock().unwrap();
        for slot in self.dpb_slots.drain(..) {
            unsafe {
                if slot.view != vk::ImageView::null() {
                    context.device.destroy_image_view(slot.view, None);
                }
                if slot.image != vk::Image::null() {
                    context.device.destroy_image(slot.image, None);
                }
                if let (Some(allocator), Some(alloc)) = (allocator_lock.as_mut(), slot.allocation) {
                    let _ = allocator.free(alloc);
                }
            }
        }
        self.session = vk::VideoSessionKHR::null();
        self.session_parameters = vk::VideoSessionParametersKHR::null();
    }
}
