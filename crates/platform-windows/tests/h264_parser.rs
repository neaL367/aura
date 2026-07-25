use aura_platform_windows::h264_parser::{ParsedH264Frame, PocReorderBuffer, avcc_to_annex_b};

#[test]
fn test_avcc_to_annex_b_conversion() {
    let nal1 = b"ABCD";
    let nal2 = b"EFG";
    let mut avcc = Vec::new();

    avcc.extend_from_slice(&(nal1.len() as u32).to_be_bytes());
    avcc.extend_from_slice(nal1);
    avcc.extend_from_slice(&(nal2.len() as u32).to_be_bytes());
    avcc.extend_from_slice(nal2);

    let annex_b = avcc_to_annex_b(&avcc);

    let mut expected = Vec::new();
    expected.extend_from_slice(&[0, 0, 0, 1]);
    expected.extend_from_slice(nal1);
    expected.extend_from_slice(&[0, 0, 0, 1]);
    expected.extend_from_slice(nal2);

    assert_eq!(annex_b, expected);
}

#[test]
fn test_poc_reorder_buffer_b_frame_display_order() {
    let mut reorder = PocReorderBuffer::new(2);

    // Decode order: I0 (POC 0), P4 (POC 8), B1 (POC 2), B2 (POC 4)
    let f0 = ParsedH264Frame {
        frame_num: 0,
        poc: 0,
        is_idr: true,
        nal_data: vec![],
        pts_ms: 0,
    };
    let f4 = ParsedH264Frame {
        frame_num: 1,
        poc: 8,
        is_idr: false,
        nal_data: vec![],
        pts_ms: 133,
    };
    let f1 = ParsedH264Frame {
        frame_num: 2,
        poc: 2,
        is_idr: false,
        nal_data: vec![],
        pts_ms: 33,
    };
    let f2 = ParsedH264Frame {
        frame_num: 3,
        poc: 4,
        is_idr: false,
        nal_data: vec![],
        pts_ms: 66,
    };

    assert_eq!(reorder.push(f0), None);
    assert_eq!(reorder.push(f4), None);
    assert_eq!(reorder.push(f1).unwrap().poc, 0);
    assert_eq!(reorder.push(f2).unwrap().poc, 2);

    let remaining = reorder.flush();
    let remaining_pocs: Vec<i32> = remaining.iter().map(|f| f.poc).collect();
    assert_eq!(remaining_pocs, vec![4, 8]);
}
