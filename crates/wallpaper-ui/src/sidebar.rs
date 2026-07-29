use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Gallery,
    Settings,
}

pub struct Sidebar;

impl Sidebar {
    pub fn show(ui: &mut egui::Ui, active_tab: &mut Tab, collapsed: &mut bool) {
        let sidebar_id = ui.make_persistent_id("aura_sidebar");
        let target_width = if *collapsed {
            theme::SIDEBAR_COLLAPSED_WIDTH
        } else {
            theme::SIDEBAR_EXPANDED_WIDTH
        };

        let sidebar_frame = egui::Frame::new()
            .fill(ui.visuals().window_fill)
            .corner_radius(0.0)
            .stroke(egui::Stroke::NONE);

        sidebar_frame.show(ui, |ui| {
            ui.set_min_width(target_width);
            ui.set_max_width(target_width);
            ui.set_min_height(ui.available_height());

            ui.vertical(|ui| {
                ui.add_space(theme::SPACING_LG);
                ui.vertical_centered(|ui| {
                    if *collapsed {
                        ui.label(
                            egui::RichText::new("A")
                                .size(theme::FONT_WINDOW_TITLE)
                                .strong(),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("Aura")
                                .size(theme::FONT_WINDOW_TITLE)
                                .strong(),
                        );
                    }
                });
                ui.add_space(theme::SPACING_LG);

                let mut clicked = None;

                if Self::nav_item(
                    ui,
                    sidebar_id.with("gallery"),
                    theme::ICON_GALLERY,
                    "Gallery",
                    *active_tab == Tab::Gallery,
                    *collapsed,
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
                    *collapsed,
                ) {
                    clicked = Some(Tab::Settings);
                }

                if let Some(tab) = clicked {
                    *active_tab = tab;
                }

                // Collapse/Expand toggle & version at bottom
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(theme::SPACING_SM);
                    let toggle_icon = if *collapsed {
                        theme::ICON_EXPAND
                    } else {
                        theme::ICON_COLLAPSE
                    };
                    let toggle_tooltip = if *collapsed {
                        "Expand sidebar"
                    } else {
                        "Collapse sidebar"
                    };
                    if theme::button(ui, toggle_icon, theme::ButtonVariant::Ghost)
                        .on_hover_text(toggle_tooltip)
                        .clicked()
                    {
                        *collapsed = !*collapsed;
                    }

                    if !*collapsed {
                        ui.add_space(theme::SPACING_SM);
                        ui.label(
                            egui::RichText::new("v0.1.0")
                                .size(theme::FONT_CAPTION)
                                .color(ui.visuals().weak_text_color()),
                        );
                    }
                    ui.add_space(theme::SPACING_SM);
                });
            });
        });
    }

    fn nav_item(
        ui: &mut egui::Ui,
        id: egui::Id,
        icon: &str,
        label: &str,
        is_active: bool,
        collapsed: bool,
    ) -> bool {
        let available = ui.available_width();
        let item_height = theme::NAV_ITEM_HEIGHT;
        let left_margin = if collapsed {
            theme::SPACING_XS
        } else {
            theme::SPACING_MD
        };

        let dark = ui.visuals().dark_mode;
        let active_bg = if dark {
            theme::BG_CARD_SELECTED_DARK
        } else {
            theme::BG_SIDEBAR_ACTIVE
        };
        let hover_bg_color = if dark {
            theme::BG_CARD_HOVER_DARK
        } else {
            theme::BG_SIDEBAR_HOVER
        };
        let active_indicator = if dark {
            theme::BORDER_ACCENT_DARK
        } else {
            theme::SIDEBAR_INDICATOR
        };

        let (bg, fg) = if is_active {
            (active_bg, ui.visuals().text_color())
        } else {
            (ui.visuals().window_fill, ui.visuals().weak_text_color())
        };

        let rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(available, item_height));

        let hovered = ui.rect_contains_pointer(rect);
        let hover_bg = if hovered && !is_active {
            hover_bg_color
        } else {
            bg
        };

        if is_active {
            let indicator = egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.top()),
                egui::vec2(3.0, rect.height()),
            );
            ui.painter().rect_filled(indicator, 0.0, active_indicator);
        }

        let bg_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + left_margin, rect.top()),
            egui::vec2(
                available
                    - left_margin
                    - if collapsed {
                        theme::SPACING_XS
                    } else {
                        theme::SPACING_SM
                    },
                item_height,
            ),
        );
        ui.painter()
            .rect_filled(bg_rect, theme::RADIUS_SM, hover_bg);

        if collapsed {
            // Icon centered
            ui.painter().text(
                bg_rect.center(),
                egui::Align2::CENTER_CENTER,
                icon,
                egui::FontId::proportional(theme::FONT_BODY),
                fg,
            );
        } else {
            // Icon + Label
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
        }

        ui.advance_cursor_after_rect(rect);

        let response = ui.interact(rect, id, egui::Sense::click());

        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        response.on_hover_text(label).clicked()
    }
}
