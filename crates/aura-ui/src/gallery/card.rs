use aura_ipc::protocol::WallpaperEntry;

use crate::theme;

pub(super) fn card(
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
                            if theme::button(ui, theme::ICON_DELETE, theme::ButtonVariant::Ghost)
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
                    aura_core::wallpaper::MediaKind::Image => ("IMG", theme::BadgeVariant::Image),
                    aura_core::wallpaper::MediaKind::Gif => ("GIF", theme::BadgeVariant::Gif),
                    aura_core::wallpaper::MediaKind::Video => ("VID", theme::BadgeVariant::Video),
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
