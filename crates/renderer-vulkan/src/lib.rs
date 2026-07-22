//! `aura-renderer-vulkan` — Vulkan rendering pipeline.
//!
//! One `VulkanContext` per process. One `MonitorRenderer` per active monitor.
//! All mutable state is owned by the render thread.

#[cfg(target_os = "windows")]
pub mod context;
#[cfg(target_os = "windows")]
pub mod error;
#[cfg(target_os = "windows")]
pub mod frame;
#[cfg(target_os = "windows")]
pub mod monitor_renderer;
#[cfg(target_os = "windows")]
pub mod pipeline;
#[cfg(target_os = "windows")]
pub mod shader;
#[cfg(target_os = "windows")]
pub mod surface;
#[cfg(target_os = "windows")]
pub mod swapchain;
#[cfg(target_os = "windows")]
pub mod texture;
#[cfg(target_os = "windows")]
pub use context::VulkanContext;
#[cfg(target_os = "windows")]
pub use error::VulkanError;
#[cfg(target_os = "windows")]
pub use frame::FrameSync;
#[cfg(target_os = "windows")]
pub use monitor_renderer::MonitorRenderer;
#[cfg(target_os = "windows")]
pub use pipeline::GraphicsPipeline;
#[cfg(target_os = "windows")]
pub use surface::Surface;
#[cfg(target_os = "windows")]
pub use swapchain::Swapchain;
#[cfg(target_os = "windows")]
pub use texture::GpuTexture;

// Stubs for non-Windows platforms (e.g. Linux CI check/test)
#[cfg(not(target_os = "windows"))]
pub mod stub {
    use thiserror::Error;

    #[derive(Debug, Clone, Error)]
    pub enum VulkanError {
        #[error("Not supported on this platform")]
        NotSupported,
    }

    pub struct VulkanContext;
    impl VulkanContext {
        pub fn new() -> Result<Self, VulkanError> {
            Err(VulkanError::NotSupported)
        }
    }

    pub struct MonitorRenderer;
}
#[cfg(not(target_os = "windows"))]
pub use stub::*;
