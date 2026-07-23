use std::path::Path;

use aura_core::wallpaper::MediaKind;
use aura_media::{DecodedFrame, GifDecoder, ImageDecoder, MediaDecoder, frame_channel};

fn create_test_png(path: &Path) {
    let mut img = image::RgbaImage::new(4, 4);
    for y in 0..4 {
        for x in 0..4 {
            let r = (x * 64) as u8;
            let g = (y * 64) as u8;
            img.put_pixel(x, y, image::Rgba([r, g, 128, 255]));
        }
    }
    img.save(path).unwrap();
}

fn create_test_gif(path: &Path) {
    let mut gif = std::fs::File::create(path).unwrap();
    let mut encoder = gif::Encoder::new(&mut gif, 4, 4, &[]).unwrap();
    encoder.set_repeat(gif::Repeat::Infinite).unwrap();

    let mut pixels = vec![0u8; 4 * 4 * 4];
    for i in (0..pixels.len()).step_by(4) {
        pixels[i] = 0xFF;
        pixels[i + 3] = 0xFF;
    }
    let frame = gif::Frame::from_rgba(4, 4, &mut pixels);
    encoder.write_frame(&frame).unwrap();

    let mut pixels2 = vec![0u8; 4 * 4 * 4];
    for i in (0..pixels2.len()).step_by(4) {
        pixels2[i + 1] = 0xFF;
        pixels2[i + 3] = 0xFF;
    }
    let frame2 = gif::Frame::from_rgba(4, 4, &mut pixels2);
    encoder.write_frame(&frame2).unwrap();
}

#[test]
fn test_image_decoder_open_and_decode() {
    let dir = std::env::temp_dir().join("aura_test_image_decoder");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test.png");
    create_test_png(&path);

    let mut decoder = ImageDecoder::open(&path).unwrap();
    assert_eq!(decoder.width(), 4);
    assert_eq!(decoder.height(), 4);

    let frame = decoder.next_frame().unwrap().expect("expected a frame");
    assert_eq!(frame.width, 4);
    assert_eq!(frame.height, 4);
    assert_eq!(frame.data.len(), 4 * 4 * 4);
    assert_eq!(frame.duration_ms, 0);

    assert!(decoder.next_frame().unwrap().is_none());

    decoder.loop_reset().unwrap();
    let frame_again = decoder
        .next_frame()
        .unwrap()
        .expect("expected frame after reset");
    assert_eq!(frame_again.data, frame.data);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_gif_decoder_open_and_decode() {
    let dir = std::env::temp_dir().join("aura_test_gif_decoder");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test.gif");
    create_test_gif(&path);

    let mut decoder = GifDecoder::open(&path).unwrap();
    assert_eq!(decoder.width(), 4);
    assert_eq!(decoder.height(), 4);

    let frame1 = decoder.next_frame().unwrap().expect("expected frame 1");
    assert_eq!(frame1.width, 4);
    assert_eq!(frame1.height, 4);
    assert!(frame1.duration_ms >= 20);

    let frame2 = decoder.next_frame().unwrap().expect("expected frame 2");
    assert_eq!(frame2.width, 4);
    assert_eq!(frame2.height, 4);
    assert_ne!(frame1.data, frame2.data);

    decoder.loop_reset().unwrap();
    let frame1_again = decoder
        .next_frame()
        .unwrap()
        .expect("expected frame after loop reset");
    assert_eq!(frame1.data, frame1_again.data);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_frame_channel_basic() {
    let (tx, rx) = frame_channel();
    let frame = DecodedFrame {
        width: 2,
        height: 2,
        data: vec![255u8; 16],
        timestamp_ms: 0,
        duration_ms: 0,
    };

    assert!(tx.try_send(frame.clone()));
    let received = rx.try_recv().expect("expected frame");
    assert_eq!(received.data, frame.data);
    assert!(rx.try_recv().is_none());
}

#[test]
fn test_frame_channel_capacity() {
    let (tx, rx) = frame_channel();
    let frame = DecodedFrame {
        width: 1,
        height: 1,
        data: vec![0u8; 4],
        timestamp_ms: 0,
        duration_ms: 0,
    };

    for _ in 0..3 {
        assert!(tx.try_send(frame.clone()));
    }
    assert!(!tx.try_send(frame.clone()));
    drop(rx);
    assert!(!tx.send_blocking(frame));
}

#[test]
fn test_media_kind_detection_by_extension() {
    assert_eq!(
        detect_media_kind(Path::new("wallpaper.gif")),
        Some(MediaKind::Gif)
    );
    assert_eq!(
        detect_media_kind(Path::new("photo.png")),
        Some(MediaKind::Image)
    );
    assert_eq!(
        detect_media_kind(Path::new("picture.JPG")),
        Some(MediaKind::Image)
    );
    assert_eq!(
        detect_media_kind(Path::new("video.mp4")),
        Some(MediaKind::Video)
    );
    assert_eq!(detect_media_kind(Path::new("document.pdf")), None);
}

fn detect_media_kind(path: &Path) -> Option<MediaKind> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("gif") => Some(MediaKind::Gif),
        Some("png" | "jpg" | "jpeg" | "bmp" | "tiff" | "tif" | "webp") => Some(MediaKind::Image),
        Some("mp4" | "webm" | "mkv" | "avi") => Some(MediaKind::Video),
        _ => None,
    }
}
