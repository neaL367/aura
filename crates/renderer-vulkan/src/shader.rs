//! SPIR-V shader binaries embedded at compile time for the full-screen wallpaper quad.

/// Vertex shader SPIR-V bytecode for the wallpaper quad.
pub fn vertex_shader_spv() -> &'static [u8] {
    include_bytes!("../../../resources/shaders/quad.vert.spv")
}

/// Fragment shader SPIR-V bytecode for the wallpaper textured quad.
pub fn fragment_shader_spv() -> &'static [u8] {
    include_bytes!("../../../resources/shaders/quad.frag.spv")
}
