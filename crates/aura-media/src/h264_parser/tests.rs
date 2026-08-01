use super::*;

#[test]
fn test_avcc_to_annex_b_conversion() {
    let nal_payload = [0x67, 0x42, 0x00, 0x0a];
    let mut avcc = vec![0, 0, 0, 4];
    avcc.extend_from_slice(&nal_payload);

    let annex_b = avcc_to_annex_b(&avcc);
    assert_eq!(&annex_b[..4], &[0, 0, 0, 1]);
    assert_eq!(&annex_b[4..], &nal_payload);
}

#[test]
fn test_poc_reorder_buffer_sorting() {
    let mut reorder = PocReorderBuffer::new(2);

    let frame1 = ParsedH264Frame {
        frame_num: 0,
        poc: 4,
        is_idr: false,
        nal_data: vec![1],
        pts_ms: 100,
    };
    let frame2 = ParsedH264Frame {
        frame_num: 1,
        poc: 2,
        is_idr: false,
        nal_data: vec![2],
        pts_ms: 50,
    };
    let frame3 = ParsedH264Frame {
        frame_num: 2,
        poc: 0,
        is_idr: true,
        nal_data: vec![3],
        pts_ms: 0,
    };

    assert!(reorder.push(frame1).is_none());
    assert!(reorder.push(frame2).is_none());

    // Pushing 3rd frame exceeds latency of 2, returning smallest POC (poc 0)
    let popped = reorder.push(frame3).unwrap();
    assert_eq!(popped.poc, 0);

    let flushed = reorder.flush();
    assert_eq!(flushed.len(), 2);
    assert_eq!(flushed[0].poc, 2);
    assert_eq!(flushed[1].poc, 4);
}

#[test]
fn test_split_annex_b_and_emulation_removal() {
    // One SPS NAL (header 0x67) containing an emulation-prevention byte.
    let payload = [0x67, 0x42, 0x00, 0x00, 0x03, 0x02, 0x80, 0x01];
    let mut bitstream = vec![0, 0, 0, 1];
    bitstream.extend_from_slice(&payload);
    bitstream.extend_from_slice(&[0, 0, 1]);
    bitstream.extend_from_slice(&[0x65, 0x88, 0x84]);

    let nals = split_annex_b_nal_units(&bitstream);
    assert_eq!(nals.len(), 2);
    assert_eq!(nals[0][0], 0x67);
    let stripped = remove_emulation_prevention(&nals[0]);
    assert_eq!(stripped.len(), payload.len() - 1);
    assert_eq!(&stripped[2..4], &[0x00, 0x00]);
}

#[test]
fn test_parse_sps_main_profile() {
    // Hand-built SPS (profile_idc 77 = Main, level 3.1, 640x360, 4:2:0)
    // Built with an independent encoder-style bit writer.
    let sps = build_sps_640x360();
    let parsed = parse_sps(&sps).unwrap();
    assert_eq!(parsed.profile_idc, 77, "profile");
    assert_eq!(parsed.level_idc, 31, "level");
    assert_eq!(
        parsed.log2_max_frame_num_minus4, 1,
        "log2_max_frame_num_minus4"
    );
    assert_eq!(parsed.pic_order_cnt_type, 0, "pic_order_cnt_type");
    assert_eq!(parsed.log2_max_pic_order_cnt_lsb_minus4, 0, "log2_lsb");
    assert_eq!(parsed.coded_width(), 640, "coded_width");
    assert_eq!(parsed.coded_height(), 368, "coded_height");
    assert_eq!(parsed.cropped_height(), 364, "cropped_height");
    assert_eq!(parsed.max_num_ref_frames, 4, "max_num_ref_frames");
    assert_eq!(parsed.pic_order_cnt_type, 0);
    assert_eq!(parsed.log2_max_frame_num(), 5);
    assert_eq!(parsed.log2_max_pic_order_cnt_lsb(), 4);
}

