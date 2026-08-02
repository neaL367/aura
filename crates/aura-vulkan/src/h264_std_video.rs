//! Mapping from parsed H.264 syntax elements to Vulkan Video `StdVideoH264*` structs.
//!
//! `StdVideoH264SequenceParameterSet` / `StdVideoH264PictureParameterSet` are
//! passed to `vkCreateVideoSessionParametersKHR` to describe the bitstream
//! the session will decode. The ownership wrappers below keep the Vec-backed
//! pointer fields (`pOffsetForRefFrame`) alive and produce the raw FFI struct
//! on demand.

use ash::vk;
use ash::vk::native::{
    StdVideoH264ChromaFormatIdc,
    StdVideoH264ChromaFormatIdc_STD_VIDEO_H264_CHROMA_FORMAT_IDC_420 as STD_VIDEO_H264_CHROMA_FORMAT_IDC_420,
    StdVideoH264ChromaFormatIdc_STD_VIDEO_H264_CHROMA_FORMAT_IDC_422 as STD_VIDEO_H264_CHROMA_FORMAT_IDC_422,
    StdVideoH264ChromaFormatIdc_STD_VIDEO_H264_CHROMA_FORMAT_IDC_444 as STD_VIDEO_H264_CHROMA_FORMAT_IDC_444,
    StdVideoH264ChromaFormatIdc_STD_VIDEO_H264_CHROMA_FORMAT_IDC_MONOCHROME as STD_VIDEO_H264_CHROMA_FORMAT_IDC_MONOCHROME,
    StdVideoH264LevelIdc,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_1_0 as STD_VIDEO_H264_LEVEL_IDC_1_0,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_1_1 as STD_VIDEO_H264_LEVEL_IDC_1_1,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_1_2 as STD_VIDEO_H264_LEVEL_IDC_1_2,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_1_3 as STD_VIDEO_H264_LEVEL_IDC_1_3,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_2_0 as STD_VIDEO_H264_LEVEL_IDC_2_0,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_2_1 as STD_VIDEO_H264_LEVEL_IDC_2_1,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_2_2 as STD_VIDEO_H264_LEVEL_IDC_2_2,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_3_0 as STD_VIDEO_H264_LEVEL_IDC_3_0,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_3_1 as STD_VIDEO_H264_LEVEL_IDC_3_1,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_3_2 as STD_VIDEO_H264_LEVEL_IDC_3_2,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_4_0 as STD_VIDEO_H264_LEVEL_IDC_4_0,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_4_1 as STD_VIDEO_H264_LEVEL_IDC_4_1,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_4_2 as STD_VIDEO_H264_LEVEL_IDC_4_2,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_5_0 as STD_VIDEO_H264_LEVEL_IDC_5_0,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_5_1 as STD_VIDEO_H264_LEVEL_IDC_5_1,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_5_2 as STD_VIDEO_H264_LEVEL_IDC_5_2,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_6_0 as STD_VIDEO_H264_LEVEL_IDC_6_0,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_6_1 as STD_VIDEO_H264_LEVEL_IDC_6_1,
    StdVideoH264LevelIdc_STD_VIDEO_H264_LEVEL_IDC_6_2 as STD_VIDEO_H264_LEVEL_IDC_6_2,
    StdVideoH264PictureParameterSet, StdVideoH264PocType,
    StdVideoH264PocType_STD_VIDEO_H264_POC_TYPE_0 as STD_VIDEO_H264_POC_TYPE_0,
    StdVideoH264PocType_STD_VIDEO_H264_POC_TYPE_1 as STD_VIDEO_H264_POC_TYPE_1,
    StdVideoH264PocType_STD_VIDEO_H264_POC_TYPE_2 as STD_VIDEO_H264_POC_TYPE_2,
    StdVideoH264ProfileIdc,
    StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_BASELINE as STD_VIDEO_H264_PROFILE_IDC_BASELINE,
    StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_HIGH as STD_VIDEO_H264_PROFILE_IDC_HIGH,
    StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_HIGH_444_PREDICTIVE as STD_VIDEO_H264_PROFILE_IDC_HIGH_444_PREDICTIVE,
    StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_MAIN as STD_VIDEO_H264_PROFILE_IDC_MAIN,
    StdVideoH264SequenceParameterSet, StdVideoH264WeightedBipredIdc,
    StdVideoH264WeightedBipredIdc_STD_VIDEO_H264_WEIGHTED_BIPRED_IDC_DEFAULT as STD_VIDEO_H264_WEIGHTED_BIPRED_IDC_DEFAULT,
    StdVideoH264WeightedBipredIdc_STD_VIDEO_H264_WEIGHTED_BIPRED_IDC_EXPLICIT as STD_VIDEO_H264_WEIGHTED_BIPRED_IDC_EXPLICIT,
    StdVideoH264WeightedBipredIdc_STD_VIDEO_H264_WEIGHTED_BIPRED_IDC_IMPLICIT as STD_VIDEO_H264_WEIGHTED_BIPRED_IDC_IMPLICIT,
};

