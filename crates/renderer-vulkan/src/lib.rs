//! `aura-renderer-vulkan` — Vulkan rendering pipeline.
//!
//! One `VulkanContext` per process. One `MonitorRenderer` per active monitor.
//! All mutable state is owned by the render thread.

pub mod context;
pub mod error;
pub mod frame;
pub mod monitor_renderer;
pub mod pipeline;
pub mod shader;
pub mod surface;
pub mod swapchain;
pub mod texture;
pub mod upload;

pub use context::VulkanContext;
pub use error::VulkanError;
pub use monitor_renderer::MonitorRenderer;
