use thiserror::Error;

#[derive(Debug, Error)]
pub enum VulkanError {
    #[error("Vulkan error: {0}")]
    Vk(#[from] ash::vk::Result),

    #[error("No suitable physical device found")]
    NoSuitableDevice,

    #[error("Required extension not available: {0}")]
    MissingExtension(&'static str),

    #[error("Surface error: {0}")]
    Surface(String),

    #[error("Swapchain error: {0}")]
    Swapchain(String),

    #[error("Pipeline error: {0}")]
    Pipeline(String),

    #[error("Texture error: {0}")]
    Texture(String),

    #[error("Upload error: {0}")]
    Upload(String),

    #[error("Frame synchronization error: {0}")]
    FrameSync(String),

    #[error("Rendering error: {0}")]
    Render(String),

    #[error("Win32 error: {0}")]
    Win32(String),

    #[error("Swapchain out of date")]
    SwapchainOutOfDate,

    #[error("Device lost")]
    DeviceLost,

    #[error("Out of memory")]
    OutOfMemory,

    #[error("Allocation error: {0}")]
    Allocation(String),

    #[error("Shader compilation failed: {0}")]
    ShaderCompilation(String),
}
