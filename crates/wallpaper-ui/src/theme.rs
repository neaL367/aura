use egui::{
    Color32, FontData, FontDefinitions, FontFamily, Frame, Margin, Stroke, Visuals, epaint::Shadow,
};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Color Palette — Light Theme (Zinc / Vercel)
// ---------------------------------------------------------------------------

pub const BG_APP: Color32 = Color32::from_rgb(250, 250, 250);
pub const BG_CARD: Color32 = Color32::from_rgb(255, 255, 255);
pub const BG_CARD_HOVER: Color32 = Color32::from_rgb(244, 244, 245);
pub const BG_CARD_SELECTED: Color32 = Color32::from_rgb(244, 244, 245);

pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(228, 228, 231);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(161, 161, 170);
pub const BORDER_ACCENT: Color32 = Color32::from_rgb(24, 24, 27);

pub const ACCENT_PRIMARY: Color32 = Color32::from_rgb(24, 24, 27);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(39, 39, 42);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(9, 9, 11);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(113, 113, 122);
pub const TEXT_ON_DARK: Color32 = Color32::from_rgb(255, 255, 255);

pub const STATUS_CONNECTED: Color32 = Color32::from_rgb(22, 163, 74);
pub const STATUS_CONNECTING: Color32 = Color32::from_rgb(217, 119, 6);
pub const STATUS_DISCONNECTED: Color32 = Color32::from_rgb(220, 38, 38);

pub const STATUS_BADGE_CONNECTED: Color32 = Color32::from_rgb(240, 253, 244);
pub const STATUS_BADGE_CONNECTING: Color32 = Color32::from_rgb(255, 251, 235);
pub const STATUS_BADGE_DISCONNECTED: Color32 = Color32::from_rgb(254, 242, 242);

// ---------------------------------------------------------------------------
// Differentiated badge colors per media kind
// ---------------------------------------------------------------------------

pub const BG_SIDEBAR: Color32 = Color32::from_rgb(255, 255, 255);
pub const BG_SIDEBAR_HOVER: Color32 = Color32::from_rgb(244, 244, 245);
pub const BG_SIDEBAR_ACTIVE: Color32 = Color32::from_rgb(244, 244, 245);
pub const SIDEBAR_INDICATOR: Color32 = Color32::from_rgb(24, 24, 27);
pub const CONTENT_CARD: Color32 = Color32::from_rgb(255, 255, 255);

pub const BG_INSPECTOR: Color32 = Color32::from_rgb(255, 255, 255);
pub const INSPECTOR_DIVIDER: Color32 = Color32::from_rgb(228, 228, 231);

pub const BADGE_IMAGE_BG: Color32 = Color32::from_rgb(244, 244, 245);
pub const BADGE_IMAGE_TEXT: Color32 = Color32::from_rgb(39, 39, 42);

pub const BADGE_GIF_BG: Color32 = Color32::from_rgb(254, 243, 199);
pub const BADGE_GIF_TEXT: Color32 = Color32::from_rgb(146, 64, 14);

pub const BADGE_VIDEO_BG: Color32 = Color32::from_rgb(219, 234, 254);
pub const BADGE_VIDEO_TEXT: Color32 = Color32::from_rgb(30, 64, 175);

// ---------------------------------------------------------------------------
// Typography Scale
// ---------------------------------------------------------------------------

pub const FONT_WINDOW_TITLE: f32 = 18.0;
pub const FONT_SECTION_HEADER: f32 = 16.0;
pub const FONT_CARD_TITLE: f32 = 14.0;
pub const FONT_BODY: f32 = 13.0;
pub const FONT_SECONDARY: f32 = 12.0;
pub const FONT_LABEL: f32 = 11.0;
pub const FONT_CAPTION: f32 = 10.0;

// ---------------------------------------------------------------------------
// Spacing & Radius (8px/4px Grid)
// ---------------------------------------------------------------------------

pub const SPACING_XS: f32 = 4.0;
pub const SPACING_SM: f32 = 8.0;
pub const SPACING_MD: f32 = 12.0;
pub const SPACING_LG: f32 = 16.0;
pub const SPACING_XL: f32 = 24.0;

