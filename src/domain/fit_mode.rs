use serde::{Deserialize, Serialize};

/// Defines how an image or video wallpaper is sized and positioned on a monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitMode {
    /// Scale the content to fill the screen, cropping if the aspect ratios differ.
    Fill,
    /// Scale the content to fit the screen entirely, leaving black bars (letterbox/pillarbox) if needed.
    Fit,
    /// Stretch the content in both dimensions to match the screen size exactly, disregarding aspect ratio.
    Stretch,
    /// Center the content at its native resolution, without scaling.
    Center,
}
