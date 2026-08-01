use super::H264ParseError;
use super::bitreader::BitReader;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct H264Pps {
    pub pic_parameter_set_id: u8,
    pub seq_parameter_set_id: u8,
    pub entropy_coding_mode_flag: bool,
    pub bottom_field_pic_order_in_frame_present_flag: bool,
    pub num_ref_idx_l0_default_active_minus1: u8,
    pub num_ref_idx_l1_default_active_minus1: u8,
    pub weighted_pred_flag: bool,
    pub weighted_bipred_idc: u8,
    pub pic_init_qp_minus26: i8,
    pub pic_init_qs_minus26: i8,
    pub chroma_qp_index_offset: i8,
    pub second_chroma_qp_index_offset: i8,
    pub deblocking_filter_control_present_flag: bool,
    pub constrained_intra_pred_flag: bool,
    pub redundant_pic_cnt_present_flag: bool,
    pub transform_8x8_mode_flag: bool,
}

/// Parse a Picture Parameter Set NAL payload (RBSP, emulation removed).
pub fn parse_pps(payload: &[u8]) -> Result<H264Pps, H264ParseError> {
    let mut r = BitReader::new(payload);
    let mut pps = H264Pps {
        pic_parameter_set_id: r.ue()? as u8,
        seq_parameter_set_id: r.ue()? as u8,
        ..Default::default()
    };

    pps.entropy_coding_mode_flag = r.u(1)? == 1;
    pps.bottom_field_pic_order_in_frame_present_flag = r.u(1)? == 1;
    let num_slice_groups_minus1 = r.ue()?;
    if num_slice_groups_minus1 > 0 {
        // Slice-group map types are unsupported for M2a; reject the stream.
        return Err(H264ParseError::Unsupported(
            "multi-slice-group PPS".to_string(),
        ));
    }
    pps.num_ref_idx_l0_default_active_minus1 = r.ue()? as u8;
    pps.num_ref_idx_l1_default_active_minus1 = r.ue()? as u8;
    pps.weighted_pred_flag = r.u(1)? == 1;
    pps.weighted_bipred_idc = r.u(2)? as u8;
    pps.pic_init_qp_minus26 = r.se()? as i8;
    pps.pic_init_qs_minus26 = r.se()? as i8;
    pps.chroma_qp_index_offset = r.se()? as i8;
    pps.deblocking_filter_control_present_flag = r.u(1)? == 1;
    pps.constrained_intra_pred_flag = r.u(1)? == 1;
    pps.redundant_pic_cnt_present_flag = r.u(1)? == 1;

    // Optional PPS tail (present when more_rbsp_data()): transform_8x8_mode_flag,
    // pic_scaling_matrix_present_flag + scaling lists, second_chroma_qp_index_offset.
    if r.bits_left() > 0 {
        pps.transform_8x8_mode_flag = r.u(1)? == 1;
        let pic_scaling_matrix_present = r.u(1)? == 1;
        if pic_scaling_matrix_present {
            // 4:2:0 default: 6 scaling lists, 8 when 8x8 transform is enabled.
            let count = 6 + 2 * u32::from(pps.transform_8x8_mode_flag);
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
        pps.second_chroma_qp_index_offset = r.se()? as i8;
    }

    Ok(pps)
}
