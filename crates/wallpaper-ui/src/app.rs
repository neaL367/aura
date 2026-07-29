use aura_ipc::protocol::WallpaperEntry;
use std::time::{Duration, Instant};

use crate::{
    gallery::GalleryPanel,
    inspector::InspectorPanel,
    ipc_client::UiIpcClient,
    settings_panel::SettingsPanel,
    sidebar::{Sidebar, Tab},
    status_bar::StatusBar,
    toast::{ToastEvent, ToastManager},
};

#[cfg(target_os = "windows")]
fn trim_ui_working_set() {
    use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

#[cfg(not(target_os = "windows"))]
fn trim_ui_working_set() {}

pub struct AuraApp {
    gallery: GalleryPanel,
    inspector: InspectorPanel,
    settings: SettingsPanel,
    status: StatusBar,
    ipc_client: UiIpcClient,
    active_tab: Tab,
    selected_wallpaper: Option<WallpaperEntry>,
    last_interaction: Instant,
    trimmed_since_idle: bool,
    toasts: ToastManager,
    toast_rx: std::sync::mpsc::Receiver<ToastEvent>,
    dark_mode: bool,
    sidebar_collapsed: bool,
}

impl AuraApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = aura_storage::config_store::ConfigStore::default_path();
        let config_store = aura_storage::config_store::ConfigStore::new(&config_path);
        let dark_mode = config_store
            .load()
            .ok()
            .map(|c| c.appearance.dark_mode)
            .unwrap_or(false);

        if dark_mode {
            crate::theme::setup_dark_theme(&cc.egui_ctx);
        } else {
            crate::theme::setup_theme(&cc.egui_ctx);
        }

        egui_extras::install_image_loaders(&cc.egui_ctx);
        let (toast_tx, toast_rx) = std::sync::mpsc::channel();

        Self {
            gallery: GalleryPanel::new(),
            inspector: InspectorPanel::new(),
            settings: SettingsPanel::new(),
            status: StatusBar::new(),
            ipc_client: UiIpcClient::new(cc.egui_ctx.clone(), toast_tx),
            active_tab: Tab::Gallery,
            selected_wallpaper: None,
            last_interaction: Instant::now(),
            trimmed_since_idle: false,
            toasts: ToastManager::new(),
            toast_rx,
            dark_mode,
            sidebar_collapsed: false,
        }
    }

    pub fn dispatch_action(&mut self, action: crate::action::UiAction) {
        use crate::action::UiAction;
        use aura_ipc::protocol::Request;
        match action {
            UiAction::AssignWallpaper {
                monitor_id,
                wallpaper_id,
                fit_mode,
            } => {
                self.ipc_client.send(Request::AssignWallpaper {
                    monitor_id,
                    wallpaper_id,
                    fit_mode,
                });
            }
            UiAction::RemoveAssignment { monitor_id } => {
                self.ipc_client
                    .send(Request::RemoveAssignment { monitor_id });
            }
            UiAction::DeleteWallpaper { wallpaper_id } => {
                self.ipc_client
                    .send(Request::DeleteWallpaper { id: wallpaper_id });
            }
            UiAction::RefreshLibrary => {
                self.ipc_client.send(Request::RefreshLibrary);
            }
            UiAction::ImportFiles { paths } => {
                self.ipc_client.import_files(paths);
            }
            UiAction::SetWallpaperLibrary { path } => {
                self.ipc_client.send(Request::SetWallpaperLibrary { path });
            }
            UiAction::PauseAll => {
                self.ipc_client.send(Request::PauseAll);
            }
            UiAction::ResumeAll => {
                self.ipc_client.send(Request::ResumeAll);
            }
        }
    }
}

impl eframe::App for AuraApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let prev_tab = self.active_tab;

        if let Some(ref config) = self.ipc_client.config()
            && config.appearance.dark_mode != self.dark_mode
        {
            self.dark_mode = config.appearance.dark_mode;
            if self.dark_mode {
                crate::theme::setup_dark_theme(ui.ctx());
            } else {
                crate::theme::setup_theme(ui.ctx());
            }
        }

        if ui.input(|i| {
            i.pointer.any_click()
                || i.pointer.any_down()
                || i.pointer.primary_down()
                || i.smooth_scroll_delta != egui::Vec2::ZERO
        }) {
            self.last_interaction = Instant::now();
            self.trimmed_since_idle = false;
        }

        // --- Bottom: Status Bar ---
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

        // --- Main content area ---
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(ui.visuals().panel_fill))
            .show(ui, |ui| {
                let full_height = ui.available_height();
                let gap = crate::theme::SPACING_SM;

                // Reserve total width: sidebar + gap + content + (gap + inspector)
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
                let content_avail = avail
                    - sidebar_w
                    - gap
                    - if show_inspector {
                        inspector_width + gap
                    } else {
                        0.0
                    };

                ui.horizontal(|ui| {
                    // Zero out default spacing — we manage gaps manually
                    ui.spacing_mut().item_spacing.x = 0.0;
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

                                crate::canvas::MonitorCanvas::show(
                                    ui,
                                    &monitors,
                                    &assignments,
                                    self.selected_wallpaper.as_ref(),
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

        // Handle file drag-and-drop import.
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

        if prev_tab != self.active_tab {
            self.selected_wallpaper = None;
        }

        if self.last_interaction.elapsed() > Duration::from_secs(3) && !self.trimmed_since_idle {
            trim_ui_working_set();
            self.trimmed_since_idle = true;
        }

        self.toasts.drain_events(&self.toast_rx);
        self.toasts.show(ui.ctx());
    }
}
