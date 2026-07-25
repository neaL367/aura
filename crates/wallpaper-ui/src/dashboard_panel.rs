use aura_core::playback::PlaybackCommand;
use aura_core::wallpaper::{FitMode, MediaKind};
use aura_ipc::protocol::{Request, WallpaperEntry};
use std::collections::HashMap;

use crate::ipc_client::{ConnectionStatus, UiIpcClient};
use crate::theme;

pub struct DashboardPanel {
    selected_target_monitor: Option<aura_core::monitor::MonitorId>,
    selected_fit_modes: HashMap<aura_core::monitor::MonitorId, FitMode>,
    selected_wallpapers: HashMap<aura_core::monitor::MonitorId, aura_core::wallpaper::WallpaperId>,
    search_query: String,
    filter_kind: Option<MediaKind>,
}

impl Default for DashboardPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardPanel {
    pub fn new() -> Self {
        Self {
            selected_target_monitor: None,
            selected_fit_modes: HashMap::new(),
            selected_wallpapers: HashMap::new(),
            search_query: String::new(),
            filter_kind: None,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ipc_client: &UiIpcClient) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BG_APP))
            .show(ui, |ui| {
                ui.add_space(theme::SPACING_SM);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.render_monitors_section(ui, ipc_client);

                        ui.add_space(theme::SPACING_XL);
                        ui.separator();
                        ui.add_space(theme::SPACING_LG);

                        self.render_library_section(ui, ipc_client);
                    });
            });
    }

    // -----------------------------------------------------------------------
    // Top Section: Monitor Workspace & Controls
    // -----------------------------------------------------------------------

    fn render_monitors_section(&mut self, ui: &mut egui::Ui, ipc_client: &UiIpcClient) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("MONITOR WORKSPACE")
                    .size(theme::FONT_LABEL)
                    .strong()
                    .color(theme::TEXT_MUTED),
            );
        });

        ui.add_space(theme::SPACING_SM);

        let status = ipc_client.status();
        let monitors = match status {
            ConnectionStatus::Connected(ref s) if !s.monitors.is_empty() => s.monitors.clone(),
            _ => Vec::new(),
        };

        if monitors.is_empty() {
            theme::group_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    ui.add_space(theme::SPACING_MD);
                    ui.label(
                        egui::RichText::new("🖥 No active displays reported by daemon.")
                            .size(theme::FONT_BODY)
                            .strong()
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(theme::SPACING_SM);
                    if ui.button("🔄 Refresh Status").clicked() {
                        ipc_client.send(Request::GetStatus);
                    }
                    ui.add_space(theme::SPACING_MD);
                });
            });
            return;
        }

        // Auto-select first monitor as target if none selected
        if self.selected_target_monitor.is_none()
            || !monitors
                .iter()
                .any(|m| Some(m.id) == self.selected_target_monitor)
        {
            self.selected_target_monitor = Some(monitors[0].id);
        }

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(theme::SPACING_MD, theme::SPACING_MD);

            for (idx, mon) in monitors.iter().enumerate() {
                let is_target = self.selected_target_monitor == Some(mon.id);

                theme::card_frame(false, is_target).show(ui, |ui| {
                    ui.set_width(360.0);
                    ui.vertical(|ui| {
                        // Monitor Header Row
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("Display {}", idx + 1))
                                    .strong()
                                    .size(theme::FONT_SECTION_HEADER)
                                    .color(theme::TEXT_PRIMARY),
                            );

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if theme::pill_button(
                                        ui,
                                        is_target,
                                        if is_target { "Target" } else { "Set Target" },
                                    ) {
                                        self.selected_target_monitor = Some(mon.id);
                                    }
                                },
                            );
                        });

                        ui.label(
                            egui::RichText::new(&mon.name)
                                .size(theme::FONT_SECONDARY)
                                .color(theme::TEXT_MUTED),
                        );

                        ui.add_space(theme::SPACING_XS);
                        ui.separator();
                        ui.add_space(theme::SPACING_XS);

                        // Fit Mode Pills
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Fit Mode:")
                                    .size(theme::FONT_SECONDARY)
                                    .strong(),
                            );
                            let current_fit = self
                                .selected_fit_modes
                                .get(&mon.id)
                                .copied()
                                .unwrap_or_default();

                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing =
                                    egui::vec2(theme::SPACING_XS, theme::SPACING_XS);
                                for mode in [
                                    FitMode::Fill,
                                    FitMode::Fit,
                                    FitMode::Stretch,
                                    FitMode::Center,
                                    FitMode::Tile,
                                    FitMode::Span,
                                ] {
                                    if theme::pill_button(
                                        ui,
                                        current_fit == mode,
                                        &format!("{}", mode),
                                    ) {
                                        self.selected_fit_modes.insert(mon.id, mode);
                                        ipc_client.send(Request::SetFitMode {
                                            monitor_id: mon.id,
                                            fit_mode: mode,
                                        });
                                    }
                                }
                            });
                        });

                        ui.add_space(theme::SPACING_XS);

                        // Playback Control Pills
                        ui.horizontal(|ui| {
                            if ui.button("⏸ Pause").clicked() {
                                ipc_client.send(Request::SetPlayback {
                                    monitor_id: mon.id,
                                    command: PlaybackCommand::Pause,
                                });
                            }
                            if ui.button("▶ Play").clicked() {
                                ipc_client.send(Request::SetPlayback {
                                    monitor_id: mon.id,
                                    command: PlaybackCommand::Play,
                                });
                            }
                            if ui.button("🔄 Restart").clicked() {
                                ipc_client.send(Request::SetPlayback {
                                    monitor_id: mon.id,
                                    command: PlaybackCommand::Loop,
                                });
                            }
                            if ui.button("❌ Clear").clicked() {
                                ipc_client.send(Request::RemoveAssignment { monitor_id: mon.id });
                                self.selected_wallpapers.remove(&mon.id);
                            }
                        });
                    });
                });
            }
        });
    }

    // -----------------------------------------------------------------------
    // Bottom Section: Responsive Y-Scrollable Wallpaper Gallery Grid
    // -----------------------------------------------------------------------

    fn render_library_section(&mut self, ui: &mut egui::Ui, ipc_client: &UiIpcClient) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("WALLPAPER GALLERY")
                    .size(theme::FONT_LABEL)
                    .strong()
                    .color(theme::TEXT_MUTED),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄 Refresh").clicked() {
                    ipc_client.send(Request::RefreshLibrary);
                }
                ui.add_space(theme::SPACING_XS);
                if ui.button("📄 Add File(s)...").clicked() {
                    self.pick_files(ipc_client);
                }
                ui.add_space(theme::SPACING_XS);
                if ui.button("➕ Add Folder...").clicked() {
                    self.pick_folder(ipc_client);
                }
            });
        });

        ui.add_space(theme::SPACING_SM);

        // Search & Category Filter Toolbar
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text("🔍 Search wallpapers...")
                    .desired_width(260.0),
            );

            if !self.search_query.is_empty() && ui.button("✖").clicked() {
                self.search_query.clear();
            }

            ui.add_space(theme::SPACING_MD);
            ui.separator();
            ui.add_space(theme::SPACING_MD);

            ui.label(
                egui::RichText::new("Filter:")
                    .size(theme::FONT_SECONDARY)
                    .strong(),
            );

            if theme::pill_button(ui, self.filter_kind.is_none(), "All") {
                self.filter_kind = None;
            }
            if theme::pill_button(ui, self.filter_kind == Some(MediaKind::Image), "🖼 Images") {
                self.filter_kind = Some(MediaKind::Image);
            }
            if theme::pill_button(ui, self.filter_kind == Some(MediaKind::Gif), "🎞 GIFs") {
                self.filter_kind = Some(MediaKind::Gif);
            }
            if theme::pill_button(ui, self.filter_kind == Some(MediaKind::Video), "🎬 Videos") {
                self.filter_kind = Some(MediaKind::Video);
            }
        });

        ui.add_space(theme::SPACING_MD);

        let wallpapers = ipc_client.wallpapers();

        // Filter wallpapers
        let filtered_wallpapers: Vec<_> = wallpapers
            .into_iter()
            .filter(|w| {
                if let Some(kind) = self.filter_kind
                    && w.kind != kind
                {
                    return false;
                }
                if !self.search_query.trim().is_empty() {
                    let q = self.search_query.to_lowercase();
                    let filename = w
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let full_path = w.path.to_string_lossy().to_lowercase();
                    if !filename.contains(&q) && !full_path.contains(&q) {
                        return false;
                    }
                }
                true
            })
            .collect();

        if filtered_wallpapers.is_empty() {
            theme::group_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    ui.add_space(theme::SPACING_XL);
                    ui.label(
                        egui::RichText::new("🖼 No wallpapers match your filter")
                            .size(theme::FONT_SECTION_HEADER)
                            .strong()
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(theme::SPACING_SM);
                    ui.label(
                        egui::RichText::new(
                            "Click 'Add Folder...' or 'Add File(s)...' above to add media.",
                        )
                        .size(theme::FONT_SECONDARY)
                        .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(theme::SPACING_XL);
                });
            });
            return;
        }

        // Dynamically compute grid columns based on container width
        let card_width: f32 = 220.0;
        let gap: f32 = theme::SPACING_MD;
        let avail_width = ui.available_width();
        let cols = ((avail_width + gap) / (card_width + gap)).floor().max(1.0) as usize;

        for chunk in filtered_wallpapers.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
                for entry in chunk {
                    self.render_wallpaper_card(ui, entry, ipc_client);
                }
            });
            ui.add_space(gap);
        }
    }

    fn render_wallpaper_card(
        &mut self,
        ui: &mut egui::Ui,
        entry: &WallpaperEntry,
        ipc_client: &UiIpcClient,
    ) {
        let id = ui.make_persistent_id(format!("dash_card_{:?}", entry.id));
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
                            .size(theme::FONT_CARD_TITLE)
                            .color(theme::TEXT_PRIMARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = match entry.kind {
                            MediaKind::Image => "Image",
                            MediaKind::Gif => "GIF",
                            MediaKind::Video => "Video",
                        };
                        theme::badge_frame(theme::BADGE_IMAGE_BG).show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(label)
                                    .size(theme::FONT_LABEL)
                                    .strong()
                                    .color(theme::BADGE_IMAGE_TEXT),
                            );
                        });
                    });
                });

                ui.add_space(theme::SPACING_XS);

                // Preview Thumbnail
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
                                    egui::RichText::new("🖼 Generating...")
                                        .size(theme::FONT_SECONDARY)
                                        .color(theme::TEXT_MUTED),
                                );
                            });
                        });
                }

                ui.add_space(theme::SPACING_XS);

                ui.add(
                    egui::Label::new(
                        egui::RichText::new(entry.path.to_string_lossy())
                            .size(theme::FONT_SECONDARY)
                            .color(theme::TEXT_MUTED),
                    )
                    .truncate(),
                );

                ui.add_space(theme::SPACING_SM);

                // Apply Actions
                let status = ipc_client.status();
                match status {
                    ConnectionStatus::Connected(ref s) if !s.monitors.is_empty() => {
                        if s.monitors.len() == 1 {
                            let mon = &s.monitors[0];
                            if ui.button("Apply Wallpaper").clicked() {
                                ipc_client.send(Request::AssignWallpaper {
                                    monitor_id: mon.id,
                                    wallpaper_id: entry.id,
                                    fit_mode: None,
                                });
                            }
                        } else {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing =
                                    egui::vec2(theme::SPACING_XS, theme::SPACING_XS);
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
                    }
                    _ => {
                        ui.add_enabled(false, egui::Button::new("Apply (waiting for daemon...)"));
                    }
                }
            });
        });
    }

    fn pick_folder(&self, ipc_client: &UiIpcClient) {
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            ipc_client.send(Request::AddScanPath { path: folder });
        }
    }

    fn pick_files(&self, ipc_client: &UiIpcClient) {
        let files = rfd::FileDialog::new()
            .add_filter(
                "Media Files",
                &["png", "jpg", "jpeg", "bmp", "webp", "gif", "mp4", "webm"],
            )
            .pick_files();
        if let Some(files) = files {
            for file in files {
                ipc_client.send(Request::AddScanPath { path: file });
            }
        }
    }
}
