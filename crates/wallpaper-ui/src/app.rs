use aura_ipc::protocol::WallpaperEntry;
use std::time::{Duration, Instant};

use crate::{
    gallery::GalleryPanel,
    inspector::InspectorPanel,
    ipc_client::UiIpcClient,
    settings_panel::SettingsPanel,
    sidebar::{Sidebar, Tab},
    status_bar::StatusBar,
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
}

impl AuraApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::setup_theme(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        Self {
            gallery: GalleryPanel::new(),
            inspector: InspectorPanel::new(),
            settings: SettingsPanel::new(),
            status: StatusBar::new(),
            ipc_client: UiIpcClient::new(cc.egui_ctx.clone()),
            active_tab: Tab::Gallery,
            selected_wallpaper: None,
            last_interaction: Instant::now(),
        }
    }
}

impl eframe::App for AuraApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let prev_tab = self.active_tab;

        if ui.input(|i| {
            i.pointer.any_click()
                || i.pointer.any_down()
                || i.pointer.primary_down()
                || i.smooth_scroll_delta != egui::Vec2::ZERO
        }) {
            self.last_interaction = Instant::now();
        }

        // --- Bottom: Status Bar ---
        egui::Panel::bottom("status_bar")
            .frame(egui::Frame::new().fill(crate::theme::BG_CARD).inner_margin(
                egui::Margin::symmetric(
                    crate::theme::SPACING_SM as i8,
                    crate::theme::SPACING_XS as i8,
                ),
            ))
            .show(ui, |ui| {
                self.status.show(
                    ui,
                    &self.ipc_client.status(),
                    self.ipc_client.last_error().as_deref(),
                );
            });

        // --- Main content area ---
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(crate::theme::BG_APP))
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
                let sidebar_w = crate::theme::SIDEBAR_WIDTH;
                let content_avail = avail
                    - sidebar_w
                    - gap
                    - if show_inspector {
                        inspector_width + gap
                    } else {
                        0.0
                    };

                ui.horizontal(|ui| {
                    // Sidebar column
                    ui.vertical(|ui| {
                        ui.set_min_width(sidebar_w);
                        ui.set_max_width(sidebar_w);
                        ui.set_min_height(full_height);
                        Sidebar::show(ui, &mut self.active_tab);
                    });

                    ui.add(egui::Separator::default().vertical());

                    // Content column
                    ui.vertical(|ui| {
                        ui.set_min_width(content_avail.max(100.0));
                        ui.set_max_width(content_avail);
                        ui.set_min_height(full_height);
                        let content_frame = egui::Frame::new()
                            .fill(crate::theme::CONTENT_CARD)
                            .corner_radius(0.0)
                            .stroke(egui::Stroke::NONE)
                            .inner_margin(egui::Margin::symmetric(
                                crate::theme::SPACING_LG as i8,
                                crate::theme::SPACING_LG as i8,
                            ));
                        content_frame.show(ui, |ui| match self.active_tab {
                            Tab::Gallery => {
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
                        ui.vertical(|ui| {
                            ui.set_min_width(inspector_width);
                            ui.set_max_width(inspector_width);
                            ui.set_min_height(full_height);
                            if let Some(ref entry) = self.selected_wallpaper {
                                self.inspector.show(ui, entry, &self.ipc_client, &monitors);
                            } else {
                                self.inspector.show_placeholder(ui);
                            }
                        });
                    }
                });
            });

        if prev_tab != self.active_tab {
            self.selected_wallpaper = None;
            trim_ui_working_set();
        }

        if self.last_interaction.elapsed() > Duration::from_secs(3) {
            trim_ui_working_set();
            ui.ctx().request_repaint_after(Duration::from_millis(500));
        }
    }
}
