use std::path::Path;

use aura_core::wallpaper::{MediaKind, detect_media_kind};

#[test]
fn detect_gif() {
    assert_eq!(detect_media_kind(Path::new("test.gif")), Some(MediaKind::Gif));
}

#[test]
fn detect_image_extensions() {
    for ext in &["png", "jpg", "jpeg", "bmp", "tiff", "tif", "webp"] {
        assert_eq!(
            detect_media_kind(Path::new(&format!("test.{}", ext))),
            Some(MediaKind::Image),
            "expected Image for .{ext}",
        );
    }
}

#[test]
fn detect_video_extensions() {
    for ext in &["mp4", "mkv", "avi", "mov", "wmv", "webm"] {
        assert_eq!(
            detect_media_kind(Path::new(&format!("test.{}", ext))),
            Some(MediaKind::Video),
            "expected Video for .{ext}",
        );
    }
}

#[test]
fn detect_unknown_extension() {
    assert_eq!(detect_media_kind(Path::new("test.txt")), None);
    assert_eq!(detect_media_kind(Path::new("test")), None);
    assert_eq!(detect_media_kind(Path::new("")), None);
}

#[test]
fn detect_case_insensitive() {
    assert_eq!(detect_media_kind(Path::new("test.GIF")), Some(MediaKind::Gif));
    assert_eq!(detect_media_kind(Path::new("test.PNG")), Some(MediaKind::Image));
    assert_eq!(detect_media_kind(Path::new("test.MP4")), Some(MediaKind::Video));
}
