use windows::core::{PCSTR, s};
use windows::Win32::Graphics::Direct3D::ID3DBlob;
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Texture2D,
    ID3D11VertexShader, ID3D11PixelShader, ID3D11InputLayout, ID3D11Buffer, ID3D11SamplerState,
    D3D11_INPUT_ELEMENT_DESC, D3D11_BUFFER_DESC, D3D11_SUBRESOURCE_DATA,
    D3D11_BIND_VERTEX_BUFFER, D3D11_USAGE_DEFAULT, D3D11_SAMPLER_DESC,
    D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_TEXTURE_ADDRESS_CLAMP,
    D3D11_COMPARISON_NEVER, D3D11_VIEWPORT, D3D11_INPUT_PER_VERTEX_DATA,
    ID3D11ShaderResourceView,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R32G32_FLOAT;
use crate::domain::fit_mode::FitMode;
use crate::utils::error::{AppError, Result};

const SHADER_SOURCE: &str = r#"
struct VS_INPUT {
    float2 pos : POSITION;
    float2 tex : TEXCOORD;
};

struct PS_INPUT {
    float4 pos : SV_POSITION;
    float2 tex : TEXCOORD;
};

PS_INPUT vs_main(VS_INPUT input) {
    PS_INPUT output;
    output.pos = float4(input.pos, 0.0f, 1.0f);
    output.tex = input.tex;
    return output;
}

Texture2D txColor : register(t0);
SamplerState samLinear : register(s0);

float4 ps_main(PS_INPUT input) : SV_Target {
    return txColor.Sample(samLinear, input.tex);
}
"#;

#[repr(C)]
struct Vertex {
    pos: [f32; 2],
    tex: [f32; 2],
}

pub struct TextureRenderer {
    _vertex_shader: ID3D11VertexShader,
    _pixel_shader: ID3D11PixelShader,
    input_layout: ID3D11InputLayout,
    vertex_buffer: ID3D11Buffer,
    sampler_state: ID3D11SamplerState,
}

impl TextureRenderer {
    /// Compiles shaders and initializes the vertex buffer and pipeline state.
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        // 1. Compile Shaders
        let vs_blob = compile_shader(SHADER_SOURCE, s!("vs_main"), s!("vs_5_0"))?;
        let ps_blob = compile_shader(SHADER_SOURCE, s!("ps_main"), s!("ps_5_0"))?;

        // 2. Create VS and PS
        let vs_bytes = unsafe {
            std::slice::from_raw_parts(vs_blob.GetBufferPointer() as *const u8, vs_blob.GetBufferSize())
        };
        let ps_bytes = unsafe {
            std::slice::from_raw_parts(ps_blob.GetBufferPointer() as *const u8, ps_blob.GetBufferSize())
        };

        let mut vertex_shader = None;
        let mut pixel_shader = None;
        unsafe {
            device.CreateVertexShader(vs_bytes, None, Some(&mut vertex_shader))?;
            device.CreatePixelShader(ps_bytes, None, Some(&mut pixel_shader))?;
        }
        let vertex_shader = vertex_shader.ok_or_else(|| {
            AppError::Renderer("Failed to create Vertex Shader".to_string())
        })?;
        let pixel_shader = pixel_shader.ok_or_else(|| {
            AppError::Renderer("Failed to create Pixel Shader".to_string())
        })?;

        // 3. Create Input Layout
        let layout_desc = [
            D3D11_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(b"POSITION\0".as_ptr()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 0,
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D11_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(b"TEXCOORD\0".as_ptr()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 8, // size of float2 (2 * 4 bytes)
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
        ];

        let mut input_layout = None;
        unsafe {
            device.CreateInputLayout(&layout_desc, vs_bytes, Some(&mut input_layout))?;
        }
        let input_layout = input_layout.ok_or_else(|| {
            AppError::Renderer("Failed to create Input Layout".to_string())
        })?;

        // 4. Create Vertex Buffer (Full screen quad using triangle strip)
        let vertices = [
            Vertex { pos: [-1.0,  1.0], tex: [0.0, 0.0] }, // Top-Left
            Vertex { pos: [ 1.0,  1.0], tex: [1.0, 0.0] }, // Top-Right
            Vertex { pos: [-1.0, -1.0], tex: [0.0, 1.0] }, // Bottom-Left
            Vertex { pos: [ 1.0, -1.0], tex: [1.0, 1.0] }, // Bottom-Right
        ];

        let buffer_desc = D3D11_BUFFER_DESC {
            ByteWidth: std::mem::size_of_val(&vertices) as u32,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
            StructureByteStride: 0,
        };

        let init_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: vertices.as_ptr() as *const _,
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        };

        let mut vertex_buffer = None;
        unsafe {
            device.CreateBuffer(&buffer_desc, Some(&init_data), Some(&mut vertex_buffer))?;
        }
        let vertex_buffer = vertex_buffer.ok_or_else(|| {
            AppError::Renderer("Failed to create Vertex Buffer".to_string())
        })?;

        // 5. Create Sampler State
        let sampler_desc = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            MipLODBias: 0.0,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            BorderColor: [0.0, 0.0, 0.0, 0.0],
            MinLOD: 0.0,
            MaxLOD: f32::MAX,
        };

