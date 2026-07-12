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
