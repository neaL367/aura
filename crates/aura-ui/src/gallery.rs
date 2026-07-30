use aura_ipc::protocol::{Request, WallpaperEntry};

use crate::ipc_client::UiIpcClient;
use crate::theme;

pub struct GalleryPanel {
    search_query: String,
    delete_target: Option<WallpaperEntry>,
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
            delete_target: None,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ipc_client: &UiIpcClient,
        selected: &mut Option<WallpaperEntry>,
    ) {
        let wallpapers = ipc_client.wallpapers();

        // --- Search + action button header ---
        // Right-to-left: buttons on right, search fills remaining left space.
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::button(ui, theme::ICON_REFRESH, theme::ButtonVariant::Ghost)
                    .on_hover_text("Refresh library")
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
                // Search box fills remaining width (added last in rtl layout = leftmost).
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text(format!("{} Search wallpapers...", theme::ICON_SEARCH))
                        .desired_width(260.0),
                );
            });
        });

        ui.add_space(theme::SPACING_MD);

        egui::ScrollArea::vertical()
            .id_salt("gallery_scroll")
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
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

                // Fluid column + card width computed inside scroll area,
                // accounting for scrollbar width so rightmost cards never overlap the scrollbar.
                let scroll_w = ui.spacing().scroll.bar_width + theme::SPACING_SM;
                let avail_w = (ui.available_width() - scroll_w).max(1.0);
                let gap = theme::SPACING_MD;
                let min_card_w = theme::CARD_MIN_WIDTH;
                let columns = ((avail_w + gap) / (min_card_w + gap)).floor().max(1.0) as usize;
                let card_w = ((avail_w - gap * (columns.saturating_sub(1) as f32))
                    / columns as f32)
                    .max(min_card_w);

                egui::Grid::new("gallery_grid")
                    .min_col_width(card_w)
                    .max_col_width(card_w)
                    .spacing(egui::vec2(gap, gap))
                    .show(ui, |ui| {
                        for (i, entry) in filtered.iter().enumerate() {
                            if let Some(target) = Self::card(ui, entry, selected, card_w) {
                                self.delete_target = Some(target);
                            }
                            if (i + 1) % columns == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });

        // Delete confirmation modal dialog
        if let Some(ref target) = self.delete_target.clone() {
            let mut close = false;
            let file_name = target
                .path
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_else(|| std::borrow::Cow::Borrowed("this wallpaper"));

            egui::Window::new("Delete Wallpaper?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ui.ctx(), |ui| {
                    ui.add_space(theme::SPACING_SM);
                    ui.label(format!(
                        "Are you sure you want to permanently delete \"{}\" from disk?",
                        file_name
                    ));
                    ui.add_space(theme::SPACING_MD);
                    ui.horizontal(|ui| {
                        if theme::button(ui, "Cancel", theme::ButtonVariant::Secondary).clicked() {
                            close = true;
                        }
                        ui.add_space(theme::SPACING_SM);
                        if theme::button(ui, "Delete", theme::ButtonVariant::Primary).clicked() {
                            ipc_client.send(Request::DeleteWallpaper { id: target.id });
                            if selected.as_ref().map(|s| s.id) == Some(target.id) {
                                *selected = None;
                            }
                            close = true;
                        }
                    });
                });

            if close {
                self.delete_target = None;
            }
        }
    }

    fn card(
        ui: &mut egui::Ui,
        entry: &WallpaperEntry,
        selected: &mut Option<WallpaperEntry>,
        card_w: f32,
    ) -> Option<WallpaperEntry> {
        let mut delete_clicked = None;
        let is_selected = selected.as_ref().is_some_and(|s| s.id == entry.id);
        let id = egui::Id::new("gallery_card").with(entry.id);
        let elevation = if is_selected {
            theme::Elevation::Raised
        } else {
            theme::Elevation::Rest
        };

        let response = theme::card_frame(ui, id, is_selected, elevation, |ui| {
            // Inner width = card_w minus frame inner_margin * 2 (SPACING_MD each side).
            let inner_w = (card_w - 2.0 * theme::SPACING_MD).max(1.0);
            ui.set_width(inner_w);
            ui.vertical(|ui| {
                // 16:9 thumbnail.
                let thumb_h = (inner_w * 9.0 / 16.0).round();
                let thumbnail_size = egui::vec2(inner_w, thumb_h);

                if let Some(ref thumb) = entry.thumbnail_path {
                    let uri = theme::file_uri(thumb);
                    ui.add(
                        egui::Image::new(&uri)
                            .fit_to_exact_size(thumbnail_size)
                            .maintain_aspect_ratio(false)
                            .corner_radius(theme::RADIUS_SM),
                    );
                } else {
                    let (rect, _) = ui.allocate_exact_size(thumbnail_size, egui::Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        theme::RADIUS_SM,
                        ui.visuals().widgets.noninteractive.bg_fill,
                    );
                }

                ui.add_space(theme::SPACING_XS);

                // Filename row - truncated single-line + delete button (fixed 28px height).
                let card_hovered =
                    is_selected || ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
                ui.allocate_ui_with_layout(
                    egui::vec2(inner_w, 28.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let name_w = (inner_w - 32.0).max(20.0);
                        let file_name = entry
                            .path
                            .file_name()
                            .map(|n| n.to_string_lossy())
                            .unwrap_or_else(|| std::borrow::Cow::Borrowed("unknown"));
                        ui.allocate_ui_with_layout(
                            egui::vec2(name_w, 28.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(file_name.as_ref())
                                            .size(theme::FONT_CARD_TITLE),
                                    )
                                    .truncate(),
                                );
                            },
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().interact_size = egui::vec2(28.0, 28.0);
                            if card_hovered {
                                if theme::button(
                                    ui,
                                    theme::ICON_DELETE,
                                    theme::ButtonVariant::Ghost,
                                )
                                .on_hover_text("Delete wallpaper")
                                .clicked()
                                {
                                    delete_clicked = Some((*entry).clone());
                                }
                            } else {
                                ui.add_space(28.0);
                            }
                        });
                    },
                );

                // Meta row: badge + dimensions.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = theme::SPACING_XS;
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
                        if entry.width < 1920 || entry.height < 1080 {
                            theme::badge(ui, "LOW RES", theme::BadgeVariant::Gif);
                        }
                        let res_tag = if entry.width >= 3840 || entry.height >= 2160 {
                            format!("4K ({} \u{00d7} {})", entry.width, entry.height)
                        } else if entry.width >= 2560 || entry.height >= 1440 {
                            format!("2K ({} \u{00d7} {})", entry.width, entry.height)
                        } else if entry.width >= 1920 || entry.height >= 1080 {
                            format!("1080p ({} \u{00d7} {})", entry.width, entry.height)
                        } else {
                            format!("{} \u{00d7} {}", entry.width, entry.height)
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(res_tag)
                                    .size(theme::FONT_CAPTION)
                                    .color(ui.visuals().weak_text_color()),
                            )
                            .truncate(),
                        );
                    }
                });
            });
        });

        if response.clicked() && delete_clicked.is_none() && !is_selected {
            *selected = Some((*entry).clone());
        } else if response.clicked() && delete_clicked.is_none() {
            *selected = None;
        }

        delete_clicked
    }
}