        let mut sampler_state = None;
        unsafe {
            device.CreateSamplerState(&sampler_desc, Some(&mut sampler_state))?;
        }
        let sampler_state = sampler_state.ok_or_else(|| {
            AppError::Renderer("Failed to create Sampler State".to_string())
        })?;

        Ok(Self {
            _vertex_shader: vertex_shader,
            _pixel_shader: pixel_shader,
            input_layout,
            vertex_buffer,
            sampler_state,
        })
    }

    /// Helper to compute viewport size and offsets based on FitMode.
    /// Safe against zero or negative dimensions, returning default screen mappings.
    pub fn calculate_viewport(
        fit_mode: FitMode,
        tex_w: f32,
        tex_h: f32,
        mon_w: f32,
        mon_h: f32,
    ) -> (f32, f32, f32, f32) {
        if tex_w <= 0.0 || tex_h <= 0.0 || mon_w <= 0.0 || mon_h <= 0.0 {
            return (mon_w, mon_h, 0.0, 0.0);
        }
        match fit_mode {
            FitMode::Stretch => (mon_w, mon_h, 0.0, 0.0),
            FitMode::Fit => {
                let scale = (mon_w / tex_w).min(mon_h / tex_h);
                let w = tex_w * scale;
                let h = tex_h * scale;
                (w, h, (mon_w - w) / 2.0, (mon_h - h) / 2.0)
            }
            FitMode::Fill => {
                let scale = (mon_w / tex_w).max(mon_h / tex_h);
                let w = tex_w * scale;
                let h = tex_h * scale;
                (w, h, (mon_w - w) / 2.0, (mon_h - h) / 2.0)
            }
            FitMode::Center => {
                (tex_w, tex_h, (mon_w - tex_w) / 2.0, (mon_h - tex_h) / 2.0)
            }
        }
    }

    /// Renders the texture onto the swapchain render target.
    /// Manages aspect ratio and position using D3D11 viewports based on the fit mode.
    pub fn render(
        &self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        rtv: &ID3D11RenderTargetView,
        texture: &ID3D11Texture2D,
        fit_mode: FitMode,
        monitor_width: u32,
        monitor_height: u32,
    ) -> Result<()> {
        // 1. Resolve Texture dimensions
        let mut tex_desc = windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.GetDesc(&mut tex_desc); }
        let tex_w = tex_desc.Width as f32;
        let tex_h = tex_desc.Height as f32;
        let mon_w = monitor_width as f32;
        let mon_h = monitor_height as f32;

        // 2. Perform Viewport scaling calculations based on FitMode
        let (vp_w, vp_h, vp_x, vp_y) = Self::calculate_viewport(fit_mode, tex_w, tex_h, mon_w, mon_h);

        let viewport = D3D11_VIEWPORT {
            TopLeftX: vp_x,
            TopLeftY: vp_y,
            Width: vp_w,
            Height: vp_h,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };

        // 3. Create a Shader Resource View (SRV) for the texture
        let mut srv = None;
        unsafe {
            device.CreateShaderResourceView(texture, None, Some(&mut srv))?;
        }
        let srv = srv.ok_or_else(|| {
            AppError::Renderer("Failed to create Shader Resource View".to_string())
        })?;

        // 4. Bind resources to the pipeline
        unsafe {
            // Set Render Targets
            context.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);

            // Clear render target (black bars for Fit/Center or background padding)
            context.ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 1.0]);

            // Set Viewport
            context.RSSetViewports(Some(&[viewport]));

            // Set Input Layout
            context.IASetInputLayout(&self.input_layout);

            // Set Primitive Topology
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);

            // Set Vertex Buffer
            let stride = std::mem::size_of::<Vertex>() as u32;
            let offset = 0u32;
            let buffers = [Some(self.vertex_buffer.clone())];
            context.IASetVertexBuffers(
                0,
                1,
                Some(buffers.as_ptr()),
                Some(&stride),
                Some(&offset),
            );

            // Set Shaders
            context.VSSetShader(&self._vertex_shader, None);
            context.PSSetShader(&self._pixel_shader, None);

            // Set Sampler
            context.PSSetSamplers(0, Some(&[Some(self.sampler_state.clone())]));

            // Set Texture Shader Resource
            context.PSSetShaderResources(0, Some(&[Some(srv)]));

            // Draw full-screen quad (4 vertices, triangle strip)
            context.Draw(4, 0);

            // Clean up bindings to avoid leaving dangling resources bound
            context.OMSetRenderTargets(None, None);
            let null_srvs: [Option<ID3D11ShaderResourceView>; 1] = [None];
            context.PSSetShaderResources(0, Some(&null_srvs));
        }

        Ok(())
    }
}

