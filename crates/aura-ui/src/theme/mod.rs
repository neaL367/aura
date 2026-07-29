pub mod components;
pub mod icons;
pub mod palette;
pub mod spacing;
pub mod typography;

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, Stroke, Visuals};

pub use components::*;
pub use icons::*;
pub use palette::*;
pub use spacing::*;
pub use typography::*;

/// Build a `file:///` URI from a Windows path. Converts backslashes to
/// forward slashes so egui's image loaders can parse the URI correctly.
pub fn file_uri(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    let normalized: String = s.replace('\\', "/");
    // Percent-encode '#' since it's a URI fragment delimiter.
    let normalized = normalized.replace('#', "%23");
    format!("file:///{}", normalized)
}

pub fn setup_theme(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let regular_ttf = include_bytes!("../../../../resources/fonts/Geist-Regular.ttf");
    let medium_ttf = include_bytes!("../../../../resources/fonts/Geist-Medium.ttf");

    fonts.font_data.insert(
        "Geist-Regular".to_owned(),
        Arc::new(FontData::from_static(regular_ttf)),
    );
    fonts.font_data.insert(
        "Geist-Medium".to_owned(),
        Arc::new(FontData::from_static(medium_ttf)),
    );

    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "Geist-Regular".to_owned());
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(1, "Geist-Medium".to_owned());
    fonts
        .families
        .entry(FontFamily::Name("geist-medium".into()))
        .or_default()
        .push("Geist-Medium".to_owned());

    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    ctx.set_fonts(fonts);

    let mut visuals = Visuals::light();
    visuals.panel_fill = BG_APP;
    visuals.window_fill = BG_CARD;
    visuals.extreme_bg_color = egui::Color32::from_rgb(255, 255, 255);
    visuals.override_text_color = Some(TEXT_PRIMARY);

    visuals.widgets.noninteractive.bg_fill = BG_CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
    visuals.widgets.noninteractive.corner_radius = RADIUS_MD.into();
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    visuals.widgets.inactive.bg_fill = BG_CARD;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
    visuals.widgets.inactive.corner_radius = RADIUS_SM.into();
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    visuals.widgets.hovered.bg_fill = BG_CARD_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    visuals.widgets.hovered.corner_radius = RADIUS_SM.into();
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    visuals.widgets.active.bg_fill = ACCENT_PRIMARY;
    visuals.widgets.active.bg_stroke = Stroke::NONE;
    visuals.widgets.active.corner_radius = RADIUS_SM.into();
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_ON_DARK);

    visuals.widgets.open.bg_fill = ACCENT_PRIMARY;
    visuals.widgets.open.bg_stroke = Stroke::NONE;
    visuals.widgets.open.corner_radius = RADIUS_SM.into();
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT_ON_DARK);

    visuals.selection.bg_fill = ACCENT_PRIMARY;
    visuals.selection.stroke = Stroke::new(1.0, TEXT_ON_DARK);

    ctx.set_visuals(visuals);
}

pub fn setup_dark_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG_APP_DARK;
    visuals.window_fill = BG_CARD_DARK;
    visuals.extreme_bg_color = egui::Color32::from_rgb(18, 18, 20);
    visuals.override_text_color = Some(TEXT_PRIMARY_DARK);

    visuals.widgets.noninteractive.bg_fill = BG_CARD_DARK;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE_DARK);
    visuals.widgets.noninteractive.corner_radius = RADIUS_MD.into();
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY_DARK);

    visuals.widgets.inactive.bg_fill = BG_CARD_DARK;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE_DARK);
    visuals.widgets.inactive.corner_radius = RADIUS_SM.into();
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY_DARK);

    visuals.widgets.hovered.bg_fill = BG_CARD_HOVER_DARK;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG_DARK);
    visuals.widgets.hovered.corner_radius = RADIUS_SM.into();
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY_DARK);

    visuals.widgets.active.bg_fill = ACCENT_PRIMARY_DARK;
    visuals.widgets.active.bg_stroke = Stroke::NONE;
    visuals.widgets.active.corner_radius = RADIUS_SM.into();
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_ON_DARK_DARK);

    visuals.widgets.open.bg_fill = ACCENT_PRIMARY_DARK;
    visuals.widgets.open.bg_stroke = Stroke::NONE;
    visuals.widgets.open.corner_radius = RADIUS_SM.into();
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT_ON_DARK_DARK);

    visuals.selection.bg_fill = ACCENT_PRIMARY_DARK;
    visuals.selection.stroke = Stroke::new(1.0, TEXT_ON_DARK_DARK);

    ctx.set_visuals(visuals);
}
