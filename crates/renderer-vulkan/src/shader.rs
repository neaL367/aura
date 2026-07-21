//! SPIR-V shader blobs compiled at build time by `build.rs` via `glslc`.
//!
//! The `build.rs` script invokes:
//! ```text
//! $VULKAN_SDK/Bin/glslc.exe src/shaders/wallpaper.vert -o out/wallpaper.vert.spv
//! $VULKAN_SDK/Bin/glslc.exe src/shaders/wallpaper.frag -o out/wallpaper.frag.spv
//! ```
//!
//! The compiled blobs are embedded at compile time via `include_bytes!`.

/// Vertex shader SPIR-V for the wallpaper quad.
pub fn vertex_shader_spv() -> &'static [u8] {
    // TODO: uncomment when build.rs generates these files
    // include_bytes!(concat!(env!("OUT_DIR"), "/wallpaper.vert.spv"))
    &[]
}

/// Fragment shader SPIR-V for the wallpaper textured quad.
pub fn fragment_shader_spv() -> &'static [u8] {
    // TODO: uncomment when build.rs generates these files
    // include_bytes!(concat!(env!("OUT_DIR"), "/wallpaper.frag.spv"))
    &[]
}
