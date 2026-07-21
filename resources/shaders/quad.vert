#version 450

layout(location = 0) out vec2 outUV;

// Full-screen quad positions (2 triangles, 6 vertices)
const vec2 POSITIONS[6] = vec2[](
    vec2(-1.0, -1.0),
    vec2( 1.0, -1.0),
    vec2(-1.0,  1.0),
    vec2(-1.0,  1.0),
    vec2( 1.0, -1.0),
    vec2( 1.0,  1.0)
);

// Corresponding UV texture coordinates
const vec2 UVS[6] = vec2[](
    vec2(0.0, 0.0),
    vec2(1.0, 0.0),
    vec2(0.0, 1.0),
    vec2(0.0, 1.0),
    vec2(1.0, 0.0),
    vec2(1.0, 1.0)
);

void main() {
    gl_Position = vec4(POSITIONS[gl_VertexIndex], 0.0, 1.0);
    outUV = UVS[gl_VertexIndex];
}
