use aura_media::ImageDecoder;
use aura_media::MediaDecoder;

#[test]
fn image_decoder_nonexistent_file() {
    let path = std::path::Path::new("C:/nonexistent/path/to/image.png");
    let result = ImageDecoder::open(path);
    assert!(result.is_err(), "expected error for nonexistent file");
}

#[test]
fn image_decoder_empty_file() {
    let dir = std::env::temp_dir().join("aura-img-err-empty");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("empty.png");
    std::fs::write(&path, b"").unwrap();

    let result = ImageDecoder::open(&path);
    assert!(result.is_err(), "expected error for empty file");

    std::fs::remove_dir_all(&dir).unwrap_or(());
}

#[test]
fn image_decoder_corrupt_png() {
    let dir = std::env::temp_dir().join("aura-img-err-corrupt");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("corrupt.png");
    // Write bytes that start like a PNG but have invalid content.
    std::fs::write(&path, &[137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 0, 0xff, 0xff, 0xff]).unwrap();

    let mut decoder = ImageDecoder::open(&path);
    // Depending on the image crate, it might fail at open or at next_frame.
    if let Ok(ref mut d) = decoder {
        let result = d.next_frame();
        assert!(result.is_err() || result.unwrap().is_none(), "expected error or None for corrupt PNG");
    }

    std::fs::remove_dir_all(&dir).unwrap_or(());
}
