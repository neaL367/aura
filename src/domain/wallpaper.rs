use serde::{Deserialize, Serialize};

/// The type of media content used for a wallpaper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallpaperType {
    Image,
    Video,
}

/// Unique identifier for a wallpaper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WallpaperId(pub String);
