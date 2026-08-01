use aura_ipc::protocol::WallpaperEntry;

use crate::ipc_client::{ConnectionStatus, UiIpcClient};
use crate::theme;

use super::{GalleryPanel, card};

pub(super) fn show_grid(
    panel: &mut GalleryPanel,
    ui: &mut egui::Ui,
    ipc_client: &UiIpcClient,
    selected: &mut Option<WallpaperEntry>,
    status: ConnectionStatus,
    wallpapers: Vec<WallpaperEntry>,
) {
    egui::ScrollArea::vertical()
        .id_salt("gallery_scroll")
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            match status {
                ConnectionStatus::Connecting => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(theme::SPACING_LG);
                        ui.spinner();
                        ui.add_space(theme::SPACING_SM);
                        ui.label(
                            egui::RichText::new("Connecting to Aura daemon...")
                                .size(theme::FONT_BODY)
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                    return;
                }
                ConnectionStatus::Disconnected | ConnectionStatus::Error(_) => {
                    theme::empty_state(
                        ui,
                        theme::ICON_WARNING,
                        "Daemon disconnected",
                        "The background daemon is not responding. It will retry automatically.",
                        |_| {},
                    );
                    return;
                }
                ConnectionStatus::Connected(_) => {}
            }

            if wallpapers.is_empty() {
                theme::empty_state(
                    ui,
                    theme::ICON_GALLERY,
                    "No wallpapers found",
                    "Import files or set a library directory to get started.",
                    |ui| {
                        ui.add_space(theme::SPACING_MD);
                        if theme::button(ui, "Import File(s)", theme::ButtonVariant::Primary)
                            .clicked()
                            && let Some(files) = rfd::FileDialog::new()
                                .add_filter(
                                    "Media Files",
                                    &["png", "jpg", "jpeg", "bmp", "webp", "gif", "mp4", "webm"],
                                )
                                .pick_files()
                        {
                            ipc_client.import_files(files);
                        }
                        ui.add_space(theme::SPACING_XS);
                        if theme::button(ui, "Choose Library Folder", theme::ButtonVariant::Ghost)
                            .clicked()
                            && let Some(folder) = rfd::FileDialog::new().pick_folder()
                        {
                            ipc_client.set_library_path_optimistic(folder.clone());
                            ipc_client
                                .send(aura_ipc::Request::SetWallpaperLibrary { path: folder });
                        }
                    },
                );
                return;
            }

            let filtered: Vec<&WallpaperEntry> = if panel.search_query.is_empty() {
                wallpapers.iter().collect()
            } else {
                let q = panel.search_query.to_lowercase();
                wallpapers
                    .iter()
                    .filter(|w| {
                        w.path.to_string_lossy().to_lowercase().contains(&q)
                            || format!("{:?}", w.kind).to_lowercase().contains(&q)
                    })
                    .collect()
            };

            if filtered.is_empty() {
                theme::empty_state(
                    ui,
                    theme::ICON_SEARCH,
                    "No matches",
                    &format!(
                        "No wallpapers match \"{}\". Try a different search.",
                        panel.search_query
                    ),
                    |_| {},
                );
                return;
            }

            // Fluid column + card width computed inside scroll area,
            // accounting for scrollbar width so rightmost cards never overlap the scrollbar.
            let scroll_w = ui.spacing().scroll.bar_width + theme::SPACING_SM;
            let avail_w = (ui.available_width() - scroll_w).max(1.0);
            let gap = theme::SPACING_MD;
            let min_card_w = theme::CARD_MIN_WIDTH;
            let columns = ((avail_w + gap) / (min_card_w + gap)).floor().max(1.0) as usize;
            let card_w = ((avail_w - gap * (columns.saturating_sub(1) as f32)) / columns as f32)
                .max(min_card_w);

            egui::Grid::new("gallery_grid")
                .min_col_width(card_w)
                .max_col_width(card_w)
                .spacing(egui::vec2(gap, gap))
                .show(ui, |ui| {
                    for (i, entry) in filtered.iter().enumerate() {
                        if let Some(target) = card(ui, entry, selected, card_w) {
                            panel.delete_target = Some(target);
                        }
                        if (i + 1) % columns == 0 {
                            ui.end_row();
                        }
                    }
                });
        });
}
