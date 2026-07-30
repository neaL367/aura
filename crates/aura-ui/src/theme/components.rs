use egui::{Color32, Frame, Margin, Stroke, epaint::Shadow};

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
        .stroke(stroke);

    let response = ui.add(btn);
    if response.hovered() {
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
    let dark = ui.visuals().dark_mode;

    let was_hovered = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let is_hovered = was_hovered;

    let bg = if is_selected {
        if dark {
            BG_CARD_SELECTED_DARK
        } else {
            BG_CARD_SELECTED
        }
    } else if is_hovered {
        if dark {
            BG_CARD_HOVER_DARK
        } else {
            BG_CARD_HOVER
        }
    } else {
        if dark { BG_CARD_DARK } else { BG_CARD }
    };

    let border_stroke = if is_selected {
        Stroke::new(
            1.5,
            if dark {
                BORDER_ACCENT_DARK
            } else {
                BORDER_ACCENT
            },
        )
    } else if is_hovered {
        Stroke::new(
            1.0,
            if dark {
                BORDER_STRONG_DARK
            } else {
                BORDER_STRONG
            },
        )
    } else {
        Stroke::new(
            1.0,
            if dark {
                BORDER_SUBTLE_DARK
            } else {
                BORDER_SUBTLE
            },
        )
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

    let hover_now = response.hovered();
    if hover_now != was_hovered {
        ui.ctx().data_mut(|d| d.insert_temp(id, hover_now));
        ui.ctx().request_repaint();
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    response
}

pub fn section_label(ui: &mut egui::Ui, text: &str) {
    let dark = ui.visuals().dark_mode;
    ui.label(
        egui::RichText::new(text)
            .size(FONT_LABEL)
            .strong()
            .color(if dark { TEXT_MUTED_DARK } else { TEXT_MUTED }),
    );
}

pub fn empty_state(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    description: &str,
    add_extra: impl FnOnce(&mut egui::Ui),
) {
    let dark = ui.visuals().dark_mode;
    let muted = if dark { TEXT_MUTED_DARK } else { TEXT_MUTED };
    group_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical_centered(|ui| {
            ui.add_space(SPACING_XL);
            ui.label(egui::RichText::new(icon).size(32.0).color(muted));
            ui.add_space(SPACING_SM);
            ui.label(
                egui::RichText::new(title)
                    .size(FONT_SECTION_HEADER)
                    .strong()
                    .color(muted),
            );
            ui.add_space(SPACING_SM);
            ui.label(
                egui::RichText::new(description)
                    .size(FONT_SECONDARY)
                    .color(muted),
            );
            add_extra(ui);
            ui.add_space(SPACING_XL);
        });
    });
}

pub fn status_dot(ui: &mut egui::Ui, color: Color32, size: f32) {
    ui.label(egui::RichText::new("●").size(size).color(color));
}

pub fn connection_dot(ui: &mut egui::Ui, color: Color32) {
    status_dot(ui, color, 10.0);
}

pub fn header_frame(ui: &egui::Ui) -> Frame {
    let dark = ui.visuals().dark_mode;
    Frame::new()
        .fill(if dark { BG_CARD_DARK } else { BG_CARD })
        .stroke(Stroke::new(
            1.0,
            if dark {
                BORDER_SUBTLE_DARK
            } else {
                BORDER_SUBTLE
            },
        ))
        .inner_margin(Margin::symmetric(SPACING_LG as i8, SPACING_MD as i8))
}

pub fn group_frame(ui: &egui::Ui) -> Frame {
    let dark = ui.visuals().dark_mode;
    Frame::new()
        .fill(if dark { BG_CARD_DARK } else { BG_CARD })
        .stroke(Stroke::new(
            1.0,
            if dark {
                BORDER_SUBTLE_DARK
            } else {
                BORDER_SUBTLE
            },
        ))
        .corner_radius(RADIUS_MD)
        .inner_margin(Margin::same(SPACING_LG as i8))
}

pub fn badge_frame(bg: Color32) -> Frame {
    Frame::new()
        .fill(bg)
        .corner_radius(RADIUS_SM)
        .inner_margin(Margin::symmetric(SPACING_SM as i8, SPACING_XS as i8))
}

pub fn segmented_container(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    let dark = ui.visuals().dark_mode;
    let bg = if dark {
        Color32::from_rgb(18, 18, 22)
    } else {
        Color32::from_rgb(238, 240, 244)
    };
    let border = if dark {
        BORDER_SUBTLE_DARK
    } else {
        BORDER_SUBTLE
    };
    Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(RADIUS_MD)
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(3.0, 2.0);
            ui.spacing_mut().button_padding = egui::vec2(10.0, 5.0);
            add_contents(ui);
        });
}

pub fn toggle_switch(ui: &mut egui::Ui, value: &mut bool) -> egui::Response {
    let desired_size = egui::vec2(36.0, 20.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let dark = ui.visuals().dark_mode;
        let active_bg = if dark {
            ACCENT_PRIMARY_DARK
        } else {
            ACCENT_PRIMARY
        };
        let inactive_bg = if dark {
            Color32::from_rgb(45, 45, 52)
        } else {
            Color32::from_rgb(220, 224, 230)
        };
        let thumb_color = if *value {
            if dark {
                TEXT_ON_DARK_DARK
            } else {
                TEXT_ON_DARK
            }
        } else {
            if dark {
                TEXT_PRIMARY_DARK
            } else {
                TEXT_PRIMARY
            }
        };

        let track_bg = if *value { active_bg } else { inactive_bg };
        let radius = rect.height() / 2.0;
        ui.painter().rect_filled(rect, radius, track_bg);

        let circle_x = if *value {
            rect.max.x - radius
        } else {
            rect.min.x + radius
        };
        let circle_center = egui::pos2(circle_x, rect.center().y);
        ui.painter()
            .circle_filled(circle_center, radius - 2.0, thumb_color);
    }

    response
}
