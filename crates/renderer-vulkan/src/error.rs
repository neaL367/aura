use thiserror::Error;

#[derive(Debug, Error)]
pub enum VulkanError {
    #[error("Vulkan error: {0}")]
    Vk(#[from] ash::vk::Result),

    #[error("no suitable physical device found")]
    NoSuitableDevice,

    #[error("required extension not available: {0}")]
    MissingExtension(&'static str),

    #[error("surface creation failed")]
    SurfaceCreation,

    #[error("swapchain out of date")]
    SwapchainOutOfDate,

    #[error("device lost")]
    DeviceLost,

    #[error("out of memory")]
    OutOfMemory,

    #[error("allocation error: {0}")]
    Allocation(String),

    #[error("shader compilation failed: {0}")]
    ShaderCompilation(String),
}
