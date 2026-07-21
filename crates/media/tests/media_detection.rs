use aura_core::wallpaper::MediaKind;
use std::path::Path;

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

#[test]
fn test_media_kind_detection() {
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
