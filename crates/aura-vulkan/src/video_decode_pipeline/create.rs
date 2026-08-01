use super::types::{DECODE_RING_SIZE, DecodeStagingSlot, VideoDecodePipeline};
use crate::{context::VulkanContext, error::VulkanError};
use ash::vk;

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
            .command_buffer_count(DECODE_RING_SIZE as u32);

        let command_buffers = unsafe {
            context
                .device
                .allocate_command_buffers(&alloc_info)
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?
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

        let mut gfx_type_info = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(0);
        let gfx_semaphore_info = vk::SemaphoreCreateInfo::default().push_next(&mut gfx_type_info);

        let gfx_timeline = unsafe {
            context
                .device
                .create_semaphore(&gfx_semaphore_info, None)
                .map_err(|e| VulkanError::FrameSync(e.to_string()))?
        };

        let staging_slots = command_buffers
            .into_iter()
            .map(|command_buffer| DecodeStagingSlot {
                buffer: vk::Buffer::null(),
                allocation: None,
                capacity: 0,
                command_buffer,
                last_value: 0,
            })
            .collect();

        Ok(Self {
            decode_command_pool,
            timeline_semaphore,
            timeline_value: 0,
            gfx_timeline,
            staging_slots,
            ring_head: 0,
            slot_state: Vec::new(),
            session_reset_required: true,
            slot_layouts: Vec::new(),
            gfx_sampled: Vec::new(),
        })
    }
}
