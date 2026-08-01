use crate::sidebar::{Sidebar, Tab};

use super::AuraApp;

impl AuraApp {
    pub(super) fn show_status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status_bar")
            .frame(
                egui::Frame::new()
                    .fill(ui.visuals().window_fill)
                    .inner_margin(egui::Margin::symmetric(
                        crate::theme::SPACING_SM as i8,
                        crate::theme::SPACING_XS as i8,
                    )),
            )
            .show(ui, |ui| {
                if let Some(action) = self.status.show(
                    ui,
                    &self.ipc_client.status(),
                    self.ipc_client.last_error().as_deref(),
                ) {
                    self.dispatch_action(action);
                }
            });
    }

    pub(super) fn show_central_layout(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(ui.visuals().panel_fill))
            .show(ui, |ui| {
                let full_height = ui.available_height();
                let gap = crate::theme::SPACING_SM;
                let sep_w = 1.0;

                // Reserve total width: sidebar + gap + separator + gap + content
                // + (gap + separator + gap + inspector).
                let show_inspector = self.active_tab == Tab::Gallery;
                let avail = ui.available_width();
                let inspector_width = if show_inspector {
                    (avail * 0.28).clamp(260.0, 380.0)
                } else {
                    0.0
                };
                let sidebar_w = if self.sidebar_collapsed {
                    crate::theme::SIDEBAR_COLLAPSED_WIDTH
                } else {
                    crate::theme::SIDEBAR_EXPANDED_WIDTH
                };
                let gaps_total = if show_inspector {
                    4.0 * gap + 2.0 * sep_w
                } else {
                    2.0 * gap + sep_w
                };
                let content_avail = (avail - sidebar_w - inspector_width - gaps_total).max(100.0);

                ui.horizontal(|ui| {
                    // Single spacing value drives the actual rendered gaps
                    // between columns (matches the reserved math above).
                    ui.spacing_mut().item_spacing.x = gap;
                    // Sidebar column
                    ui.vertical(|ui| {
                        ui.set_min_width(sidebar_w);
                        ui.set_max_width(sidebar_w);
                        ui.set_min_height(full_height);
                        Sidebar::show(ui, &mut self.active_tab, &mut self.sidebar_collapsed);
                    });

                    ui.add(egui::Separator::default().vertical());

                    // Content column
                    ui.vertical(|ui| {
                        ui.set_min_width(content_avail.max(100.0));
                        ui.set_max_width(content_avail);
                        ui.set_min_height(full_height);
                        let content_frame = egui::Frame::new()
                            .fill(ui.visuals().window_fill)
                            .corner_radius(0.0)
                            .stroke(egui::Stroke::NONE)
                            .inner_margin(egui::Margin::symmetric(
                                crate::theme::SPACING_LG as i8,
                                crate::theme::SPACING_LG as i8,
                            ));
                        content_frame.show(ui, |ui| match self.active_tab {
                            Tab::Gallery => {
                                let monitors = match self.ipc_client.status() {
                                    crate::ipc_client::ConnectionStatus::Connected(ref s) => {
                                        s.monitors.clone()
                                    }
                                    _ => Vec::new(),
                                };
                                let assignments = self
                                    .ipc_client
                                    .config()
                                    .map(|c| c.assignments)
                                    .unwrap_or_default();
                                let fit_mode = self.inspector.selected_fit_mode();

                                crate::canvas::MonitorCanvas::show(
                                    ui,
                                    &monitors,
                                    &assignments,
                                    self.selected_wallpaper.as_ref(),
                                    fit_mode,
                                    &self.ipc_client,
                                );

                                self.gallery.show(
                                    ui,
                                    &self.ipc_client,
                                    &mut self.selected_wallpaper,
                                );
                            }
                            Tab::Settings => {
                                self.settings.show(ui, &self.ipc_client);
                            }
                        });
                    });

                    // Inspector column — only on Gallery page
                    if show_inspector {
                        ui.add(egui::Separator::default().vertical());
                        let monitors = match self.ipc_client.status() {
                            crate::ipc_client::ConnectionStatus::Connected(ref s) => {
                                s.monitors.clone()
                            }
                            _ => Vec::new(),
                        };
                        let assignments = self
                            .ipc_client
                            .config()
                            .map(|c| c.assignments.clone())
                            .unwrap_or_default();
                        ui.vertical(|ui| {
                            ui.set_min_width(inspector_width);
                            ui.set_max_width(inspector_width);
                            ui.set_min_height(full_height);
                            if let Some(ref entry) = self.selected_wallpaper {
                                self.inspector.show(
                                    ui,
                                    entry,
                                    &self.ipc_client,
                                    &monitors,
                                    &assignments,
                                );
                            } else {
                                self.inspector.show_placeholder(ui);
                            }
                        });
                    }
                });
            });
    }

    pub(super) fn handle_shortcuts(&mut self, ui: &mut egui::Ui) {
        enum KbShortcut {
            Refresh,
            Import,
            FocusSearch,
            TogglePause,
            DeleteSelected,
        }
        let shortcut = ui.input(|i| {
            if i.focused {
                return None;
            }
            let ctrl = i.modifiers.command;
            if ctrl && i.key_pressed(egui::Key::R) {
                Some(KbShortcut::Refresh)
            } else if ctrl && i.key_pressed(egui::Key::I) {
                Some(KbShortcut::Import)
            } else if ctrl && i.key_pressed(egui::Key::F) {
                Some(KbShortcut::FocusSearch)
            } else if ctrl && i.key_pressed(egui::Key::P) {
                Some(KbShortcut::TogglePause)
            } else if i.key_pressed(egui::Key::Delete) {
                Some(KbShortcut::DeleteSelected)
            } else {
                None
            }
        });
        match shortcut {
            Some(KbShortcut::Refresh) => {
                self.dispatch_action(crate::action::UiAction::RefreshLibrary);
            }
            Some(KbShortcut::Import) => {
                if let Some(files) = rfd::FileDialog::new()
                    .add_filter(
                        "Media Files",
                        &["png", "jpg", "jpeg", "bmp", "webp", "gif", "mp4", "webm"],
                    )
                    .pick_files()
                {
                    self.ipc_client.import_files(files);
                }
            }
            Some(KbShortcut::FocusSearch) => {
                self.gallery.request_search_focus();
            }
            Some(KbShortcut::TogglePause) => {
                let paused = match self.ipc_client.status() {
                    crate::ipc_client::ConnectionStatus::Connected(ref s) => s.is_paused,
                    _ => false,
                };
                self.dispatch_action(if paused {
                    crate::action::UiAction::ResumeAll
                } else {
                    crate::action::UiAction::PauseAll
                });
            }
            Some(KbShortcut::DeleteSelected) => {
                if let Some(ref entry) = self.selected_wallpaper {
                    self.gallery.delete_selected((*entry).clone());
                }
            }
            None => {}
        }
    }

    pub(super) fn handle_dropped_files(&mut self, ui: &mut egui::Ui) {
        let dropped: Vec<_> = ui.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            let valid = ["png", "jpg", "jpeg", "bmp", "webp", "gif", "mp4", "webm"];
            let paths: Vec<_> = dropped
                .into_iter()
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| valid.contains(&e.to_lowercase().as_str()))
                        .unwrap_or(false)
                })
                .collect();
            if !paths.is_empty() {
                self.ipc_client.import_files(paths);
            }
        }
    }
}
