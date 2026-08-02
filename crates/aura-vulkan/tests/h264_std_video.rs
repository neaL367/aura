use aura_media::h264_parser::{H264Pps, H264Sps};
use aura_vulkan::h264_std_video::{StdH264Pps, StdH264Sps};

#[test]
fn maps_sps_to_std_video_struct() {
    let sps = H264Sps {
        profile_idc: 77,
        level_idc: 31,
        chroma_format_idc: 1,
        pic_order_cnt_type: 2,
        frame_mbs_only_flag: true,
        frame_cropping_flag: true,
        frame_crop_bottom_offset: 2,
        pic_width_in_mbs_minus1: 39,
        pic_height_in_map_units_minus1: 22,
        max_num_ref_frames: 4,
        log2_max_frame_num_minus4: 1,
        ..Default::default()
    };

    let std = StdH264Sps::from_sps(&sps).unwrap().as_std();
    assert_eq!(std.profile_idc, 77); // STD_VIDEO_H264_PROFILE_IDC_MAIN
    assert_eq!(std.level_idc, 8); // STD_VIDEO_H264_LEVEL_IDC_3_1
    assert_eq!(std.chroma_format_idc, 1); // STD_VIDEO_H264_CHROMA_FORMAT_IDC_420
    assert_eq!(std.pic_order_cnt_type, 2); // STD_VIDEO_H264_POC_TYPE_2
    assert_eq!(std.max_num_ref_frames, 4);
    assert_eq!(std.pic_width_in_mbs_minus1, 39);
    assert_eq!(std.flags.frame_mbs_only_flag(), 1);
    assert_eq!(std.flags.frame_cropping_flag(), 1);
    assert!(!std.pOffsetForRefFrame.is_null());
}

#[test]
fn maps_pps_to_std_video_struct() {
    let pps = H264Pps {
        pic_parameter_set_id: 1,
        seq_parameter_set_id: 0,
        entropy_coding_mode_flag: true,
        num_ref_idx_l0_default_active_minus1: 3,
        ..Default::default()
    };

    let std = StdH264Pps::from_pps(&pps).as_std();
    assert_eq!(std.pic_parameter_set_id, 1);
    assert_eq!(std.seq_parameter_set_id, 0);
    assert_eq!(std.num_ref_idx_l0_default_active_minus1, 3);
    assert_eq!(std.flags.entropy_coding_mode_flag(), 1);
    assert!(std.pScalingLists.is_null());
}
