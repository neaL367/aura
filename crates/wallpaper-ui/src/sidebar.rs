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
            ui.set_min_width(theme::SIDEBAR_EXPANDED_WIDTH);
            ui.set_max_width(theme::SIDEBAR_EXPANDED_WIDTH);
            ui.set_min_height(ui.available_height());

            ui.vertical(|ui| {
                // Logo / wordmark area
                ui.add_space(theme::SPACING_LG);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("Aura")
                            .size(theme::FONT_WINDOW_TITLE)
                            .strong()
                            .color(theme::TEXT_PRIMARY),
                    );
                });
                ui.add_space(theme::SPACING_LG);

                // Nav items with icon + label
                let mut clicked = None;

                if Self::nav_item(
                    ui,
                    sidebar_id.with("gallery"),
                    theme::ICON_GALLERY,
                    "Gallery",
                    *active_tab == Tab::Gallery,
                ) {
                    clicked = Some(Tab::Gallery);
                }
                ui.add_space(theme::SPACING_XS);
                if Self::nav_item(
                    ui,
                    sidebar_id.with("settings"),
                    theme::ICON_SETTINGS,
                    "Settings",
                    *active_tab == Tab::Settings,
                ) {
                    clicked = Some(Tab::Settings);
                }

                if let Some(tab) = clicked {
                    *active_tab = tab;
                }

                // Spacer + version at bottom
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(theme::SPACING_SM);
                    ui.label(
                        egui::RichText::new("v0.1.0")
                            .size(theme::FONT_CAPTION)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(theme::SPACING_SM);
                });
            });
        });
    }

    fn nav_item(ui: &mut egui::Ui, id: egui::Id, icon: &str, label: &str, is_active: bool) -> bool {
        let available = ui.available_width();
        let item_height = theme::NAV_ITEM_HEIGHT;
        let left_margin = theme::SPACING_MD;

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

        // Background fill — rounded on right side
        let bg_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + left_margin, rect.top()),
            egui::vec2(available - left_margin - theme::SPACING_SM, item_height),
        );
        ui.painter()
            .rect_filled(bg_rect, theme::RADIUS_SM, hover_bg);

        // Icon
        let icon_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + left_margin + theme::SPACING_SM, rect.top()),
            egui::vec2(20.0, item_height),
        );
        ui.painter().text(
            icon_rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(theme::FONT_BODY),
            fg,
        );

        // Label
        let label_rect = egui::Rect::from_min_size(
            egui::pos2(
                rect.left() + left_margin + 20.0 + theme::SPACING_SM,
                rect.top(),
            ),
            egui::vec2(
                available - left_margin - 20.0 - theme::SPACING_LG,
                item_height,
            ),
        );
        ui.painter().text(
            label_rect.center(),
            egui::Align2::LEFT_CENTER,
            label,
            theme::font_body(),
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
