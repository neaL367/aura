use aura_media::{GifDecoder, MediaDecoder};

/// Build a minimal single-frame in-memory GIF (2×2, one opaque color).
fn make_gif(width: u16, height: u16, color: [u8; 3], dispose: gif::DisposalMethod) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut buf, width, height, &[]).expect("encoder");
        encoder.set_repeat(gif::Repeat::Infinite).expect("repeat");
        let palette = color.to_vec();
        let pixel_count = (width as usize) * (height as usize);
        let mut frame = gif::Frame {
            width,
            height,
            palette: Some(palette.clone()),
            buffer: std::borrow::Cow::Owned(vec![0u8; pixel_count]),
            dispose,
            transparent: None,
            delay: 10,
            ..Default::default()
        };
        encoder.write_frame(&frame).expect("frame 1");
        frame.dispose = gif::DisposalMethod::Keep;
        encoder.write_frame(&frame).expect("frame 2");
    }
    buf
}

/// Open a GIF from in-memory bytes by writing to a temp file.
fn open_gif_bytes(bytes: &[u8]) -> GifDecoder {
    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "aura_test_{}.gif",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::write(&path, bytes).expect("write temp gif");
    let decoder = GifDecoder::open(&path).expect("open gif");
    let _ = std::fs::remove_file(&path);
    decoder
}

/// Helper: extract RGBA pixel at (x, y) from a flat RGBA buffer.
fn pixel(data: &[u8], x: u32, y: u32, width: u32) -> [u8; 4] {
    let idx = ((y * width + x) * 4) as usize;
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

#[test]
fn keep_disposal_accumulates_frames() {
    let color = [0xFF_u8, 0x00, 0x00];
    let bytes = make_gif(2, 2, color, gif::DisposalMethod::Keep);
    let mut dec = open_gif_bytes(&bytes);

    let f1 = dec.next_frame().expect("ok").expect("frame1");
    assert_eq!(pixel(&f1.data, 0, 0, 2), [0xFF, 0x00, 0x00, 0xFF]);

    let f2 = dec.next_frame().expect("ok").expect("frame2");
    assert_eq!(pixel(&f2.data, 0, 0, 2), [0xFF, 0x00, 0x00, 0xFF]);
}

#[test]
fn background_disposal_output_is_not_premutated() {
    let color = [0x00_u8, 0xFF, 0x00];
    let bytes = make_gif(2, 2, color, gif::DisposalMethod::Background);
    let mut dec = open_gif_bytes(&bytes);

    let f1 = dec.next_frame().expect("ok").expect("frame1");
    assert_eq!(
        pixel(&f1.data, 0, 0, 2),
        [0x00, 0xFF, 0x00, 0xFF],
        "Background disposal must NOT pre-clear the output frame"
    );
}

#[test]
fn previous_disposal_output_is_not_premutated() {
    let color = [0x00_u8, 0x00, 0xFF];
    let bytes = make_gif(2, 2, color, gif::DisposalMethod::Previous);
    let mut dec = open_gif_bytes(&bytes);

    let f1 = dec.next_frame().expect("ok").expect("frame1");
    assert_eq!(
        pixel(&f1.data, 0, 0, 2),
        [0x00, 0x00, 0xFF, 0xFF],
        "Previous disposal must NOT restore canvas before outputting frame"
    );
}

#[test]
fn any_disposal_behaves_like_keep() {
    let color = [0xFF_u8, 0xFF, 0x00];
    let bytes = make_gif(2, 2, color, gif::DisposalMethod::Any);
    let mut dec = open_gif_bytes(&bytes);

    let f1 = dec.next_frame().expect("ok").expect("frame1");
    assert_eq!(pixel(&f1.data, 0, 0, 2), [0xFF, 0xFF, 0x00, 0xFF]);
}

#[test]
fn frame_duration_minimum_20ms() {
    let color = [0x80_u8, 0x80, 0x80];
    let mut buf = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut buf, 1, 1, &[]).expect("enc");
        encoder.set_repeat(gif::Repeat::Finite(0)).expect("repeat");
        let frame = gif::Frame {
            width: 1,
            height: 1,
            palette: Some(color.to_vec()),
            buffer: std::borrow::Cow::Owned(vec![0u8]),
            delay: 0,
            dispose: gif::DisposalMethod::Keep,
            transparent: None,
            ..Default::default()
        };
        encoder.write_frame(&frame).expect("frame");
    }
    let mut dec = open_gif_bytes(&buf);
    let f = dec.next_frame().expect("ok").expect("frame");
    assert!(
        f.duration_ms >= 20,
        "zero-delay frames must be clamped to ≥20ms, got {}",
        f.duration_ms
    );
}
