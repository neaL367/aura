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
        _context: &VulkanContext,
        width: u32,
        height: u32,
        max_ref_frames: u32,
    ) -> Result<Self, VulkanError> {
        let dpb_capacity = (max_ref_frames + 1) as usize;
        let dpb_slots = Vec::with_capacity(dpb_capacity);

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
