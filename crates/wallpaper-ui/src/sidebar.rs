use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Gallery,
    Settings,
}

pub struct Sidebar;

impl Sidebar {
    pub fn show(ui: &mut egui::Ui, active_tab: &mut Tab) {
        let sidebar_id = ui.make_persistent_id("aura_sidebar");
        let sidebar_frame = egui::Frame::new()
            .fill(theme::BG_SIDEBAR)
            .corner_radius(0.0)
            .stroke(egui::Stroke::NONE);

        sidebar_frame.show(ui, |ui| {
            ui.set_min_width(theme::SIDEBAR_WIDTH);
            ui.set_max_width(theme::SIDEBAR_WIDTH);
            ui.set_min_height(ui.available_height());

            ui.vertical(|ui| {
                ui.add_space(theme::SPACING_MD);

                let mut clicked = None;

                if Self::nav_item(
                    ui,
                    sidebar_id.with("gallery"),
                    "G",
                    "Gallery",
                    *active_tab == Tab::Gallery,
                ) {
                    clicked = Some(Tab::Gallery);
                }
                if Self::nav_item(
                    ui,
                    sidebar_id.with("settings"),
                    "S",
                    "Settings",
                    *active_tab == Tab::Settings,
                ) {
                    clicked = Some(Tab::Settings);
                }

                if let Some(tab) = clicked {
                    *active_tab = tab;
                }
            });
        });
    }

    fn nav_item(ui: &mut egui::Ui, id: egui::Id, icon: &str, label: &str, is_active: bool) -> bool {
        let available = ui.available_width();
        let item_height = 48.0;

        let (bg, fg) = if is_active {
            (theme::BG_SIDEBAR_ACTIVE, theme::TEXT_PRIMARY)
        } else {
            (theme::BG_SIDEBAR, theme::TEXT_MUTED)
        };

        let rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(available, item_height));

        let hovered = ui.rect_contains_pointer(rect);
        let hover_bg = if hovered && !is_active {
            theme::BG_SIDEBAR_HOVER
        } else {
            bg
        };

        // Active indicator bar on the left edge
        if is_active {
            let indicator = egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.top()),
                egui::vec2(3.0, rect.height()),
            );
            ui.painter()
                .rect_filled(indicator, 0.0, theme::SIDEBAR_INDICATOR);
        }

        // Background fill with rounded corners
        ui.painter().rect_filled(rect, 0.0, hover_bg);

        // Icon text
        let icon_pos = egui::pos2(rect.center().x, rect.center().y - theme::SPACING_XS);
        ui.painter().text(
            icon_pos,
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(theme::FONT_CARD_TITLE),
            fg,
        );

        ui.advance_cursor_after_rect(rect);

        let response = ui.interact(rect, id, egui::Sense::click());

        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        response.on_hover_text(label).clicked()
    }
}
