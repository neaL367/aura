use std::path::Path;

use aura_core::wallpaper::{MediaKind, WallpaperId, WallpaperMeta};
use aura_ipc::protocol::WallpaperEntry;

fn make_meta(path: &str, kind: MediaKind) -> WallpaperMeta {
    WallpaperMeta {
        id: WallpaperId::from_path(Path::new(path)),
        path: Path::new(path).to_path_buf(),
        kind,
        width: 1920,
        height: 1080,
        duration_ms: 0,
        file_size: 1024,
        scanned_at: "UNIX-12345".into(),
    }
}

#[test]
fn from_image_meta() {
    let meta = make_meta("test.png", MediaKind::Image);
    let entry = WallpaperEntry::from(&meta);
    assert_eq!(entry.id, meta.id);
    assert_eq!(entry.path, meta.path);
    assert_eq!(entry.kind, MediaKind::Image);
    assert!(entry.thumbnail_path.is_none());
}

#[test]
fn from_gif_meta() {
    let meta = make_meta("test.gif", MediaKind::Gif);
    let entry = WallpaperEntry::from(&meta);
    assert_eq!(entry.kind, MediaKind::Gif);
}

#[test]
fn from_video_meta() {
    let meta = make_meta("test.mp4", MediaKind::Video);
    let entry = WallpaperEntry::from(&meta);
    assert_eq!(entry.kind, MediaKind::Video);
}

#[test]
fn from_meta_preserves_path() {
    let meta = make_meta("C:/Wallpapers/bg.png", MediaKind::Image);
    let entry = WallpaperEntry::from(&meta);
    assert_eq!(entry.path, std::path::Path::new("C:/Wallpapers/bg.png"));
}

#[test]
fn from_meta_thumbnail_default_none() {
    let meta = make_meta("test.jpg", MediaKind::Image);
    let entry = WallpaperEntry::from(&meta);
    assert_eq!(entry.thumbnail_path, None);
}
