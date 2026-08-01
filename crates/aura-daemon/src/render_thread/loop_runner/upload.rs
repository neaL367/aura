use std::path::Path;

use aura_media::{ImageDecoder, MediaDecoder};
use aura_vulkan::{VulkanContext, monitor_renderer::MonitorRenderer};

pub fn load_and_upload_static_image(
    path: &Path,
    renderer: &mut MonitorRenderer,
    context: &VulkanContext,
) {
    match ImageDecoder::open(path) {
        Ok(mut decoder) => match decoder.next_frame() {
            Ok(Some(frame)) => {
                let w = frame.width;
                let h = frame.height;
                let res = renderer.set_wallpaper_pixels(context, w, h, &frame.data);
                drop(frame);
                drop(decoder);
                if let Err(e) = res {
                    tracing::warn!("Texture upload failed for {:?}: {}", path, e);
                } else {
                    tracing::info!("Texture upload succeeded for {:?} ({}x{})", path, w, h);
                    renderer.trim_staging(context);
                }
            }
            Ok(None) => tracing::warn!("ImageDecoder produced no frames for {:?}", path),
            Err(e) => tracing::warn!("ImageDecoder next_frame error for {:?}: {}", path, e),
        },
        Err(e) => tracing::warn!("Failed to open image {:?}: {}", path, e),
    }
}
