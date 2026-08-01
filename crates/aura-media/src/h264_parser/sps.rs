use super::H264ParseError;
use super::bitreader::BitReader;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct H264Sps {
    pub seq_parameter_set_id: u8,
    pub profile_idc: u8,
    pub level_idc: u8,
    pub chroma_format_idc: u8,
    pub separate_colour_plane_flag: bool,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub log2_max_frame_num_minus4: u8,
    pub pic_order_cnt_type: u8,
    pub log2_max_pic_order_cnt_lsb_minus4: u8,
    pub delta_pic_order_always_zero_flag: bool,
    pub offset_for_non_ref_pic: i32,
    pub offset_for_top_to_bottom_field: i32,
    pub num_ref_frames_in_pic_order_cnt_cycle: u8,
    pub offset_for_ref_frame: Vec<i32>,
    pub max_num_ref_frames: u8,
    pub gaps_in_frame_num_value_allowed_flag: bool,
    pub pic_width_in_mbs_minus1: u32,
    pub pic_height_in_map_units_minus1: u32,
    pub frame_mbs_only_flag: bool,
    pub mb_adaptive_frame_field_flag: bool,
    pub direct_8x8_inference_flag: bool,
    pub frame_cropping_flag: bool,
    pub frame_crop_left_offset: u32,
    pub frame_crop_right_offset: u32,
    pub frame_crop_top_offset: u32,
    pub frame_crop_bottom_offset: u32,
}

impl H264Sps {
    /// Coded (aligned) picture width in pixels.
    pub fn coded_width(&self) -> u32 {
        (self.pic_width_in_mbs_minus1 + 1) * 16
    }

    pub fn coded_height(&self) -> u32 {
        let map_units = (self.pic_height_in_map_units_minus1 + 1) * 16;
        if self.frame_mbs_only_flag {
            map_units
        } else {
            map_units * 2
        }
    }

    /// Cropped display width in pixels (H.264 frame cropping).
    pub fn cropped_width(&self) -> u32 {
        let sub_w = if self.chroma_format_idc == 0 || self.chroma_format_idc == 3 {
            1
        } else {
            2
        };
        self.coded_width()
            .saturating_sub((self.frame_crop_left_offset + self.frame_crop_right_offset) * sub_w)
    }

    pub fn cropped_height(&self) -> u32 {
        let sub_h = if self.chroma_format_idc == 0 || self.chroma_format_idc == 1 {
            1
        } else {
            2
        };
        self.coded_height()
            .saturating_sub((self.frame_crop_top_offset + self.frame_crop_bottom_offset) * sub_h)
    }

    pub fn log2_max_frame_num(&self) -> u32 {
        self.log2_max_frame_num_minus4 as u32 + 4
    }

    pub fn log2_max_pic_order_cnt_lsb(&self) -> u32 {
        self.log2_max_pic_order_cnt_lsb_minus4 as u32 + 4
    }
}

/// Parse a Sequence Parameter Set NAL payload (RBSP, emulation removed).
pub fn parse_sps(payload: &[u8]) -> Result<H264Sps, H264ParseError> {
    let mut r = BitReader::new(payload);
    let profile_idc = r.u(8)? as u8;
    let _constraint_flags = r.u(8)?;
    let level_idc = r.u(8)? as u8;
    let seq_parameter_set_id = r.ue()? as u8;

    let mut sps = H264Sps {
        profile_idc,
        level_idc,
        seq_parameter_set_id,
        chroma_format_idc: 1,
        ..Default::default()
    };

    if matches!(
        profile_idc,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
    ) {
        sps.chroma_format_idc = r.ue()? as u8;
        if sps.chroma_format_idc == 3 {
            sps.separate_colour_plane_flag = r.u(1)? == 1;
        }
        sps.bit_depth_luma_minus8 = r.ue()? as u8;
        sps.bit_depth_chroma_minus8 = r.ue()? as u8;
        let _qpprime_y_zero_transform_bypass_flag = r.u(1)?;
        let seq_scaling_matrix_present_flag = r.u(1)? == 1;
        if seq_scaling_matrix_present_flag {
            // Skip scaling list matrices (not needed for session parameters).
            let count = if sps.chroma_format_idc == 3 { 12 } else { 8 };
            for i in 0..count {
                if r.u(1)? == 1 {
                    let mut size = if i < 6 { 16 } else { 64 };
                    let mut last_scale = 8i32;
                    let mut next_scale = 8i32;
                    while size > 0 {
                        if next_scale != 0 {
                            let delta = r.se()?;
                            next_scale = (last_scale + delta + 256) % 256;
                        }
                        last_scale = if next_scale == 0 {
                            last_scale
                        } else {
                            next_scale
                        };
                        size -= 1;
                    }
                }
            }
        }
    }

    sps.log2_max_frame_num_minus4 = r.ue()? as u8;
    sps.pic_order_cnt_type = r.ue()? as u8;

    if sps.pic_order_cnt_type == 0 {
        sps.log2_max_pic_order_cnt_lsb_minus4 = r.ue()? as u8;
    } else if sps.pic_order_cnt_type == 1 {
        sps.delta_pic_order_always_zero_flag = r.u(1)? == 1;
        sps.offset_for_non_ref_pic = r.se()?;
        sps.offset_for_top_to_bottom_field = r.se()?;
        sps.num_ref_frames_in_pic_order_cnt_cycle = r.ue()? as u8;
        let mut offsets = Vec::with_capacity(sps.num_ref_frames_in_pic_order_cnt_cycle as usize);
        for _ in 0..sps.num_ref_frames_in_pic_order_cnt_cycle {
            offsets.push(r.se()?);
        }
        sps.offset_for_ref_frame = offsets;
    }

    sps.max_num_ref_frames = r.ue()? as u8;
    sps.gaps_in_frame_num_value_allowed_flag = r.u(1)? == 1;
    sps.pic_width_in_mbs_minus1 = r.ue()?;
    sps.pic_height_in_map_units_minus1 = r.ue()?;
    sps.frame_mbs_only_flag = r.u(1)? == 1;
    if !sps.frame_mbs_only_flag {
        sps.mb_adaptive_frame_field_flag = r.u(1)? == 1;
    }
    sps.direct_8x8_inference_flag = r.u(1)? == 1;
    sps.frame_cropping_flag = r.u(1)? == 1;
    if sps.frame_cropping_flag {
        sps.frame_crop_left_offset = r.ue()?;
        sps.frame_crop_right_offset = r.ue()?;
        sps.frame_crop_top_offset = r.ue()?;
        sps.frame_crop_bottom_offset = r.ue()?;
    }

    Ok(sps)
}
