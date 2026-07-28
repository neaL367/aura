use aura_ipc::protocol::{Request, WallpaperEntry};

use crate::ipc_client::UiIpcClient;
use crate::theme;

pub struct GalleryPanel {
    search_query: String,
}

impl Default for GalleryPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl GalleryPanel {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ipc_client: &UiIpcClient,
        selected: &mut Option<WallpaperEntry>,
    ) {
        let wallpapers = ipc_client.wallpapers();
        let available = ui.available_width();

        egui::ScrollArea::vertical()
            .id_salt("gallery_scroll")
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show(ui, |ui| {
                // Header row
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Search wallpapers...")
                            .desired_width(f32::INFINITY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::button(ui, theme::ICON_REFRESH, theme::ButtonVariant::Ghost)
                            .on_hover_text("Refresh")
                            .clicked()
                        {
                            ipc_client.send(aura_ipc::Request::RefreshLibrary);
                        }
                        ui.add_space(theme::SPACING_XS);
                        if theme::button(ui, theme::ICON_IMPORT, theme::ButtonVariant::Ghost)
                            .on_hover_text("Import files")
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
                    });
                });

                ui.add_space(theme::SPACING_MD);

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
                                        &[
                                            "png", "jpg", "jpeg", "bmp", "webp", "gif", "mp4",
                                            "webm",
                                        ],
                                    )
                                    .pick_files()
                            {
                                ipc_client.import_files(files);
                            }
                            ui.add_space(theme::SPACING_XS);
                            if theme::button(
                                ui,
                                "Choose Library Folder",
                                theme::ButtonVariant::Ghost,
                            )
                            .clicked()
                                && let Some(folder) = rfd::FileDialog::new().pick_folder()
                            {
                                ipc_client
                                    .send(aura_ipc::Request::SetWallpaperLibrary { path: folder });
                            }
                        },
                    );
                    return;
                }

                let filtered: Vec<&WallpaperEntry> = if self.search_query.is_empty() {
                    wallpapers.iter().collect()
                } else {
                    let q = self.search_query.to_lowercase();
                    wallpapers
                        .iter()
                        .filter(|w| {
                            w.path.to_string_lossy().to_lowercase().contains(&q)
                                || format!("{:?}", w.kind).to_lowercase().contains(&q)
                        })
                        .collect()
                };

                let card_w = theme::CARD_WIDTH;
                let columns = ((available - theme::SPACING_SM) / (card_w + theme::SPACING_MD))
                    .floor()
                    .max(1.0) as usize;

                egui::Grid::new("gallery_grid")
                    .min_col_width(card_w)
                    .max_col_width(card_w)
                    .spacing(egui::vec2(theme::SPACING_MD, theme::SPACING_MD))
                    .show(ui, |ui| {
                        for (i, entry) in filtered.iter().enumerate() {
                            Self::card(ui, entry, selected, ipc_client);
                            if (i + 1) % columns == 0 && i + 1 < filtered.len() {
                                ui.end_row();
                            }
                        }
                    });
            });
    }

    fn card(
        ui: &mut egui::Ui,
        entry: &WallpaperEntry,
        selected: &mut Option<WallpaperEntry>,
        ipc_client: &UiIpcClient,
    ) {
        let is_selected = selected.as_ref().is_some_and(|s| s.id == entry.id);

        let id = egui::Id::new("gallery_card").with(entry.id);
        let elevation = if is_selected {
            theme::Elevation::Raised
        } else {
            theme::Elevation::Rest
        };
        let response = theme::card_frame(ui, id, is_selected, elevation, |ui| {
            ui.set_width(theme::CARD_WIDTH);
            ui.vertical(|ui| {
                // Thumbnail
                let thumbnail_size = egui::vec2(
                    theme::CARD_WIDTH - 2.0 * theme::SPACING_MD,
                    theme::THUMBNAIL_SIZE.y,
                );
                if let Some(ref thumb) = entry.thumbnail_path {
                    let uri = theme::file_uri(thumb);
                    ui.add(
                        egui::Image::new(&uri)
                            .fit_to_exact_size(thumbnail_size)
                            .corner_radius(theme::RADIUS_SM),
                    );
                } else {
                    ui.allocate_space(thumbnail_size);
                }

                ui.add_space(theme::SPACING_XS);

                // File name row with delete button
                ui.horizontal(|ui| {
                    let file_name = entry
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_else(|| std::borrow::Cow::Borrowed("unknown"));
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(file_name.as_ref())
                                .size(theme::FONT_CARD_TITLE)
                                .color(theme::TEXT_PRIMARY),
                        )
                        .wrap(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::button(ui, theme::ICON_DELETE, theme::ButtonVariant::Ghost)
                            .on_hover_text("Delete wallpaper")
                            .clicked()
                        {
                            ipc_client.send(Request::DeleteWallpaper { id: entry.id });
                            *selected = None;
                        }
                    });
                });

                // Meta row: media badge + dimensions
                ui.horizontal(|ui| {
                    let (badge_label, variant) = match entry.kind {
                        aura_core::wallpaper::MediaKind::Image => {
                            ("IMG", theme::BadgeVariant::Image)
                        }
                        aura_core::wallpaper::MediaKind::Gif => ("GIF", theme::BadgeVariant::Gif),
                        aura_core::wallpaper::MediaKind::Video => {
                            ("VID", theme::BadgeVariant::Video)
                        }
                    };
                    theme::badge(ui, badge_label, variant);

                    if entry.width > 0 && entry.height > 0 {
                        ui.label(
                            egui::RichText::new(format!("{} × {}", entry.width, entry.height))
                                .size(theme::FONT_CAPTION)
                                .color(theme::TEXT_MUTED),
                        );
                    }
                });
            });
        });

        if response.clicked() && !is_selected {
            *selected = Some((*entry).clone());
        } else if response.clicked() {
            *selected = None;
        }
    }
}