use aura_media::h264_parser::{H264Pps, H264Sps};

use crate::error::VulkanError;

pub fn h264_profile_idc(profile: u8) -> StdVideoH264ProfileIdc {
    match profile {
        66 => STD_VIDEO_H264_PROFILE_IDC_BASELINE,
        77 => STD_VIDEO_H264_PROFILE_IDC_MAIN,
        100 => STD_VIDEO_H264_PROFILE_IDC_HIGH,
        244 => STD_VIDEO_H264_PROFILE_IDC_HIGH_444_PREDICTIVE,
        // Unsupported profiles map to MAIN's numeric value; capability
        // checking happens before session creation, so this is a fallback.
        other => {
            tracing::warn!("Unsupported H.264 profile_idc {other}, treating as MAIN");
            STD_VIDEO_H264_PROFILE_IDC_MAIN
        }
    }
}

fn level_idc(level: u8) -> StdVideoH264LevelIdc {
    match level {
        10 => STD_VIDEO_H264_LEVEL_IDC_1_0,
        11 => STD_VIDEO_H264_LEVEL_IDC_1_1,
        12 => STD_VIDEO_H264_LEVEL_IDC_1_2,
        13 => STD_VIDEO_H264_LEVEL_IDC_1_3,
        20 => STD_VIDEO_H264_LEVEL_IDC_2_0,
        21 => STD_VIDEO_H264_LEVEL_IDC_2_1,
        22 => STD_VIDEO_H264_LEVEL_IDC_2_2,
        30 => STD_VIDEO_H264_LEVEL_IDC_3_0,
        31 => STD_VIDEO_H264_LEVEL_IDC_3_1,
        32 => STD_VIDEO_H264_LEVEL_IDC_3_2,
        40 => STD_VIDEO_H264_LEVEL_IDC_4_0,
        41 => STD_VIDEO_H264_LEVEL_IDC_4_1,
        42 => STD_VIDEO_H264_LEVEL_IDC_4_2,
        50 => STD_VIDEO_H264_LEVEL_IDC_5_0,
        51 => STD_VIDEO_H264_LEVEL_IDC_5_1,
        52 => STD_VIDEO_H264_LEVEL_IDC_5_2,
        60 => STD_VIDEO_H264_LEVEL_IDC_6_0,
        61 => STD_VIDEO_H264_LEVEL_IDC_6_1,
        62 => STD_VIDEO_H264_LEVEL_IDC_6_2,
        other => {
            tracing::warn!("Unusual H.264 level_idc {other}, using 3.1");
            STD_VIDEO_H264_LEVEL_IDC_3_1
        }
    }
}

fn chroma_format_idc(chroma: u8) -> StdVideoH264ChromaFormatIdc {
    match chroma {
        0 => STD_VIDEO_H264_CHROMA_FORMAT_IDC_MONOCHROME,
        1 => STD_VIDEO_H264_CHROMA_FORMAT_IDC_420,
        2 => STD_VIDEO_H264_CHROMA_FORMAT_IDC_422,
        _ => STD_VIDEO_H264_CHROMA_FORMAT_IDC_444,
    }
}

fn poc_type(poc: u8) -> StdVideoH264PocType {
    match poc {
        1 => STD_VIDEO_H264_POC_TYPE_1,
        2 => STD_VIDEO_H264_POC_TYPE_2,
        _ => STD_VIDEO_H264_POC_TYPE_0,
    }
}

fn weighted_bipred_idc(idc: u8) -> StdVideoH264WeightedBipredIdc {
    match idc {
        1 => STD_VIDEO_H264_WEIGHTED_BIPRED_IDC_EXPLICIT,
        2 => STD_VIDEO_H264_WEIGHTED_BIPRED_IDC_IMPLICIT,
        _ => STD_VIDEO_H264_WEIGHTED_BIPRED_IDC_DEFAULT,
    }
}

/// Owns the StdVideo SPS plus the Vec backing `pOffsetForRefFrame`.
pub struct StdH264Sps {
    std: StdVideoH264SequenceParameterSet,
    offset_for_ref_frame: Vec<i32>,
}

