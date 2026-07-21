use aura_core::wallpaper::MediaKind;
use aura_storage::LibraryScanner;
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_library_scanner_discovers_media_files() {
    let dir = tempdir().expect("failed to create temp dir");

    let img_path = dir.path().join("test_image.png");
    let gif_path = dir.path().join("test_animation.gif");
    let txt_path = dir.path().join("document.txt");

    // Write dummy files
    File::create(&img_path)
        .unwrap()
        .write_all(b"fake png data")
        .unwrap();
    File::create(&gif_path)
        .unwrap()
        .write_all(b"GIF89a dummy gif data")
        .unwrap();
    File::create(&txt_path)
        .unwrap()
        .write_all(b"text content")
        .unwrap();

    let scanned = LibraryScanner::scan_paths(&[dir.path().to_path_buf()]);

    assert_eq!(scanned.len(), 2);
    let kinds: Vec<MediaKind> = scanned.iter().map(|s| s.kind).collect();
    assert!(kinds.contains(&MediaKind::Image));
    assert!(kinds.contains(&MediaKind::Gif));
}
