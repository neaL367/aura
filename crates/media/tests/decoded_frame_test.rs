use aura_media::DecodedFrame;
use aura_media::MediaError;

#[test]
fn expected_len_standard_dims() {
    assert_eq!(DecodedFrame::expected_len(1920, 1080), 1920 * 1080 * 4);
}

#[test]
fn expected_len_zero_width() {
    assert_eq!(DecodedFrame::expected_len(0, 1080), 0);
}

#[test]
fn expected_len_zero_height() {
    assert_eq!(DecodedFrame::expected_len(1920, 0), 0);
}

#[test]
fn expected_len_square() {
    assert_eq!(DecodedFrame::expected_len(100, 100), 40_000);
}

fn make_frame(w: u32, h: u32, data_len: usize) -> DecodedFrame {
    DecodedFrame {
        width: w,
        height: h,
        data: vec![0u8; data_len],
        timestamp_ms: 0,
        duration_ms: 0,
    }
}

#[test]
fn validate_correct_length() {
    let f = make_frame(10, 10, 400);
    assert!(f.validate().is_ok());
}

#[test]
fn validate_too_short() {
    let f = make_frame(10, 10, 399);
    match f.validate() {
        Err(MediaError::Decode(msg)) => assert!(msg.contains("buffer size mismatch")),
        other => panic!("expected Decode error, got {other:?}"),
    }
}

#[test]
fn validate_too_long() {
    let f = make_frame(10, 10, 401);
    match f.validate() {
        Err(MediaError::Decode(msg)) => assert!(msg.contains("buffer size mismatch")),
        other => panic!("expected Decode error, got {other:?}"),
    }
}

#[test]
fn validate_zero_dims_empty_data() {
    let f = make_frame(0, 0, 0);
    assert!(f.validate().is_ok());
}

#[test]
fn validate_zero_dims_nonempty_data() {
    let f = make_frame(0, 1080, 100);
    match f.validate() {
        Err(MediaError::Decode(_)) => {}
        other => panic!("expected Decode error, got {other:?}"),
    }
}
