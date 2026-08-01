use super::types::VideoDecodePipeline;
use crate::context::VulkanContext;
use ash::vk;

impl VideoDecodePipeline {
    /// Clean up decode command pool, staging ring, and timeline semaphore.
    ///
    /// # Safety
    /// Must be called when GPU execution using this pipeline has completed.
    pub unsafe fn destroy(&mut self, context: &VulkanContext) {
        unsafe {
            for slot in &mut self.staging_slots {
                if slot.buffer != vk::Buffer::null() {
                    context.device.destroy_buffer(slot.buffer, None);
                    slot.buffer = vk::Buffer::null();
                }
                if let Some(alloc) = slot.allocation.take()
                    && let Ok(mut guard) = context.allocator.lock()
                    && let Some(ref mut allocator) = *guard
                {
                    let _ = allocator.free(alloc);
                }
                if slot.command_buffer != vk::CommandBuffer::null() {
                    context
                        .device
                        .free_command_buffers(self.decode_command_pool, &[slot.command_buffer]);
                    slot.command_buffer = vk::CommandBuffer::null();
                }
            }
            if self.timeline_semaphore != vk::Semaphore::null() {
                context
                    .device
                    .destroy_semaphore(self.timeline_semaphore, None);
                self.timeline_semaphore = vk::Semaphore::null();
            }
            if self.gfx_timeline != vk::Semaphore::null() {
                context.device.destroy_semaphore(self.gfx_timeline, None);
                self.gfx_timeline = vk::Semaphore::null();
            }
            if self.decode_command_pool != vk::CommandPool::null() {
                context
                    .device
                    .destroy_command_pool(self.decode_command_pool, None);
                self.decode_command_pool = vk::CommandPool::null();
            }
        }
    }
}
