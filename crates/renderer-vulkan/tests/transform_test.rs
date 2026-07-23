use aura_core::wallpaper::FitMode;
use aura_renderer_vulkan::transform::calculate_uv_transform;

#[test]
fn zero_img_w_returns_identity() {
    let r = calculate_uv_transform(FitMode::Fill, 0, 1080, 1920, 1080);
    assert_eq!(r.uv_scale, [1.0, 1.0]);
    assert_eq!(r.uv_offset, [0.0, 0.0]);
}

#[test]
fn zero_img_h_returns_identity() {
    let r = calculate_uv_transform(FitMode::Fill, 1920, 0, 1920, 1080);
    assert_eq!(r.uv_scale, [1.0, 1.0]);
}

#[test]
fn zero_mon_w_returns_identity() {
    let r = calculate_uv_transform(FitMode::Fill, 1920, 1080, 0, 1080);
    assert_eq!(r.uv_scale, [1.0, 1.0]);
}

#[test]
fn zero_mon_h_returns_identity() {
    let r = calculate_uv_transform(FitMode::Fill, 1920, 1080, 1920, 0);
    assert_eq!(r.uv_scale, [1.0, 1.0]);
}

#[test]
fn stretch_always_identity() {
    let r = calculate_uv_transform(FitMode::Stretch, 4000, 1000, 1920, 1080);
    assert_eq!(r.uv_scale, [1.0, 1.0]);
    assert_eq!(r.uv_offset, [0.0, 0.0]);
}

#[test]
fn fill_wider_image_crops_horiz() {
    let r = calculate_uv_transform(FitMode::Fill, 4000, 1000, 1920, 1080);
    assert_eq!(r.uv_scale[0], 4.0 / 9.0);
    assert_eq!(r.uv_scale[1], 1.0);
    assert_eq!(r.uv_offset[0], (1.0 - 4.0 / 9.0) * 0.5);
    assert_eq!(r.uv_offset[1], 0.0);
}

#[test]
fn fill_taller_image_crops_vert() {
    let r = calculate_uv_transform(FitMode::Fill, 500, 1000, 1920, 1080);
    assert_eq!(r.uv_scale[0], 1.0);
    assert_eq!(r.uv_scale[1], 0.5 * 9.0 / 16.0);
    assert_eq!(r.uv_offset[0], 0.0);
    assert_eq!(r.uv_offset[1], (1.0 - 0.5 * 9.0 / 16.0) * 0.5);
}

#[test]
fn fill_same_aspect() {
    let r = calculate_uv_transform(FitMode::Fill, 1920, 1080, 1920, 1080);
    assert_eq!(r.uv_scale, [1.0, 1.0]);
    assert_eq!(r.uv_offset, [0.0, 0.0]);
}

#[test]
fn fit_wider_image_letterbox() {
    let r = calculate_uv_transform(FitMode::Fit, 4000, 1000, 1920, 1080);
    assert_eq!(r.uv_scale[0], 1.0);
    assert_eq!(r.uv_scale[1], 4.0 * 9.0 / 16.0);
    assert_eq!(r.uv_offset[0], 0.0);
    assert_eq!(r.uv_offset[1], (1.0 - 4.0 * 9.0 / 16.0) * 0.5);
}

#[test]
fn fit_taller_image_pillarbox() {
    let r = calculate_uv_transform(FitMode::Fit, 500, 1000, 1920, 1080);
    assert_eq!(r.uv_scale[0], 16.0 / 9.0 / 0.5);
    assert_eq!(r.uv_scale[1], 1.0);
    assert_eq!(r.uv_offset[0], (1.0 - 16.0 / 9.0 / 0.5) * 0.5);
    assert_eq!(r.uv_offset[1], 0.0);
}

#[test]
fn fit_same_aspect() {
    let r = calculate_uv_transform(FitMode::Fit, 1920, 1080, 1920, 1080);
    assert_eq!(r.uv_scale, [1.0, 1.0]);
    assert_eq!(r.uv_offset, [0.0, 0.0]);
}

#[test]
fn center_small_image() {
    let r = calculate_uv_transform(FitMode::Center, 500, 500, 1920, 1080);
    assert_eq!(r.uv_scale[0], 1920.0 / 500.0);
    assert_eq!(r.uv_scale[1], 1080.0 / 500.0);
    assert_eq!(r.uv_offset[0], (1.0 - 1920.0 / 500.0) * 0.5);
    assert_eq!(r.uv_offset[1], (1.0 - 1080.0 / 500.0) * 0.5);
}

#[test]
fn center_large_image() {
    let r = calculate_uv_transform(FitMode::Center, 4000, 3000, 1920, 1080);
    assert_eq!(r.uv_scale[0], 1920.0 / 4000.0);
    assert_eq!(r.uv_scale[1], 1080.0 / 3000.0);
    assert_eq!(r.uv_offset[0], (1.0 - 1920.0 / 4000.0) * 0.5);
    assert_eq!(r.uv_offset[1], (1.0 - 1080.0 / 3000.0) * 0.5);
}

#[test]
fn tile_same_scale_as_center_zero_offset() {
    let r = calculate_uv_transform(FitMode::Tile, 500, 500, 1920, 1080);
    let center = calculate_uv_transform(FitMode::Center, 500, 500, 1920, 1080);
    assert_eq!(r.uv_scale, center.uv_scale);
    assert_eq!(r.uv_offset, [0.0, 0.0]);
}

#[test]
fn span_identity() {
    let r = calculate_uv_transform(FitMode::Span, 9999, 8888, 1920, 1080);
    assert_eq!(r.uv_scale, [1.0, 1.0]);
    assert_eq!(r.uv_offset, [0.0, 0.0]);
}