pub const RADIUS_SM: f32 = 4.0;
pub const RADIUS_MD: f32 = 6.0;

// ---------------------------------------------------------------------------
// Component Sizing
// ---------------------------------------------------------------------------

pub const CARD_WIDTH: f32 = 240.0;
pub const MONITOR_CARD_WIDTH: f32 = 360.0;
pub const THUMBNAIL_SIZE: egui::Vec2 = egui::vec2(240.0, 135.0);
pub const SIDEBAR_WIDTH: f32 = 48.0;

// ---------------------------------------------------------------------------
// Elevation helpers
// ---------------------------------------------------------------------------

fn elevation_rest() -> Shadow {
    Shadow {
        offset: [0, 0],
        blur: 4,
        spread: 0,
        color: Color32::from_rgba_premultiplied(0, 0, 0, 15),
    }
}

fn elevation_raised() -> Shadow {
    Shadow {
        offset: [0, 0],
        blur: 8,
        spread: 0,
        color: Color32::from_rgba_premultiplied(0, 0, 0, 26),
    }
}

// ---------------------------------------------------------------------------
// Button
// ---------------------------------------------------------------------------

pub enum ButtonVariant {
    Primary,
    Secondary,
    Ghost,
}

pub fn button(ui: &mut egui::Ui, label: &str, variant: ButtonVariant) -> egui::Response {
    let (bg, hover_bg, fg, stroke) = match variant {
        ButtonVariant::Primary => (ACCENT_PRIMARY, ACCENT_HOVER, TEXT_ON_DARK, Stroke::NONE),
        ButtonVariant::Secondary => (
            BG_CARD,
            BG_CARD_HOVER,
            TEXT_PRIMARY,
            Stroke::new(1.0, BORDER_SUBTLE),
        ),
        ButtonVariant::Ghost => (
            Color32::TRANSPARENT,
            BG_CARD_HOVER,
            TEXT_PRIMARY,
            Stroke::NONE,
        ),
    };

    let id = ui.next_auto_id();
    let was_hovered = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let fill = if was_hovered { hover_bg } else { bg };

    let text = egui::RichText::new(label)
        .size(FONT_BODY)
        .strong()
        .color(fg);

    let btn = egui::Button::new(text)
        .fill(fill)
        .corner_radius(RADIUS_SM)
        .stroke(stroke);

    let response = ui.add(btn);
    let is_hovered = response.hovered();
    if is_hovered != was_hovered {
        ui.ctx().data_mut(|d| d.insert_temp(id, is_hovered));
        ui.ctx().request_repaint();
    }
    if is_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

// ---------------------------------------------------------------------------
// Badge
// ---------------------------------------------------------------------------

pub enum BadgeVariant {
    Image,
    Gif,
    Video,
}

pub fn badge(ui: &mut egui::Ui, label: &str, variant: BadgeVariant) {
    let (bg, fg) = match variant {
        BadgeVariant::Image => (BADGE_IMAGE_BG, BADGE_IMAGE_TEXT),
        BadgeVariant::Gif => (BADGE_GIF_BG, BADGE_GIF_TEXT),
        BadgeVariant::Video => (BADGE_VIDEO_BG, BADGE_VIDEO_TEXT),
    };

    let frame = Frame::new()
        .fill(bg)
        .corner_radius(RADIUS_SM)
        .inner_margin(Margin::symmetric(SPACING_SM as i8, SPACING_XS as i8));

    frame.show(ui, |ui| {
        ui.label(
            egui::RichText::new(label)
                .size(FONT_LABEL)
                .strong()
                .color(fg),
        );
    });
}

// ---------------------------------------------------------------------------
// Card Frame — hover-aware card container
// ---------------------------------------------------------------------------

pub enum Elevation {
    Rest,
    Raised,
}

pub fn card_frame<Id: Into<egui::Id>>(
    ui: &mut egui::Ui,
    id: Id,
    is_selected: bool,
    elevation: Elevation,
    add_contents: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let id = id.into();

    let was_hovered = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);

    let bg = if is_selected {
        BG_CARD_SELECTED
    } else if was_hovered {
        BG_CARD_HOVER
    } else {
        BG_CARD
    };

    let border_stroke = if is_selected {
        Stroke::new(1.5, BORDER_ACCENT)
    } else if was_hovered {
        Stroke::new(1.0, BORDER_STRONG)
    } else {
        Stroke::new(1.0, BORDER_SUBTLE)
    };

    let shadow = match elevation {
        Elevation::Rest => elevation_rest(),
        Elevation::Raised => elevation_raised(),
    };

    let frame = Frame::new()
        .fill(bg)
        .stroke(border_stroke)
        .corner_radius(RADIUS_MD)
        .shadow(shadow)
        .inner_margin(Margin::same(SPACING_MD as i8));

    let inner = frame.show(ui, add_contents);
    let rect = inner.response.rect;
    let response = ui.interact(rect, id, egui::Sense::click());

    let is_hovered = response.hovered();
    if is_hovered != was_hovered {
        ui.ctx().request_repaint();
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, is_hovered));

    response
}

