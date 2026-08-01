use aura_core::monitor::MonitorAssignment;
use aura_core::wallpaper::FitMode;
use aura_ipc::protocol::{MonitorSummary, Request, WallpaperEntry};

use crate::ipc_client::UiIpcClient;
use crate::theme;

pub struct MonitorCanvas;

impl MonitorCanvas {
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        ui: &mut egui::Ui,
        monitors: &[MonitorSummary],
        assignments: &[MonitorAssignment],
        selected_wallpaper: Option<&WallpaperEntry>,
        fit_mode: FitMode,
        ipc_client: &UiIpcClient,
    ) {
        if monitors.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(crate::theme::SPACING_MD);
                ui.label(
                    egui::RichText::new("No active monitors detected")
                        .size(crate::theme::FONT_BODY)
                        .color(ui.visuals().weak_text_color()),
                );
            });
            return;
        }

        if monitors.len() <= 1 {
            Self::compact_status_bar(ui, &monitors[0], selected_wallpaper, fit_mode, ipc_client);
            return;
        }

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Display Topology Canvas")
                    .strong()
                    .size(theme::FONT_SECTION_HEADER),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("{} Monitor(s)", monitors.len()))
                        .size(theme::FONT_SECONDARY)
                        .color(ui.visuals().weak_text_color()),
                );
            });
        });
        ui.add_space(theme::SPACING_SM);

        // 2D Monitor Canvas Container
        let canvas_rect = ui.available_rect_before_wrap();
        let (rect, _response) =
            ui.allocate_at_least(egui::vec2(canvas_rect.width(), 140.0), egui::Sense::hover());

        let dark = ui.visuals().dark_mode;
        let canvas_bg = if dark {
            theme::BG_CARD_DARK
        } else {
            theme::BG_INSPECTOR
        };
        let box_bg = if dark {
            theme::BG_CARD_DARK
        } else {
            theme::BG_CARD
        };

        ui.painter().rect_filled(rect, theme::RADIUS_MD, canvas_bg);

        // Render Monitor boxes for 2+ monitors
        let padding = 12.0;
        let count = monitors.len() as f32;
        let box_w = ((rect.width() - (padding * (count + 1.0))) / count).clamp(120.0, 220.0);
        let box_h = (box_w * 9.0 / 16.0).clamp(70.0, 110.0);
        let start_y = rect.top() + (rect.height() - box_h) / 2.0;
        let total_boxes_w = count * box_w + (count - 1.0) * padding;
        let base_x = rect.left() + (rect.width() - total_boxes_w).max(0.0) / 2.0;

        for (i, mon) in monitors.iter().enumerate() {
            let start_x = base_x + i as f32 * (box_w + padding);
            let mon_rect =
                egui::Rect::from_min_size(egui::pos2(start_x, start_y), egui::vec2(box_w, box_h));

            let assigned = assignments.iter().find(|a| a.monitor_id == mon.id);
            let is_assigned = assigned.is_some();

            // Box Fill & Stroke
            let stroke = if is_assigned {
                egui::Stroke::new(1.5, theme::STATUS_CONNECTED)
            } else {
                egui::Stroke::new(
                    1.0,
                    if dark {
                        theme::BORDER_SUBTLE_DARK
                    } else {
                        theme::BORDER_SUBTLE
                    },
                )
            };

            ui.painter().rect_filled(mon_rect, theme::RADIUS_MD, box_bg);
            ui.painter().rect_stroke(
                mon_rect,
                theme::RADIUS_MD,
                stroke,
                egui::StrokeKind::Outside,
            );

            let child_ui = &mut ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(mon_rect)
                    .layout(egui::Layout::top_down(egui::Align::Center)),
            );

            child_ui.add_space(theme::SPACING_SM);
            child_ui.label(
                egui::RichText::new(&mon.name)
                    .strong()
                    .size(theme::FONT_CARD_TITLE),
            );

            child_ui.label(
                egui::RichText::new(format!("Display #{}", i + 1))
                    .size(theme::FONT_CAPTION)
                    .color(child_ui.visuals().weak_text_color()),
            );

            if let Some(a) = assigned {
                child_ui.add_space(theme::SPACING_XS);
                child_ui.horizontal_centered(|ui| {
                    theme::status_dot(ui, theme::STATUS_CONNECTED, 8.0);
                    ui.label(
                        egui::RichText::new(format!("Fit: {}", a.fit_mode))
                            .size(theme::FONT_CAPTION),
                    );
                });
            }

            // Quick Assign Button on click
            if child_ui.rect_contains_pointer(mon_rect) {
                child_ui
                    .ctx()
                    .set_cursor_icon(egui::CursorIcon::PointingHand);
                if child_ui.input(|i| i.pointer.any_click())
                    && let Some(sel) = selected_wallpaper
                {
                    ipc_client.assign_wallpaper_optimistic(mon.id, sel.id, fit_mode);
                    ipc_client.send(Request::AssignWallpaper {
                        monitor_id: mon.id,
                        wallpaper_id: sel.id,
                        fit_mode: Some(fit_mode),
                    });
                }
            }

            let tooltip = if selected_wallpaper.is_some() {
                if is_assigned {
                    format!("{} — click to reassign selected wallpaper", mon.name)
                } else {
                    format!("{} — click to assign selected wallpaper", mon.name)
                }
            } else {
                format!("{} — select a wallpaper to assign", mon.name)
            };
            child_ui.response().on_hover_text(tooltip);
        }

        ui.add_space(theme::SPACING_MD);
    }

    /// Compact 36px status bar for single-monitor setups (per AGENTS.md §6).
    fn compact_status_bar(
        ui: &mut egui::Ui,
        mon: &MonitorSummary,
        selected_wallpaper: Option<&WallpaperEntry>,
        fit_mode: FitMode,
        ipc_client: &UiIpcClient,
    ) {
        let dark = ui.visuals().dark_mode;
        let bg = if dark {
            theme::BG_CARD_DARK
        } else {
            theme::BG_CARD
        };
        let (rect, response) =
            ui.allocate_at_least(egui::vec2(ui.available_width(), 36.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, theme::RADIUS_MD, bg);
        ui.painter().rect_stroke(
            rect,
            theme::RADIUS_MD,
            egui::Stroke::new(
                1.0,
                if dark {
                    theme::BORDER_SUBTLE_DARK
                } else {
                    theme::BORDER_SUBTLE
                },
            ),
            egui::StrokeKind::Outside,
        );

        let child = &mut ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        child.add_space(theme::SPACING_MD);
        child.label(
            egui::RichText::new(&mon.name)
                .strong()
                .size(theme::FONT_CARD_TITLE),
        );
        child.add_space(theme::SPACING_XS);
        theme::status_dot(child, theme::STATUS_CONNECTED, 8.0);
        child.add_space(theme::SPACING_XS);
        child.label(
            egui::RichText::new("Connected")
                .size(theme::FONT_CAPTION)
                .color(child.visuals().weak_text_color()),
        );

        child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(theme::SPACING_MD);
            if let Some(sel) = selected_wallpaper {
                let label = format!("Assign {}", theme::ICON_CHECK);
                if theme::button(ui, &label, theme::ButtonVariant::Ghost)
                    .on_hover_text(format!("Assign selected wallpaper to {}", mon.name))
                    .clicked()
                {
                    ipc_client.assign_wallpaper_optimistic(mon.id, sel.id, fit_mode);
                    ipc_client.send(Request::AssignWallpaper {
                        monitor_id: mon.id,
                        wallpaper_id: sel.id,
                        fit_mode: Some(fit_mode),
                    });
                }
            }
        });

        if selected_wallpaper.is_some() {
            response.on_hover_text("Click assign to apply the selected wallpaper");
        } else {
            response.on_hover_text("Select a wallpaper to enable assignment");
        }
        ui.add_space(theme::SPACING_MD);
    }
}