fn compile_shader(source: &str, entrypoint: PCSTR, target: PCSTR) -> Result<ID3DBlob> {
    let mut code: Option<ID3DBlob> = None;
    let mut error_msgs: Option<ID3DBlob> = None;

    let hr = unsafe {
        D3DCompile(
            source.as_ptr() as *const _,
            source.len(),
            PCSTR::null(),
            None,
            None,
            entrypoint,
            target,
            0,
            0,
            &mut code,
            Some(&mut error_msgs as *mut _),
        )
    };

    if hr.is_err() {
        if let Some(err_blob) = error_msgs {
            let buffer = unsafe { err_blob.GetBufferPointer() };
            let size = unsafe { err_blob.GetBufferSize() };
            let slice = unsafe { std::slice::from_raw_parts(buffer as *const u8, size) };
            let msg = String::from_utf8_lossy(slice);
            return Err(AppError::Renderer(format!("Shader compilation failed: {}", msg)));
        }
        return Err(AppError::Renderer("Shader compilation failed with no error messages".to_string()));
    }

    code.ok_or_else(|| AppError::Renderer("Compiled shader blob was null".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_near(val: f32, expected: f32) {
        assert!((val - expected).abs() < 0.01, "Expected {} to be near {}", val, expected);
    }

    #[test]
    fn test_calculate_viewport_stretch() {
        let (w, h, x, y) = TextureRenderer::calculate_viewport(FitMode::Stretch, 1920.0, 1080.0, 3840.0, 2160.0);
        assert_near(w, 3840.0);
        assert_near(h, 2160.0);
        assert_near(x, 0.0);
        assert_near(y, 0.0);
    }

    #[test]
    fn test_calculate_viewport_fit_letterbox() {
        // 16:9 Screen (1920x1080), 4:3 Texture (800x600)
        // Aspect scale factor matches height limit: mon_h / tex_h = 1080 / 600 = 1.8
        // w = 800 * 1.8 = 1440.0
        // h = 600 * 1.8 = 1080.0
        // x = (1920 - 1440) / 2.0 = 240.0
        let (w, h, x, y) = TextureRenderer::calculate_viewport(FitMode::Fit, 800.0, 600.0, 1920.0, 1080.0);
        assert_near(w, 1440.0);
        assert_near(h, 1080.0);
        assert_near(x, 240.0);
        assert_near(y, 0.0);
    }

    #[test]
    fn test_calculate_viewport_fit_pillarbox() {
        // 4:3 Screen (1024x768), 16:9 Texture (1920x1080)
        // Aspect scale factor matches width limit: mon_w / tex_w = 1024 / 1920 = 0.5333...
        // w = 1024.0
        // h = 1080 * (1024 / 1920) = 576.0
        // y = (768 - 576) / 2.0 = 96.0
        let (w, h, x, y) = TextureRenderer::calculate_viewport(FitMode::Fit, 1920.0, 1080.0, 1024.0, 768.0);
        assert_near(w, 1024.0);
        assert_near(h, 576.0);
        assert_near(x, 0.0);
        assert_near(y, 96.0);
    }

    #[test]
    fn test_calculate_viewport_fill_cropping() {
        // 16:9 Screen (1920x1080), 4:3 Texture (800x600)
        // Aspect scale factor matches width limit to fill: mon_w / tex_w = 1920 / 800 = 2.4
        // w = 800 * 2.4 = 1920.0
        // h = 600 * 2.4 = 1440.0
        // y = (1080 - 1440) / 2.0 = -180.0 (top and bottom cropped by 180px each)
        let (w, h, x, y) = TextureRenderer::calculate_viewport(FitMode::Fill, 800.0, 600.0, 1920.0, 1080.0);
        assert_near(w, 1920.0);
        assert_near(h, 1440.0);
        assert_near(x, 0.0);
        assert_near(y, -180.0);
    }

    #[test]
    fn test_calculate_viewport_center() {
        // 1920x1080 Screen, 800x600 Texture. Should place at native resolution centered
        // x = (1920 - 800) / 2 = 560
        // y = (1080 - 600) / 2 = 240
        let (w, h, x, y) = TextureRenderer::calculate_viewport(FitMode::Center, 800.0, 600.0, 1920.0, 1080.0);
        assert_near(w, 800.0);
        assert_near(h, 600.0);
        assert_near(x, 560.0);
        assert_near(y, 240.0);
    }

    #[test]
    fn test_calculate_viewport_local_coordinates() {
        // Check that passing a monitor size (e.g. 1920x1080) yields offsets relative to 0,
        // and does NOT translate by absolute virtual screen offsets (e.g. -1920, 0)
        let (_, _, x, y) = TextureRenderer::calculate_viewport(FitMode::Fit, 800.0, 600.0, 1920.0, 1080.0);
        assert!(x >= 0.0 && x < 1920.0, "X offset {} should be local inside the screen dimensions", x);
        assert!(y >= 0.0 && y < 1080.0, "Y offset {} should be local inside the screen dimensions", y);
    }

    #[test]
    fn test_calculate_viewport_odd_leftover_rounding() {
        // Texture width = 1921.0, Monitor width = 1920.0
        // For FitMode::Center, offset is (1920 - 1921) / 2 = -0.5 (f32)
        // Since we are using floats, assert that the sub-pixel coordinate of -0.5 is preserved
        let (w, h, x, y) = TextureRenderer::calculate_viewport(FitMode::Center, 1921.0, 1080.0, 1920.0, 1080.0);
        assert_near(w, 1921.0);
        assert_near(h, 1080.0);
        assert_near(x, -0.5);
        assert_near(y, 0.0);
    }

    #[test]
    fn test_calculate_viewport_identical_aspect_ratio() {
        // 16:9 Screen (1920x1080), 16:9 Texture (3840x2160)
        // Fit, Fill and Stretch should all produce identical viewport size matching monitor size
        let (w_fit, h_fit, _, _) = TextureRenderer::calculate_viewport(FitMode::Fit, 3840.0, 2160.0, 1920.0, 1080.0);
        let (w_fill, h_fill, _, _) = TextureRenderer::calculate_viewport(FitMode::Fill, 3840.0, 2160.0, 1920.0, 1080.0);
        let (w_str, h_str, _, _) = TextureRenderer::calculate_viewport(FitMode::Stretch, 3840.0, 2160.0, 1920.0, 1080.0);
        
        assert_near(w_fit, 1920.0);
        assert_near(h_fit, 1080.0);
        assert_near(w_fill, 1920.0);
        assert_near(h_fill, 1080.0);
        assert_near(w_str, 1920.0);
        assert_near(h_str, 1080.0);
    }

    #[test]
    fn test_calculate_viewport_extreme_aspect_ratio() {
        // Very wide texture (1920x100) on square monitor (1000x1000)
        // Fit should scale to fit width: scale = 1000 / 1920 = 0.52083
        // w = 1000, h = 52.083, y = 473.96
        let (w, h, x, y) = TextureRenderer::calculate_viewport(FitMode::Fit, 1920.0, 100.0, 1000.0, 1000.0);
        assert_near(w, 1000.0);
        assert_near(h, 52.083);
        assert_near(x, 0.0);
        assert_near(y, 473.96);

        // Fill should scale to fill height: scale = 1000 / 100 = 10.0
        // h = 1000, w = 19200, x = -9100.0
        let (w_fill, h_fill, x_fill, y_fill) = TextureRenderer::calculate_viewport(FitMode::Fill, 1920.0, 100.0, 1000.0, 1000.0);
        assert_near(w_fill, 19200.0);
        assert_near(h_fill, 1000.0);
        assert_near(x_fill, -9100.0);
        assert_near(y_fill, 0.0);
    }

    #[test]
    fn test_calculate_viewport_degenerate_sizes() {
        // Safe check for 0x0 sizes, returning default mappings (mon_w, mon_h) instead of NaN or Infinite
        let (w, h, x, y) = TextureRenderer::calculate_viewport(FitMode::Fit, 0.0, 600.0, 1920.0, 1080.0);
        assert_near(w, 1920.0);
        assert_near(h, 1080.0);
        assert_near(x, 0.0);
        assert_near(y, 0.0);

        let (w_max, h_max, _, _) = TextureRenderer::calculate_viewport(FitMode::Fit, 16384.0, 16384.0, 1920.0, 1080.0);
        assert_near(w_max, 1080.0);
        assert_near(h_max, 1080.0);
    }
}