#[test]
fn test_poc_tracker_type0() {
    let sps = H264Sps {
        pic_order_cnt_type: 0,
        log2_max_pic_order_cnt_lsb_minus4: 0, // max_lsb = 16
        offset_for_non_ref_pic: 0,
        ..Default::default()
    };
    let mut tracker = PocTracker::new();

    let slice = H264SliceHeader {
        pic_order_cnt_lsb: Some(0),
        ..make_slice()
    };
    assert_eq!(tracker.compute(&sps, &slice, true, true), 0);

    let slice = H264SliceHeader {
        pic_order_cnt_lsb: Some(4),
        ..make_slice()
    };
    assert_eq!(tracker.compute(&sps, &slice, false, true), 4);

    let slice = H264SliceHeader {
        pic_order_cnt_lsb: Some(2),
        ..make_slice()
    };
    assert_eq!(tracker.compute(&sps, &slice, false, true), 2);
}

fn make_slice() -> H264SliceHeader {
    H264SliceHeader {
        first_mb_in_slice: 0,
        slice_type: 0,
        pps_id: 0,
        frame_num: 0,
        field_pic_flag: false,
        bottom_field_flag: false,
        idr_pic_id: 0,
        pic_order_cnt_lsb: None,
        delta_pic_order_cnt_bottom: None,
        delta_pic_order_cnt: None,
    }
}

/// Independent exp-Golomb writer used to construct a valid SPS payload
/// for the parser test (so the test does not rely on the reader under test).
fn build_sps_640x360() -> Vec<u8> {
    struct W {
        bits: Vec<u8>,
    }
    impl W {
        fn u(&mut self, value: u32, n: usize) {
            for i in (0..n).rev() {
                self.bits.push(((value >> i) & 1) as u8);
            }
        }
        fn ue(&mut self, value: u32) {
            let code_num = value + 1; // e.g. 5 = 0b101
            let mut prefix_len = 0;
            let mut v = code_num;
            while v > 1 {
                v >>= 1;
                prefix_len += 1;
            }
            for _ in 0..prefix_len {
                self.bits.push(0);
            }
            self.bits.push(1); // terminating 1
            let suffix_mask = if prefix_len == 0 {
                0
            } else {
                (1 << prefix_len) - 1
            };
            for i in (0..prefix_len).rev() {
                self.bits.push(((code_num & suffix_mask) >> i) as u8);
            }
        }
        fn finish(&self) -> Vec<u8> {
            let mut bytes = Vec::new();
            let mut acc = 0u8;
            let mut n = 0;
            for &b in &self.bits {
                acc = (acc << 1) | b;
                n += 1;
                if n == 8 {
                    bytes.push(acc);
                    acc = 0;
                    n = 0;
                }
            }
            if n > 0 {
                bytes.push(acc << (8 - n));
            }
            bytes
        }
    }

    let mut w = W { bits: Vec::new() };
    w.u(77, 8); // profile_idc = Main
    w.u(0, 8); // constraint flags
    w.u(31, 8); // level_idc = 3.1
    w.ue(0); // seq_parameter_set_id
    w.ue(1); // log2_max_frame_num_minus4 = 1 -> max_frame_num = 16
    w.ue(0); // pic_order_cnt_type = 0
    w.ue(0); // log2_max_pic_order_cnt_lsb_minus4 = 0 -> max_lsb = 16
    w.ue(4); // max_num_ref_frames = 4
    w.u(0, 1); // gaps_in_frame_num_value_allowed_flag
    w.ue(39); // pic_width_in_mbs_minus1 -> 640 px
    w.ue(22); // pic_height_in_map_units_minus1 -> 368 coded, cropped to 360 below
    w.u(1, 1); // frame_mbs_only_flag
    w.u(1, 1); // direct_8x8_inference_flag
    w.u(1, 1); // frame_cropping_flag
    w.ue(0);
    w.ue(0);
    w.ue(0);
    w.ue(4); // crop bottom 4 -> 368 - 8 = 360
    w.u(0, 1); // vui_parameters_present_flag (terminates cleanly)
    w.finish()
}
