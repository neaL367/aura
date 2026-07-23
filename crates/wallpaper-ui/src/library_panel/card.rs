use aura_core::wallpaper::MediaKind;
use aura_ipc::protocol::{Request, WallpaperEntry};

use crate::ipc_client::UiIpcClient;

pub fn render_card(ui: &mut egui::Ui, entry: &WallpaperEntry, ipc_client: &UiIpcClient) {
    egui::Frame::group(ui.style())
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_width(220.0);

            ui.vertical(|ui| {
                let filename = entry
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Wallpaper");

                ui.label(egui::RichText::new(filename).strong().heading());

                let badge = match entry.kind {
                    MediaKind::Image => "🖼 Image",
                    MediaKind::Gif => "🎞 GIF",
                    MediaKind::Video => "🎬 Video",
                };
                ui.label(badge);

                render_card_preview(ui, entry);

                ui.add(
                    egui::Label::new(
                        egui::RichText::new(entry.path.to_string_lossy())
                            .small()
                            .color(egui::Color32::GRAY),
                    )
                    .truncate(),
                );

                ui.add_space(6.0);
                render_assign_buttons(ui, entry, ipc_client);
            });
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
                .corner_radius(4.0),
        );
    } else {
        egui::Frame::canvas(ui.style())
            .fill(egui::Color32::from_rgb(35, 35, 42))
            .corner_radius(4.0)
            .show(ui, |ui| {
                ui.set_min_size([200.0, 112.5].into());
                ui.set_max_size([200.0, 112.5].into());
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new("🖼 Generating thumbnail...")
                            .small()
                            .color(egui::Color32::GRAY),
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
                    .color(egui::Color32::GRAY),
            );
        }
        _ => {
            ui.add_enabled(false, egui::Button::new("Apply (waiting for daemon...)"));
        }
    }
}
