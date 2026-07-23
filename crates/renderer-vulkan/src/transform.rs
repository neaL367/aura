use aura_core::wallpaper::FitMode;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PushConstants {
    pub uv_scale: [f32; 2],
    pub uv_offset: [f32; 2],
}

pub fn calculate_uv_transform(
    mode: FitMode,
    img_w: u32,
    img_h: u32,
    mon_w: u32,
    mon_h: u32,
) -> PushConstants {
    if img_w == 0 || img_h == 0 || mon_w == 0 || mon_h == 0 {
        return PushConstants {
            uv_scale: [1.0, 1.0],
            uv_offset: [0.0, 0.0],
        };
    }

    let img_ar = img_w as f32 / img_h as f32;
    let mon_ar = mon_w as f32 / mon_h as f32;

    match mode {
        FitMode::Stretch => PushConstants {
            uv_scale: [1.0, 1.0],
            uv_offset: [0.0, 0.0],
        },
        FitMode::Fill => {
            let (scale_x, scale_y) = if img_ar > mon_ar {
                (mon_ar / img_ar, 1.0)
            } else {
                (1.0, img_ar / mon_ar)
            };
            PushConstants {
                uv_scale: [scale_x, scale_y],
                uv_offset: [(1.0 - scale_x) * 0.5, (1.0 - scale_y) * 0.5],
            }
        }
        FitMode::Fit => {
            let (scale_x, scale_y) = if img_ar > mon_ar {
                (1.0, img_ar / mon_ar)
            } else {
                (mon_ar / img_ar, 1.0)
            };
            PushConstants {
                uv_scale: [scale_x, scale_y],
                uv_offset: [(1.0 - scale_x) * 0.5, (1.0 - scale_y) * 0.5],
            }
        }
        FitMode::Center => {
            let scale_x = mon_w as f32 / img_w as f32;
            let scale_y = mon_h as f32 / img_h as f32;
            PushConstants {
                uv_scale: [scale_x, scale_y],
                uv_offset: [(1.0 - scale_x) * 0.5, (1.0 - scale_y) * 0.5],
            }
        }
        FitMode::Tile => {
            let scale_x = mon_w as f32 / img_w as f32;
            let scale_y = mon_h as f32 / img_h as f32;
            PushConstants {
                uv_scale: [scale_x, scale_y],
                uv_offset: [0.0, 0.0],
            }
        }
        FitMode::Span => PushConstants {
            uv_scale: [1.0, 1.0],
            uv_offset: [0.0, 0.0],
        },
    }
}

/// Calculate push constants for `FitMode::Span` across virtual desktop bounds.
#[allow(clippy::too_many_arguments)]
pub fn calculate_span_uv_transform(
    img_w: u32,
    img_h: u32,
    mon_x: i32,
    mon_y: i32,
    mon_w: u32,
    mon_h: u32,
    total_w: u32,
    total_h: u32,
) -> PushConstants {
    if img_w == 0 || img_h == 0 || mon_w == 0 || mon_h == 0 || total_w == 0 || total_h == 0 {
        return PushConstants {
            uv_scale: [1.0, 1.0],
            uv_offset: [0.0, 0.0],
        };
    }

    let total_pc = calculate_uv_transform(FitMode::Fill, img_w, img_h, total_w, total_h);

    let slice_x = (mon_x.max(0) as f32) / total_w as f32;
    let slice_y = (mon_y.max(0) as f32) / total_h as f32;
    let slice_w = mon_w as f32 / total_w as f32;
    let slice_h = mon_h as f32 / total_h as f32;

    let uv_scale_x = total_pc.uv_scale[0] * slice_w;
    let uv_scale_y = total_pc.uv_scale[1] * slice_h;
    let uv_offset_x = total_pc.uv_offset[0] + total_pc.uv_scale[0] * slice_x;
    let uv_offset_y = total_pc.uv_offset[1] + total_pc.uv_scale[1] * slice_y;

    PushConstants {
        uv_scale: [uv_scale_x, uv_scale_y],
        uv_offset: [uv_offset_x, uv_offset_y],
    }
}