impl StdH264Sps {
    pub fn from_sps(sps: &H264Sps) -> Result<Self, VulkanError> {
        let mut flags = unsafe { std::mem::zeroed::<vk::native::StdVideoH264SpsFlags>() };
        flags.set_separate_colour_plane_flag(u32::from(sps.separate_colour_plane_flag));
        flags.set_frame_mbs_only_flag(u32::from(sps.frame_mbs_only_flag));
        flags.set_mb_adaptive_frame_field_flag(u32::from(sps.mb_adaptive_frame_field_flag));
        flags.set_direct_8x8_inference_flag(u32::from(sps.direct_8x8_inference_flag));
        flags.set_delta_pic_order_always_zero_flag(u32::from(sps.delta_pic_order_always_zero_flag));
        flags.set_gaps_in_frame_num_value_allowed_flag(u32::from(
            sps.gaps_in_frame_num_value_allowed_flag,
        ));
        flags.set_frame_cropping_flag(u32::from(sps.frame_cropping_flag));

        let mut offset_for_ref_frame = sps.offset_for_ref_frame.clone();
        if offset_for_ref_frame.is_empty() {
            // Spec requires a non-null array for POC type 0 when
            // num_ref_frames_in_pic_order_cnt_cycle > 0; default to zero.
            offset_for_ref_frame =
                vec![0; sps.num_ref_frames_in_pic_order_cnt_cycle.max(1) as usize];
        }

        let std = StdVideoH264SequenceParameterSet {
            flags,
            profile_idc: h264_profile_idc(sps.profile_idc),
            level_idc: level_idc(sps.level_idc),
            chroma_format_idc: chroma_format_idc(sps.chroma_format_idc),
            seq_parameter_set_id: sps.seq_parameter_set_id,
            bit_depth_luma_minus8: sps.bit_depth_luma_minus8,
            bit_depth_chroma_minus8: sps.bit_depth_chroma_minus8,
            log2_max_frame_num_minus4: sps.log2_max_frame_num_minus4,
            pic_order_cnt_type: poc_type(sps.pic_order_cnt_type),
            offset_for_non_ref_pic: sps.offset_for_non_ref_pic,
            offset_for_top_to_bottom_field: sps.offset_for_top_to_bottom_field,
            log2_max_pic_order_cnt_lsb_minus4: sps.log2_max_pic_order_cnt_lsb_minus4,
            num_ref_frames_in_pic_order_cnt_cycle: sps.num_ref_frames_in_pic_order_cnt_cycle,
            max_num_ref_frames: sps.max_num_ref_frames,
            reserved1: 0,
            pic_width_in_mbs_minus1: sps.pic_width_in_mbs_minus1,
            pic_height_in_map_units_minus1: sps.pic_height_in_map_units_minus1,
            frame_crop_left_offset: sps.frame_crop_left_offset,
            frame_crop_right_offset: sps.frame_crop_right_offset,
            frame_crop_top_offset: sps.frame_crop_top_offset,
            frame_crop_bottom_offset: sps.frame_crop_bottom_offset,
            reserved2: 0,
            pOffsetForRefFrame: std::ptr::null(),
            pScalingLists: std::ptr::null(),
            pSequenceParameterSetVui: std::ptr::null(),
        };

        Ok(Self {
            std,
            offset_for_ref_frame,
        })
    }

    /// Raw FFI struct with `pOffsetForRefFrame` pointing at owned storage.
    pub fn as_std(&self) -> StdVideoH264SequenceParameterSet {
        let mut std = self.std;
        std.pOffsetForRefFrame = self.offset_for_ref_frame.as_ptr();
        std
    }
}

/// Owns the StdVideo PPS (scaling-list pointer left null — not required).
pub struct StdH264Pps {
    std: StdVideoH264PictureParameterSet,
}

impl StdH264Pps {
    pub fn from_pps(pps: &H264Pps) -> Self {
        let mut flags = unsafe { std::mem::zeroed::<vk::native::StdVideoH264PpsFlags>() };
        flags.set_entropy_coding_mode_flag(u32::from(pps.entropy_coding_mode_flag));
        flags.set_bottom_field_pic_order_in_frame_present_flag(u32::from(
            pps.bottom_field_pic_order_in_frame_present_flag,
        ));
        flags.set_weighted_pred_flag(u32::from(pps.weighted_pred_flag));
        flags.set_deblocking_filter_control_present_flag(u32::from(
            pps.deblocking_filter_control_present_flag,
        ));
        flags.set_constrained_intra_pred_flag(u32::from(pps.constrained_intra_pred_flag));
        flags.set_redundant_pic_cnt_present_flag(u32::from(pps.redundant_pic_cnt_present_flag));
        flags.set_transform_8x8_mode_flag(u32::from(pps.transform_8x8_mode_flag));

        Self {
            std: StdVideoH264PictureParameterSet {
                flags,
                seq_parameter_set_id: pps.seq_parameter_set_id,
                pic_parameter_set_id: pps.pic_parameter_set_id,
                num_ref_idx_l0_default_active_minus1: pps.num_ref_idx_l0_default_active_minus1,
                num_ref_idx_l1_default_active_minus1: pps.num_ref_idx_l1_default_active_minus1,
                weighted_bipred_idc: weighted_bipred_idc(pps.weighted_bipred_idc),
                pic_init_qp_minus26: pps.pic_init_qp_minus26,
                pic_init_qs_minus26: pps.pic_init_qs_minus26,
                chroma_qp_index_offset: pps.chroma_qp_index_offset,
                second_chroma_qp_index_offset: pps.second_chroma_qp_index_offset,
                pScalingLists: std::ptr::null(),
            },
        }
    }

    pub fn as_std(&self) -> StdVideoH264PictureParameterSet {
        self.std
    }
}
