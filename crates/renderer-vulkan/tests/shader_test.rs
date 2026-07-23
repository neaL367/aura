use aura_renderer_vulkan::shader;

const SPIRV_MAGIC: [u8; 4] = [0x03, 0x02, 0x23, 0x07];

#[test]
fn vertex_shader_starts_with_spirv_magic() {
    let spv = shader::vertex_shader_spv();
    assert!(spv.len() >= 4, "vertex shader too short");
    assert_eq!(spv[0..4], SPIRV_MAGIC, "vertex shader missing SPIR-V magic");
}

#[test]
fn fragment_shader_starts_with_spirv_magic() {
    let spv = shader::fragment_shader_spv();
    assert!(spv.len() >= 4, "fragment shader too short");
    assert_eq!(spv[0..4], SPIRV_MAGIC, "fragment shader missing SPIR-V magic");
}

#[test]
fn vertex_shader_non_empty() {
    assert!(!shader::vertex_shader_spv().is_empty());
}

#[test]
fn fragment_shader_non_empty() {
    assert!(!shader::fragment_shader_spv().is_empty());
}
