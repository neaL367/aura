use super::types::VideoDecodePipeline;
use crate::{context::VulkanContext, error::VulkanError};
use ash::vk;

impl VideoDecodePipeline {
    /// Ensure the staging slot `index` can hold `required` bytes.
    pub(super) fn ensure_staging_slot(
        &mut self,
        context: &VulkanContext,
        index: usize,
        required: u64,
    ) -> Result<(), VulkanError> {
        let slot = &mut self.staging_slots[index];
        if slot.capacity >= required {
            return Ok(());
        }

        let mut next_capacity = slot.capacity.max(16 * 1024 * 1024);
        while next_capacity < required {
            next_capacity *= 2;
        }

        if slot.buffer != vk::Buffer::null() {
            unsafe { context.device.destroy_buffer(slot.buffer, None) };
            slot.buffer = vk::Buffer::null();
        }
        if let Some(alloc) = slot.allocation.take()
            && let Ok(mut guard) = context.allocator.lock()
            && let Some(ref mut allocator) = *guard
        {
            let _ = allocator.free(alloc);
        }

        let buffer_info = vk::BufferCreateInfo::default()
            .size(next_capacity)
            .usage(vk::BufferUsageFlags::VIDEO_DECODE_SRC_KHR)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe {
            context
                .device
                .create_buffer(&buffer_info, None)
                .map_err(|e| VulkanError::Video(format!("bitstream buffer: {e:?}")))?
        };

        let reqs = unsafe { context.device.get_buffer_memory_requirements(buffer) };

        let allocation = {
            let mut guard = context.allocator.lock().unwrap();
            let allocator = guard.as_mut().ok_or_else(|| {
                VulkanError::Allocation("Allocator missing during bitstream upload".to_string())
            })?;
            allocator
                .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                    name: "Video Bitstream Buffer",
                    requirements: reqs,
                    location: gpu_allocator::MemoryLocation::CpuToGpu,
                    linear: true,
                    allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
                })
                .map_err(|e| VulkanError::Allocation(e.to_string()))?
        };

        unsafe {
            context
                .device
                .bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
                .map_err(|e| VulkanError::Video(format!("bitstream bind: {e:?}")))?;
        }

        slot.buffer = buffer;
        slot.allocation = Some(allocation);
        slot.capacity = next_capacity;

        Ok(())
    }
}
