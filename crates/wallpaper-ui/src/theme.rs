use egui::{Color32, FontData, FontDefinitions, FontFamily, Frame, Margin, Stroke, Visuals};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Design Tokens: Vercel / Next.js Light Theme Color Palette
// ---------------------------------------------------------------------------

pub const BG_APP: Color32 = Color32::from_rgb(250, 250, 250); // #fafafa
pub const BG_CARD: Color32 = Color32::from_rgb(255, 255, 255); // #ffffff
pub const BG_CARD_HOVER: Color32 = Color32::from_rgb(244, 244, 245); // #f4f4f5
pub const BG_CARD_SELECTED: Color32 = Color32::from_rgb(244, 244, 245);

pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(228, 228, 231); // #e4e4e7
pub const BORDER_STRONG: Color32 = Color32::from_rgb(161, 161, 170); // #a1a1aa
pub const BORDER_ACCENT: Color32 = Color32::from_rgb(24, 24, 27);

pub const ACCENT_PRIMARY: Color32 = Color32::from_rgb(24, 24, 27); // #18181b (Zinc 900)
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(39, 39, 42);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(9, 9, 11); // #09090b (Zinc 950)
pub const TEXT_MUTED: Color32 = Color32::from_rgb(113, 113, 122); // #71717a (Zinc 500)
pub const TEXT_ON_DARK: Color32 = Color32::from_rgb(255, 255, 255); // #ffffff

pub const BADGE_IMAGE_BG: Color32 = Color32::from_rgb(244, 244, 245);
pub const BADGE_IMAGE_TEXT: Color32 = Color32::from_rgb(39, 39, 42);

pub const BADGE_GIF_BG: Color32 = Color32::from_rgb(244, 244, 245);
pub const BADGE_GIF_TEXT: Color32 = Color32::from_rgb(39, 39, 42);

pub const BADGE_VIDEO_BG: Color32 = Color32::from_rgb(244, 244, 245);
pub const BADGE_VIDEO_TEXT: Color32 = Color32::from_rgb(39, 39, 42);

pub const STATUS_CONNECTED: Color32 = Color32::from_rgb(22, 163, 74); // green-600
pub const STATUS_CONNECTING: Color32 = Color32::from_rgb(217, 119, 6); // amber-600
pub const STATUS_DISCONNECTED: Color32 = Color32::from_rgb(220, 38, 38); // red-600

// ---------------------------------------------------------------------------
// Design Tokens: Typography Scale
// ---------------------------------------------------------------------------

pub const FONT_WINDOW_TITLE: f32 = 18.0;
pub const FONT_SECTION_HEADER: f32 = 16.0;
pub const FONT_CARD_TITLE: f32 = 14.0;
pub const FONT_BODY: f32 = 13.0;
pub const FONT_SECONDARY: f32 = 12.0;
pub const FONT_LABEL: f32 = 11.0;
pub const FONT_CAPTION: f32 = 10.0;

// ---------------------------------------------------------------------------
// Design Tokens: Spacing & Radius Tokens (8px/4px Grid)
// ---------------------------------------------------------------------------

pub const SPACING_XS: f32 = 4.0;
pub const SPACING_SM: f32 = 8.0;
pub const SPACING_MD: f32 = 12.0;
pub const SPACING_LG: f32 = 16.0;
pub const SPACING_XL: f32 = 24.0;

pub const RADIUS_SM: f32 = 4.0;
pub const RADIUS_MD: f32 = 6.0;

// ---------------------------------------------------------------------------
// Setup Geist Font & Visuals
// ---------------------------------------------------------------------------

pub fn setup_theme(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let regular_ttf = include_bytes!("../../../resources/fonts/Geist-Regular.ttf");
    let medium_ttf = include_bytes!("../../../resources/fonts/Geist-Medium.ttf");

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

    ctx.set_fonts(fonts);

    let mut visuals = Visuals::light();
    visuals.panel_fill = BG_APP;
    visuals.window_fill = BG_CARD;
    visuals.override_text_color = Some(TEXT_PRIMARY);

    // Inactive Widgets
    visuals.widgets.noninteractive.bg_fill = BG_CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
    visuals.widgets.noninteractive.corner_radius = RADIUS_MD.into();
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    visuals.widgets.inactive.bg_fill = BG_CARD;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
    visuals.widgets.inactive.corner_radius = RADIUS_SM.into();
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    // Hovered Widgets
    visuals.widgets.hovered.bg_fill = BG_CARD_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    visuals.widgets.hovered.corner_radius = RADIUS_SM.into();
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    // Active & Selected Widgets -> White Text on Dark Zinc
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

// ---------------------------------------------------------------------------
// Custom Pill Widget with Explicit High Contrast
// ---------------------------------------------------------------------------

pub fn pill_button(ui: &mut egui::Ui, selected: bool, label: &str) -> bool {
    let (bg, fg) = if selected {
        (ACCENT_PRIMARY, TEXT_ON_DARK)
    } else {
        (BG_CARD, TEXT_PRIMARY)
    };

    let text = egui::RichText::new(label)
        .size(FONT_SECONDARY)
        .strong()
        .color(fg);

    let btn = egui::Button::new(text)
        .fill(bg)
        .corner_radius(RADIUS_SM)
        .stroke(if selected {
            Stroke::NONE
        } else {
            Stroke::new(1.0, BORDER_SUBTLE)
        });

    ui.add(btn).clicked()
}

// ---------------------------------------------------------------------------
// Reusable Frame Builders
// ---------------------------------------------------------------------------

pub fn card_frame(is_hovered: bool, is_selected: bool) -> Frame {
    let bg = if is_selected {
        BG_CARD_SELECTED
    } else if is_hovered {
        BG_CARD_HOVER
    } else {
        BG_CARD
    };

    let stroke = if is_selected {
        Stroke::new(1.5, BORDER_ACCENT)
    } else if is_hovered {
        Stroke::new(1.0, BORDER_STRONG)
    } else {
        Stroke::new(1.0, BORDER_SUBTLE)
    };

    Frame::new()
        .fill(bg)
        .stroke(stroke)
        .corner_radius(RADIUS_MD)
        .inner_margin(Margin::same(SPACING_MD as i8))
}

pub fn header_frame() -> Frame {
    Frame::new()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .inner_margin(Margin::symmetric(SPACING_LG as i8, SPACING_MD as i8))
}

pub fn badge_frame(bg: Color32) -> Frame {
    Frame::new()
        .fill(bg)
        .corner_radius(RADIUS_SM)
        .inner_margin(Margin::symmetric(SPACING_SM as i8, SPACING_XS as i8))
}

pub fn group_frame() -> Frame {
    Frame::new()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .corner_radius(RADIUS_MD)
        .inner_margin(Margin::same(SPACING_LG as i8))
}
