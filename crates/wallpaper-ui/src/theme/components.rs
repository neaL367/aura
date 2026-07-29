use egui::{Color32, Frame, Margin, Stroke, epaint::Shadow};

use super::icons::ICON_DOT;
use super::palette::*;
use super::spacing::*;
use super::typography::*;

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

pub fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(FONT_LABEL)
            .strong()
            .color(TEXT_MUTED),
    );
}

pub fn empty_state(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    description: &str,
    add_extra: impl FnOnce(&mut egui::Ui),
) {
    group_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical_centered(|ui| {
            ui.add_space(SPACING_XL);
            ui.label(egui::RichText::new(icon).size(32.0).color(TEXT_MUTED));
            ui.add_space(SPACING_SM);
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

pub fn connection_dot(ui: &mut egui::Ui, color: Color32) {
    ui.colored_label(color, ICON_DOT);
}

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