// ---------------------------------------------------------------------------
// Section label — uppercase overline
// ---------------------------------------------------------------------------

pub fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(FONT_LABEL)
            .strong()
            .color(TEXT_MUTED),
    );
}

// ---------------------------------------------------------------------------
// Empty State
// ---------------------------------------------------------------------------

pub fn empty_state(
    ui: &mut egui::Ui,
    title: &str,
    description: &str,
    add_extra: impl FnOnce(&mut egui::Ui),
) {
    group_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical_centered(|ui| {
            ui.add_space(SPACING_XL);
            ui.label(
                egui::RichText::new(title)
                    .size(FONT_SECTION_HEADER)
                    .strong()
                    .color(TEXT_MUTED),
            );
            ui.add_space(SPACING_SM);
            ui.label(
                egui::RichText::new(description)
                    .size(FONT_SECONDARY)
                    .color(TEXT_MUTED),
            );
            add_extra(ui);
            ui.add_space(SPACING_XL);
        });
    });
}

// ---------------------------------------------------------------------------
// Connection status glyph
// ---------------------------------------------------------------------------

pub fn connection_dot(ui: &mut egui::Ui, color: Color32) {
    ui.colored_label(color, "\u{25CF}");
}

// ---------------------------------------------------------------------------
// Frame Builders
// ---------------------------------------------------------------------------

pub fn header_frame() -> Frame {
    Frame::new()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .inner_margin(Margin::symmetric(SPACING_LG as i8, SPACING_MD as i8))
}

pub fn group_frame() -> Frame {
    Frame::new()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .corner_radius(RADIUS_MD)
        .inner_margin(Margin::same(SPACING_LG as i8))
}

pub fn badge_frame(bg: Color32) -> Frame {
    Frame::new()
        .fill(bg)
        .corner_radius(RADIUS_SM)
        .inner_margin(Margin::symmetric(SPACING_SM as i8, SPACING_XS as i8))
}

// ---------------------------------------------------------------------------
// URI helpers
// ---------------------------------------------------------------------------

/// Build a `file:///` URI from a Windows path.  Converts backslashes to
/// forward slashes so egui's image loaders can parse the URI correctly.
pub fn file_uri(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    let normalized: String = s.replace('\\', "/");
    // Percent-encode '#' since it's a URI fragment delimiter.
    let normalized = normalized.replace('#', "%23");
    format!("file:///{}", normalized)
}

// ---------------------------------------------------------------------------
// Setup — Geist fonts + light mode Visuals
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
    fonts
        .families
        .entry(FontFamily::Name("geist-medium".into()))
        .or_default()
        .push("Geist-Medium".to_owned());

    ctx.set_fonts(fonts);

    let mut visuals = Visuals::light();
    visuals.panel_fill = BG_APP;
    visuals.window_fill = BG_CARD;
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
