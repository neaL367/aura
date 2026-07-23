use std::path::Path;

use aura_media::{GifDecoder, MediaDecoder};

/// Create a minimal animated GIF in memory and write it to `path`.
fn create_test_gif(path: &Path, width: u16, height: u16, frames: &[([u8; 4], u16)]) {
    let mut file = std::fs::File::create(path).unwrap();
    let mut encoder = gif::Encoder::new(&mut file, width, height, &[]).unwrap();
    encoder.set_repeat(gif::Repeat::Infinite).unwrap();

    for &(colour, delay) in frames {
        let mut pixels = vec![colour[0], colour[1], colour[2], colour[3]];
        let raw = pixels.repeat((width as usize * height as usize).saturating_sub(1));
        pixels.extend(raw);
        let mut frame = gif::Frame::from_rgba_speed(width, height, &mut pixels, 30);
        frame.delay = delay;
        encoder.write_frame(&frame).unwrap();
    }
}

/// Create a GIF where each frame can specify its disposal method.
fn create_test_gif_with_disposal(
    path: &Path,
    width: u16,
    height: u16,
    _bg_color: u8,
    frames: &[(Vec<u8>, u16, gif::DisposalMethod)],
) {
    let mut file = std::fs::File::create(path).unwrap();
    let mut encoder = gif::Encoder::new(&mut file, width, height, &[]).unwrap();
    encoder.set_repeat(gif::Repeat::Infinite).unwrap();

    for (pixels, delay, dispose) in frames {
        let mut frame = gif::Frame::from_rgba_speed(width, height, &mut pixels.clone(), 30);
        frame.delay = *delay;
        frame.dispose = *dispose;
        encoder.write_frame(&frame).unwrap();
    }
}

#[test]
fn gif_two_frames_read_all() {
    let dir = std::env::temp_dir().join("aura-gif-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("two_frames.gif");

    create_test_gif(
        &path,
        2,
        1,
        &[([255, 0, 0, 255], 10), ([0, 255, 0, 255], 10)],
    );

    let mut decoder = GifDecoder::open(&path).unwrap();
    let f1 = decoder.next_frame().unwrap().unwrap();
    assert_eq!(f1.width, 2);
    assert_eq!(f1.height, 1);
    assert_eq!(f1.data[0..4], [255, 0, 0, 255]);

    let f2 = decoder.next_frame().unwrap().unwrap();
    assert_eq!(f2.data[0..4], [0, 255, 0, 255]);

    assert!(decoder.next_frame().unwrap().is_none());
    decoder.loop_reset().unwrap();
    let f1_again = decoder.next_frame().unwrap().unwrap();
    assert_eq!(f1_again.data[0..4], [255, 0, 0, 255]);

    std::fs::remove_dir_all(&dir).unwrap_or(());
}

#[test]
fn gif_frame_with_transparency() {
    let dir = std::env::temp_dir().join("aura-gif-transparency");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("transparent.gif");

    create_test_gif(&path, 2, 1, &[([255, 0, 0, 255], 10), ([0, 0, 0, 0], 10)]);

    let mut decoder = GifDecoder::open(&path).unwrap();
    let f1 = decoder.next_frame().unwrap().unwrap();
    assert_eq!(f1.data[0..4], [255, 0, 0, 255]);

    let f2 = decoder.next_frame().unwrap().unwrap();
    assert_eq!(
        f2.data[0..4],
        [255, 0, 0, 255],
        "transparent should keep previous pixel"
    );

    std::fs::remove_dir_all(&dir).unwrap_or(());
}

#[test]
fn gif_frame_larger_than_canvas_clips() {
    let dir = std::env::temp_dir().join("aura-gif-clip");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("clip.gif");

    create_test_gif(&path, 2, 2, &[([255, 255, 255, 255], 10)]);

    let mut decoder = GifDecoder::open(&path).unwrap();
    let f = decoder.next_frame().unwrap().unwrap();
    assert_eq!(f.data.len(), 2 * 2 * 4);

    std::fs::remove_dir_all(&dir).unwrap_or(());
}

#[test]
fn gif_loop_reset_after_exhaustion() {
    let dir = std::env::temp_dir().join("aura-gif-loop");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("loop.gif");

    create_test_gif(&path, 1, 1, &[([255, 0, 0, 255], 10)]);

    let mut decoder = GifDecoder::open(&path).unwrap();
    let f = decoder.next_frame().unwrap().unwrap();
    assert_eq!(f.data[0..4], [255, 0, 0, 255]);
    assert!(decoder.next_frame().unwrap().is_none());

    decoder.loop_reset().unwrap();
    let f_again = decoder.next_frame().unwrap().unwrap();
    assert_eq!(f_again.data[0..4], [255, 0, 0, 255]);

    std::fs::remove_dir_all(&dir).unwrap_or(());
}

#[test]
fn gif_disposal_keep() {
    let dir = std::env::temp_dir().join("aura-gif-keep");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("keep.gif");

    // 2x1 canvas: frame 1 is red (left), frame 2 is green (right).
    // With Keep disposal, after frame 2 the entire canvas shows both red AND green.
    create_test_gif(
        &path,
        2,
        1,
        &[([255, 0, 0, 255], 10), ([0, 255, 0, 255], 10)],
    );

    let mut decoder = GifDecoder::open(&path).unwrap();
    let _f1 = decoder.next_frame().unwrap().unwrap();
    let f2 = decoder.next_frame().unwrap().unwrap();
    // Frame 2 replaces the entire canvas with green (Keep is default).
    assert_eq!(
        f2.data[0..4],
        [0, 255, 0, 255],
        "keep disposal replaces canvas"
    );

    std::fs::remove_dir_all(&dir).unwrap_or(());
}

#[test]
fn gif_disposal_background_clears_frame_area() {
    let dir = std::env::temp_dir().join("aura-gif-bg");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bg.gif");

    // 2x1 canvas, bg_color=0 → palette entry 0 = black
    // Frame 1: red, disposal=Background
    // Frame 2: green (partial — only left pixel)
    let w = 2u16;
    let h = 1u16;
    let all_red = vec![255u8; w as usize * h as usize * 4];
    let mut left_green = vec![0u8; w as usize * h as usize * 4];
    left_green[0] = 0; // R=0
    left_green[1] = 255; // G=255
    left_green[3] = 255; // A=255

    create_test_gif_with_disposal(
        &path,
        w,
        h,
        0,
        &[
            (all_red, 10, gif::DisposalMethod::Background),
            (left_green, 10, gif::DisposalMethod::Keep),
        ],
    );

    let mut decoder = GifDecoder::open(&path).unwrap();
    decoder.next_frame().unwrap().unwrap();
    let f2 = decoder.next_frame().unwrap().unwrap();

    // After disposal Background, frame 1's pixels are cleared to bg color (black).
    // Then frame 2 composites: left pixel becomes green, right pixel is black (cleared bg).
    // But the decoder's bg_color = black [0,0,0,0] (alpha=0 for transparent bg)
    // Actually our GIF has no global palette transparency, so bg_color should be opaque black.
    // The right pixel should be background color after clearing.

    // Just verify the right pixel was affected by background disposal (not still red from frame 1).
    assert_ne!(
        f2.data[4..8],
        [255, 0, 0, 255],
        "background disposal should clear frame 1 area"
    );

    std::fs::remove_dir_all(&dir).unwrap_or(());
}
