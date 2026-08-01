use egui::{Color32, Frame, Margin, Stroke};

use super::super::{palette::*, spacing::*};

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
