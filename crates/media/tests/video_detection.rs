use std::path::Path;

use aura_media::is_video_by_extension;

#[test]
fn video_extensions() {
    for ext in &["mp4", "mkv", "avi", "mov", "wmv", "webm"] {
        assert!(
            is_video_by_extension(Path::new(&format!("test.{ext}"))),
            "expected true for .{ext}",
        );
    }
}

#[test]
fn non_video_extensions() {
    for ext in &["png", "jpg", "gif", "txt", "pdf"] {
        assert!(
            !is_video_by_extension(Path::new(&format!("test.{ext}"))),
            "expected false for .{ext}",
        );
    }
}

#[test]
fn case_insensitive() {
    assert!(is_video_by_extension(Path::new("test.MP4")));
    assert!(is_video_by_extension(Path::new("test.Mkv")));
    assert!(is_video_by_extension(Path::new("test.AVI")));
}

#[test]
fn no_extension() {
    assert!(!is_video_by_extension(Path::new("test")));
    assert!(!is_video_by_extension(Path::new("")));
}
