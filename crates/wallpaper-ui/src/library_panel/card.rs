use aura_core::wallpaper::MediaKind;
use aura_ipc::protocol::{Request, WallpaperEntry};

use crate::ipc_client::UiIpcClient;
use crate::theme;

pub fn render_card(ui: &mut egui::Ui, entry: &WallpaperEntry, ipc_client: &UiIpcClient) {
    let id = ui.make_persistent_id(format!("card_{:?}", entry.id));
    let response = ui.interact(egui::Rect::NOTHING, id, egui::Sense::hover());
    let is_hovered = response.hovered();

    theme::card_frame(is_hovered, false).show(ui, |ui| {
        ui.set_width(220.0);

        ui.vertical(|ui| {
            let filename = entry
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Wallpaper");

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(filename)
                        .strong()
                        .size(14.0)
                        .color(theme::TEXT_PRIMARY),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    render_kind_badge(ui, entry.kind);
                });
            });

            ui.add_space(theme::SPACING_XS);

            render_card_preview(ui, entry);

            ui.add_space(theme::SPACING_XS);

            ui.add(
                egui::Label::new(
                    egui::RichText::new(entry.path.to_string_lossy())
                        .small()
                        .color(theme::TEXT_MUTED),
                )
                .truncate(),
            );

            ui.add_space(theme::SPACING_SM);
            render_assign_buttons(ui, entry, ipc_client);
        });
    });
}

fn render_kind_badge(ui: &mut egui::Ui, kind: MediaKind) {
    let (bg, fg, label) = match kind {
        MediaKind::Image => (theme::BADGE_IMAGE_BG, theme::BADGE_IMAGE_TEXT, "Image"),
        MediaKind::Gif => (theme::BADGE_GIF_BG, theme::BADGE_GIF_TEXT, "GIF"),
        MediaKind::Video => (theme::BADGE_VIDEO_BG, theme::BADGE_VIDEO_TEXT, "Video"),
    };

    theme::badge_frame(bg).show(ui, |ui| {
        ui.label(egui::RichText::new(label).small().strong().color(fg));
    });
}

fn render_card_preview(ui: &mut egui::Ui, entry: &WallpaperEntry) {
    if let Some(ref thumb_path) = entry.thumbnail_path {
        let path_str = thumb_path.to_string_lossy().replace('\\', "/");
        let uri = if path_str.starts_with('/') {
            format!("file://{}", path_str)
        } else {
            format!("file:///{}", path_str)
        };
        ui.add(
            egui::Image::new(uri)
                .max_size([200.0, 112.5].into())
                .corner_radius(theme::RADIUS_SM),
        );
    } else {
        egui::Frame::canvas(ui.style())
            .fill(theme::BG_APP)
            .corner_radius(theme::RADIUS_SM)
            .show(ui, |ui| {
                ui.set_min_size([200.0, 112.5].into());
                ui.set_max_size([200.0, 112.5].into());
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new("🖼 Generating thumbnail...")
                            .small()
                            .color(theme::TEXT_MUTED),
                    );
                });
            });
    }
}

fn render_assign_buttons(ui: &mut egui::Ui, entry: &WallpaperEntry, ipc_client: &UiIpcClient) {
    let status = ipc_client.status();
    match status {
        crate::ipc_client::ConnectionStatus::Connected(ref s) if !s.monitors.is_empty() => {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(theme::SPACING_XS, theme::SPACING_XS);
                for (idx, mon) in s.monitors.iter().enumerate() {
                    let btn_label = format!("Apply → Display {}", idx + 1);
                    if ui.button(btn_label).clicked() {
                        ipc_client.send(Request::AssignWallpaper {
                            monitor_id: mon.id,
                            wallpaper_id: entry.id,
                            fit_mode: None,
                        });
                    }
                }
            });
        }
        crate::ipc_client::ConnectionStatus::Connected(_) => {
            ui.label(
                egui::RichText::new("No monitors reported by daemon")
                    .small()
                    .color(theme::TEXT_MUTED),
            );
        }
        _ => {
            ui.add_enabled(false, egui::Button::new("Apply (waiting for daemon...)"));
        }
    }
}
