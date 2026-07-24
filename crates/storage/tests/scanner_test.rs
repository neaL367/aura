use aura_core::wallpaper::MediaKind;
use aura_storage::LibraryScanner;
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

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
    let dir = tempdir().expect("failed to create temp dir");

    let img_path = dir.path().join("test_image.png");
    let gif_path = dir.path().join("test_animation.gif");
    let txt_path = dir.path().join("document.txt");

    // Write valid media files
    File::create(&img_path)
        .unwrap()
        .write_all(MINIMAL_PNG)
        .unwrap();
    File::create(&gif_path)
        .unwrap()
        .write_all(MINIMAL_GIF)
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

#[test]
fn test_library_scanner_rejects_corrupt_images() {
    let dir = tempdir().expect("failed to create temp dir");

    let corrupt_img_path = dir.path().join("corrupt_image.png");
    let corrupt_gif_path = dir.path().join("corrupt_animation.gif");
    let truncated_gif_path = dir.path().join("truncated_header.gif");

    // Write corrupt media files (matching extension but invalid image content)
    File::create(&corrupt_img_path)
        .unwrap()
        .write_all(b"not a valid png file header or content")
        .unwrap();
    File::create(&corrupt_gif_path)
        .unwrap()
        .write_all(b"invalid gif content without header")
        .unwrap();
    File::create(&truncated_gif_path)
        .unwrap()
        .write_all(b"GIF89a") // header only, missing dimension bytes
        .unwrap();

    let meta_img = LibraryScanner::inspect_file(&corrupt_img_path);
    assert!(meta_img.is_none(), "corrupt PNG should be rejected");

    let meta_gif = LibraryScanner::inspect_file(&corrupt_gif_path);
    assert!(meta_gif.is_none(), "corrupt GIF should be rejected");

    let meta_trunc = LibraryScanner::inspect_file(&truncated_gif_path);
    assert!(
        meta_trunc.is_none(),
        "truncated GIF header should be rejected"
    );

    let scanned = LibraryScanner::scan_paths(&[dir.path().to_path_buf()]);
    assert!(
        scanned.is_empty(),
        "directory scan should exclude corrupt images"
    );
}
