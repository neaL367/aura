use super::types::{DecodeFrameInput, VideoDecodePipeline};
use crate::{context::VulkanContext, error::VulkanError, video_session::VulkanVideoSession};
use ash::vk;
use ash::vk::native::{
    StdVideoDecodeH264PictureInfo, StdVideoDecodeH264PictureInfoFlags,
    StdVideoDecodeH264ReferenceInfo, StdVideoDecodeH264ReferenceInfoFlags,
};

impl VideoDecodePipeline {
    /// Record the decode picture commands (layout transitions, coding
    /// scope, reset, `vkCmdDecodeVideoKHR`) for a picture already staged
    /// in `src_buffer`. Called from `submit_decode` between command buffer
    /// begin and queue submit.
    pub(super) unsafe fn record_decode_commands(
        &mut self,
        context: &VulkanContext,
        session: &VulkanVideoSession,
        input: &DecodeFrameInput,
        decode_command_buffer: vk::CommandBuffer,
        src_buffer: vk::Buffer,
    ) -> Result<(), VulkanError> {
        let Some(loader) = context.video_queue_device_loader.as_ref() else {
            return Err(VulkanError::MissingExtension("VK_KHR_video_queue"));
        };
        let Some(decode_loader) = context.video_decode_queue_loader.as_ref() else {
            return Err(VulkanError::MissingExtension("VK_KHR_video_decode_queue"));
        };
        let coded_extent = vk::Extent2D {
            width: session.width,
            height: session.height,
        };

        // Picture resources for every DPB slot.
        let picture_resources: Vec<vk::VideoPictureResourceInfoKHR> = session
            .dpb_slots
            .iter()
            .map(|slot| {
                vk::VideoPictureResourceInfoKHR::default()
                    .coded_offset(vk::Offset2D { x: 0, y: 0 })
                    .coded_extent(coded_extent)
                    .base_array_layer(0)
                    .image_view_binding(slot.view)
            })
            .collect();

        // -- Decode picture chain structs (alive through submit) --------------
        let mut std_pic_flags = unsafe { std::mem::zeroed::<StdVideoDecodeH264PictureInfoFlags>() };
        std_pic_flags.set_is_intra(input.is_idr as u32);
        std_pic_flags.set_IdrPicFlag(input.is_idr as u32);
        std_pic_flags.set_is_reference(input.is_reference as u32);

        let std_pic_info = StdVideoDecodeH264PictureInfo {
            flags: std_pic_flags,
            seq_parameter_set_id: input.seq_parameter_set_id,
            pic_parameter_set_id: input.pic_parameter_set_id,
            reserved1: 0,
            reserved2: 0,
            frame_num: input.frame_num,
            idr_pic_id: input.idr_pic_id,
            PicOrderCnt: input.pic_order_cnt,
        };

        let std_ref_infos: Vec<StdVideoDecodeH264ReferenceInfo> = input
            .references
            .iter()
            .map(|r| StdVideoDecodeH264ReferenceInfo {
                flags: unsafe { std::mem::zeroed::<StdVideoDecodeH264ReferenceInfoFlags>() },
                FrameNum: r.frame_num,
                reserved: 0,
                PicOrderCnt: r.pic_order_cnt,
            })
            .collect();

        let dpb_slot_infos: Vec<vk::VideoDecodeH264DpbSlotInfoKHR> = input
            .references
            .iter()
            .enumerate()
            .map(|(i, _)| {
                vk::VideoDecodeH264DpbSlotInfoKHR::default().std_reference_info(&std_ref_infos[i])
            })
            .collect();

        // Reference slots chain their `VideoDecodeH264DpbSlotInfoKHR` via
        // `p_next` raw pointer (the builder's `&'a mut` borrow cannot span
        // loop iterations; `dpb_slot_infos` lives until after submit).
        let ref_slot_infos: Vec<vk::VideoReferenceSlotInfoKHR> = input
            .references
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut slot = vk::VideoReferenceSlotInfoKHR::default()
                    .slot_index(r.slot_index as i32)
                    .picture_resource(&picture_resources[r.slot_index as usize]);
                slot.p_next = std::ptr::addr_of!(dpb_slot_infos[i]).cast::<std::ffi::c_void>();
                slot
            })
            .collect();

        // Setup (output picture) slot with the current picture's ref info.
        let setup_std_ref_info = StdVideoDecodeH264ReferenceInfo {
            flags: unsafe { std::mem::zeroed::<StdVideoDecodeH264ReferenceInfoFlags>() },
            FrameNum: input.frame_num,
            reserved: 0,
            PicOrderCnt: input.pic_order_cnt,
        };

        let mut setup_dpb_slot_info =
            vk::VideoDecodeH264DpbSlotInfoKHR::default().std_reference_info(&setup_std_ref_info);

        let setup_slot_info = vk::VideoReferenceSlotInfoKHR::default()
            .slot_index(input.setup_slot_index as i32)
            .picture_resource(&picture_resources[input.setup_slot_index as usize])
            .push_next(&mut setup_dpb_slot_info);

        let mut h264_picture_info = vk::VideoDecodeH264PictureInfoKHR::default()
            .std_picture_info(&std_pic_info)
            .slice_offsets(input.slice_offsets);

        let decode_info = vk::VideoDecodeInfoKHR::default()
            .src_buffer(src_buffer)
            .src_buffer_offset(0)
            .src_buffer_range(input.bitstream.len() as u64)
            .dst_picture_resource(picture_resources[input.setup_slot_index as usize])
            .setup_reference_slot(&setup_slot_info)
            .reference_slots(&ref_slot_infos)
            .push_next(&mut h264_picture_info);

        let end_coding = vk::VideoEndCodingInfoKHR::default();

        unsafe {
            // -- Layout transitions for the slots used by this decode --------
            // Every used slot (setup output + references) must be in
            // `VIDEO_DECODE_DPB_KHR`. First use starts from `UNDEFINED`;
            // slots sampled by the graphics queue come back from
            // `SHADER_READ_ONLY_OPTIMAL` after the renderer acked them.
            let mut used_slots: Vec<u32> = input.references.iter().map(|r| r.slot_index).collect();
            used_slots.push(input.setup_slot_index);

            let mut slot_barriers: Vec<vk::ImageMemoryBarrier2> = Vec::new();
            for slot_index in used_slots {
                let layout = self.slot_layouts[slot_index as usize];
                if layout == vk::ImageLayout::VIDEO_DECODE_DPB_KHR {
                    continue;
                }
                let (src_stage, src_access) = match layout {
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => (
                        vk::PipelineStageFlags2::FRAGMENT_SHADER,
                        vk::AccessFlags2::SHADER_SAMPLED_READ,
                    ),
                    _ => (vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE),
                };
                slot_barriers.push(
                    vk::ImageMemoryBarrier2::default()
                        .old_layout(layout)
                        .new_layout(vk::ImageLayout::VIDEO_DECODE_DPB_KHR)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .image(session.dpb_slots[slot_index as usize].image)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        })
                        .src_stage_mask(src_stage)
                        .src_access_mask(src_access)
                        .dst_stage_mask(vk::PipelineStageFlags2::VIDEO_DECODE_KHR)
                        .dst_access_mask(vk::AccessFlags2::VIDEO_DECODE_WRITE_KHR),
                );
                self.slot_layouts[slot_index as usize] = vk::ImageLayout::VIDEO_DECODE_DPB_KHR;
            }
            if !slot_barriers.is_empty() {
                let dependency_info =
                    vk::DependencyInfo::default().image_memory_barriers(&slot_barriers);
                context
                    .device
                    .cmd_pipeline_barrier2(decode_command_buffer, &dependency_info);
            }

            // -- Begin coding scope (bind all DPB slots) ----------------------
            let bound_slots: Vec<vk::VideoReferenceSlotInfoKHR> = session
                .dpb_slots
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    vk::VideoReferenceSlotInfoKHR::default()
                        .slot_index(i as i32)
                        .picture_resource(&picture_resources[i])
                })
                .collect();

            let begin_coding = vk::VideoBeginCodingInfoKHR::default()
                .video_session(session.session)
                .video_session_parameters(session.session_parameters)
                .reference_slots(&bound_slots);

            (loader.fp().cmd_begin_video_coding_khr)(decode_command_buffer, &begin_coding);

            // -- Reset on first scope / loop / seek ---------------------------
            if self.session_reset_required {
                let control = vk::VideoCodingControlInfoKHR::default()
                    .flags(vk::VideoCodingControlFlagsKHR::RESET);
                (loader.fp().cmd_control_video_coding_khr)(decode_command_buffer, &control);
                self.session_reset_required = false;
            }

            // -- Decode picture -------------------------------------------------
            (decode_loader.fp().cmd_decode_video_khr)(decode_command_buffer, &decode_info);

            // -- End coding scope ------------------------------------------------
            (loader.fp().cmd_end_video_coding_khr)(decode_command_buffer, &end_coding);

            context
                .device
                .end_command_buffer(decode_command_buffer)
                .map_err(|e| VulkanError::Video(format!("cb end: {e:?}")))?;
        }

        Ok(())
    }
}
