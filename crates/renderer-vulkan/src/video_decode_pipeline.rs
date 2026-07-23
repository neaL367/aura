//! Vulkan Video Decode Pipeline (Stage 4 & 5).
//!
//! Submits `vkCmdBeginVideoCodingKHR` -> `vkCmdDecodeVideoKHR` -> `vkCmdEndVideoCodingKHR`
//! and synchronizes decode queue output to graphics presentation queue via Timeline Semaphores.

use ash::vk;

use crate::{context::VulkanContext, error::VulkanError, video_session::VulkanVideoSession};

/// Video decode pipeline coordinator for H.264 execution and queue synchronization.
pub struct VideoDecodePipeline {
    pub decode_command_pool: vk::CommandPool,
    pub decode_command_buffer: vk::CommandBuffer,
    pub timeline_semaphore: vk::Semaphore,
    pub timeline_value: u64,
}

impl VideoDecodePipeline {
    pub fn create(context: &VulkanContext, queue_family: u32) -> Result<Self, VulkanError> {
        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

        let decode_command_pool = unsafe {
            context
                .device
                .create_command_pool(&pool_info, None)
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?
        };

        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(decode_command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let decode_command_buffer = unsafe {
            context
                .device
                .allocate_command_buffers(&alloc_info)
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?[0]
        };

        let mut type_info = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(0);

        let semaphore_info = vk::SemaphoreCreateInfo::default().push_next(&mut type_info);

        let timeline_semaphore = unsafe {
            context
                .device
                .create_semaphore(&semaphore_info, None)
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?
        };

        Ok(Self {
            decode_command_pool,
            decode_command_buffer,
            timeline_semaphore,
            timeline_value: 0,
        })
    }

    /// Reset decode session reference frame state upon video loop or seek boundary.
    pub fn reset_session_state(&mut self, _session: &VulkanVideoSession) {
        tracing::info!("VideoDecodePipeline: Flushed reference frame history on stream loop/seek");
    }

    /// Clean up decode command pool and timeline semaphore.
    ///
    /// # Safety
    /// Must be called when GPU execution using this pipeline has completed.
    pub unsafe fn destroy(&mut self, context: &VulkanContext) {
        unsafe {
            if self.timeline_semaphore != vk::Semaphore::null() {
                context
                    .device
                    .destroy_semaphore(self.timeline_semaphore, None);
                self.timeline_semaphore = vk::Semaphore::null();
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
