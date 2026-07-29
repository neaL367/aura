use ash::vk;

use crate::{context::VulkanContext, error::VulkanError};

pub fn ensure_staging_buffer(
    context: &VulkanContext,
    staging_buffer: &mut Option<vk::Buffer>,
    staging_allocation: &mut Option<gpu_allocator::vulkan::Allocation>,
    current_size: &mut u64,
    required_size: u64,
) -> Result<(), VulkanError> {
    if *current_size >= required_size {
        return Ok(());
    }

    if let Some(buf) = staging_buffer.take() {
        unsafe { context.device.destroy_buffer(buf, None) };
    }
    if let Some(alloc) = staging_allocation.take()
        && let Ok(mut guard) = context.allocator.lock()
        && let Some(ref mut allocator) = *guard
    {
        let _ = allocator.free(alloc);
    }

    let buffer_info = vk::BufferCreateInfo::default()
        .size(required_size)
        .usage(vk::BufferUsageFlags::TRANSFER_SRC)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let new_buffer = unsafe {
        context
            .device
            .create_buffer(&buffer_info, None)
            .map_err(|e| VulkanError::Upload(e.to_string()))?
    };

    let reqs = unsafe { context.device.get_buffer_memory_requirements(new_buffer) };

    let new_alloc = {
        let mut guard = context.allocator.lock().unwrap();
        let alloc = guard.as_mut().ok_or_else(|| {
            VulkanError::Allocation("Allocator missing during staging upload".to_string())
        })?;
        alloc
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "Staging Buffer",
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
            .bind_buffer_memory(new_buffer, new_alloc.memory(), new_alloc.offset())
            .map_err(|e| VulkanError::Upload(e.to_string()))?;
    }

    *staging_buffer = Some(new_buffer);
    *staging_allocation = Some(new_alloc);
    *current_size = required_size;

    Ok(())
}

pub fn trim_staging_buffer(
    context: &VulkanContext,
    staging_buffer: &mut Option<vk::Buffer>,
    staging_allocation: &mut Option<gpu_allocator::vulkan::Allocation>,
    current_size: &mut u64,
) {
    if let Some(buf) = staging_buffer.take() {
        unsafe { context.device.destroy_buffer(buf, None) };
    }
    if let Some(alloc) = staging_allocation.take()
        && let Ok(mut guard) = context.allocator.lock()
        && let Some(ref mut allocator) = *guard
    {
        let _ = allocator.free(alloc);
    }
    *current_size = 0;
}
