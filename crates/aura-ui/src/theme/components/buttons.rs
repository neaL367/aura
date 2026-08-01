use egui::{Color32, Stroke};

use super::super::{palette::*, spacing::*, typography::*};

pub enum ButtonVariant {
    Primary,
    Secondary,
    Ghost,
}

pub fn button(ui: &mut egui::Ui, label: &str, variant: ButtonVariant) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let (bg, _hover_bg, fg, stroke) = match variant {
        ButtonVariant::Primary => (
            if dark {
                ACCENT_PRIMARY_DARK
            } else {
                ACCENT_PRIMARY
            },
            if dark {
                ACCENT_HOVER_DARK
            } else {
                ACCENT_HOVER
            },
            if dark {
                TEXT_ON_DARK_DARK
            } else {
                TEXT_ON_DARK
            },
            Stroke::NONE,
        ),
        ButtonVariant::Secondary => (
            if dark { BG_CARD_DARK } else { BG_CARD },
            if dark {
                BG_CARD_HOVER_DARK
            } else {
                BG_CARD_HOVER
            },
            if dark {
                TEXT_PRIMARY_DARK
            } else {
                TEXT_PRIMARY
            },
            Stroke::new(
                1.0,
                if dark {
                    BORDER_SUBTLE_DARK
                } else {
                    BORDER_SUBTLE
                },
            ),
        ),
        ButtonVariant::Ghost => (
            Color32::TRANSPARENT,
            if dark {
                BG_CARD_HOVER_DARK
            } else {
                BG_CARD_HOVER
            },
            if dark {
                TEXT_PRIMARY_DARK
            } else {
                TEXT_PRIMARY
            },
            Stroke::NONE,
        ),
    };

    let text = egui::RichText::new(label)
        .size(FONT_BODY)
        .strong()
        .color(fg);

    let btn = egui::Button::new(text)
        .fill(bg)
        .corner_radius(RADIUS_SM)
        .stroke(stroke)
        .min_size(egui::vec2(0.0, BUTTON_MIN_HEIGHT));

    let response = ui.add(btn);
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}
