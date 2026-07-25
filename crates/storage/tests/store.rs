use std::path::Path;
use std::sync::Arc;

use aura_core::config::AppConfig;
use aura_core::monitor::{MonitorAssignment, MonitorId};
use aura_core::wallpaper::{FitMode, MediaKind, WallpaperId, WallpaperMeta};
use aura_storage::{LibraryScanner, ThumbnailStore, library_store::LibraryStore};
use tempfile::tempdir;

#[test]
fn test_library_store_save_and_load_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("library.json");
    let store = LibraryStore::new(&path);

    let entries = vec![
        WallpaperMeta {
            id: WallpaperId::new(),
            path: Path::new("test.png").to_path_buf(),
            kind: MediaKind::Image,
            width: 1920,
            height: 1080,
            duration_ms: 0,
            file_size: 12345,
            scanned_at: "2026-01-15T10:00:00Z".to_string(),
        },
        WallpaperMeta {
            id: WallpaperId::from_path(Path::new("anim.gif")),
            path: Path::new("anim.gif").to_path_buf(),
            kind: MediaKind::Gif,
            width: 320,
            height: 240,
            duration_ms: 3000,
            file_size: 54321,
            scanned_at: "2026-01-15T10:00:01Z".to_string(),
        },
    ];

    store.save(&entries).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].id, entries[0].id);
    assert_eq!(loaded[0].path, entries[0].path);
    assert_eq!(loaded[0].kind, entries[0].kind);
    assert_eq!(loaded[1].id, entries[1].id);
}

#[test]
fn test_library_store_load_missing_file_returns_empty() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nonexistent.json");
    let store = LibraryStore::new(&path);
    let loaded = store.load().unwrap();
    assert!(loaded.is_empty());
}

#[test]
fn test_config_store_save_and_load_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("aura.toml");
    let store = aura_storage::config_store::ConfigStore::new(&path);

    let mut config = AppConfig::default();
    config.assignments.push(MonitorAssignment {
        monitor_id: MonitorId::from_device_path(r"\\.\DISPLAY1"),
        wallpaper_id: WallpaperId::new(),
        fit_mode: FitMode::Fill,
    });

    store.save(&config).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.version, config.version);
    assert_eq!(loaded.assignments.len(), 1);
    assert_eq!(
        loaded.assignments[0].monitor_id,
        config.assignments[0].monitor_id
    );
    assert_eq!(
        loaded.assignments[0].fit_mode,
        config.assignments[0].fit_mode
    );
}

#[test]
fn test_config_store_load_missing_creates_default() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("missing.toml");
    let store = aura_storage::config_store::ConfigStore::new(&path);
    let loaded = store.load().unwrap();
    assert_eq!(loaded.version, 1);
}

#[test]
fn test_thumbnail_get_or_create_for_image() {
    let dir = tempdir().unwrap();
    let img_path = dir.path().join("test.png");
    let mut img = image::RgbaImage::new(64, 64);
    for y in 0..64 {
        for x in 0..64 {
            img.put_pixel(x, y, image::Rgba([x as u8, y as u8, 128, 255]));
        }
    }
    img.save(&img_path).unwrap();

    let meta = WallpaperMeta {
        id: WallpaperId::from_path(&img_path),
        path: img_path,
        kind: MediaKind::Image,
        width: 64,
        height: 64,
        duration_ms: 0,
        file_size: 0,
        scanned_at: String::new(),
    };

    let thumb = ThumbnailStore::get_or_create_in(&meta, dir.path());
    assert!(thumb.is_some(), "expected thumbnail to be generated");
    let thumb_path = thumb.unwrap();
    assert!(thumb_path.exists(), "thumbnail file should exist on disk");

    let cached = ThumbnailStore::get_or_create_in(&meta, dir.path());
    assert!(cached.is_some(), "expected cached thumbnail on second call");
}

#[test]
fn test_thumbnail_concurrent_generation() {
    let dir = tempdir().unwrap();
    let img_path = dir.path().join("concurrent.png");
    let mut img = image::RgbaImage::new(32, 32);
    for y in 0..32 {
        for x in 0..32 {
            img.put_pixel(x, y, image::Rgba([200, 100, 50, 255]));
        }
    }
    img.save(&img_path).unwrap();

    let meta = Arc::new(WallpaperMeta {
        id: WallpaperId::from_path(&img_path),
        path: img_path,
        kind: MediaKind::Image,
        width: 32,
        height: 32,
        duration_ms: 0,
        file_size: 0,
        scanned_at: String::new(),
    });

    let dir_path = Arc::new(dir.path().to_path_buf());
    let mut handles = Vec::new();

    for _ in 0..4 {
        let meta_clone = meta.clone();
        let dir_clone = dir_path.clone();
        handles.push(std::thread::spawn(move || {
            ThumbnailStore::get_or_create_in(&meta_clone, &dir_clone)
        }));
    }

    for h in handles {
        let res = h.join().unwrap();
        assert!(
            res.is_some(),
            "concurrent thread should receive thumbnail path"
        );
    }
}

const MINIMAL_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x60, 0x60, 0x60, 0x00,
    0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

const MINIMAL_GIF: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0xFF, 0xFF, 0xFF,
    0x00, 0x00, 0x00, 0x21, 0xF9, 0x04, 0x04, 0x00, 0x00, 0x00, 0x00, 0x2C, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3B,
];

#[test]
fn test_library_scanner_discovers_media_files() {
    let dir = tempdir().unwrap();

    let img_path = dir.path().join("test_image.png");
    std::fs::write(&img_path, MINIMAL_PNG).unwrap();

    let gif_path = dir.path().join("test_animation.gif");
    std::fs::write(&gif_path, MINIMAL_GIF).unwrap();

    let txt_path = dir.path().join("document.txt");
    std::fs::write(&txt_path, b"text content").unwrap();

    let scanned = LibraryScanner::scan_paths(&[dir.path().to_path_buf()]);
    assert_eq!(scanned.len(), 2);
    let kinds: Vec<MediaKind> = scanned.iter().map(|s| s.kind).collect();
    assert!(kinds.contains(&MediaKind::Image));
    assert!(kinds.contains(&MediaKind::Gif));
}
