use std::path::Path;

/// Detect whether a file is a video by inspecting its extension.
pub fn is_video_by_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm")
    )
}
