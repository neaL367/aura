use std::time::{Duration, Instant};

use crate::{
    dashboard_panel::DashboardPanel, ipc_client::UiIpcClient, settings_panel::SettingsPanel,
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
    dashboard: DashboardPanel,
    settings: SettingsPanel,
    status: StatusBar,
    ipc_client: UiIpcClient,
    active_tab: Tab,
    last_interaction: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Dashboard,
    Settings,
}

impl AuraApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::setup_theme(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        Self {
            dashboard: DashboardPanel::new(),
            settings: SettingsPanel::new(),
            status: StatusBar::new(),
            ipc_client: UiIpcClient::new(cc.egui_ctx.clone()),
            active_tab: Tab::Dashboard,
            last_interaction: Instant::now(),
        }
    }
}

impl eframe::App for AuraApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let prev_tab = self.active_tab;
        let ctx = ui.ctx().clone();

        if ui.input(|i| {
            i.pointer.any_click()
                || i.pointer.any_down()
                || i.pointer.primary_down()
                || i.smooth_scroll_delta != egui::Vec2::ZERO
        }) {
            self.last_interaction = Instant::now();
        }

        crate::theme::header_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(crate::theme::SPACING_SM);
                ui.label(
                    egui::RichText::new("✨ Aura Wallpaper")
                        .strong()
                        .size(17.0)
                        .color(crate::theme::TEXT_PRIMARY),
                );
                ui.add_space(crate::theme::SPACING_MD);
                ui.separator();
                ui.add_space(crate::theme::SPACING_MD);

                if crate::theme::pill_button(ui, self.active_tab == Tab::Dashboard, "⚡ Dashboard")
                {
                    self.active_tab = Tab::Dashboard;
                }
                ui.add_space(crate::theme::SPACING_XS);
                if crate::theme::pill_button(ui, self.active_tab == Tab::Settings, "⚙ Settings") {
                    self.active_tab = Tab::Settings;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⏸ Pause All").clicked() {
                        self.ipc_client.send(aura_ipc::Request::PauseAll);
                    }
                    ui.add_space(crate::theme::SPACING_XS);
                    if ui.button("▶ Resume All").clicked() {
                        self.ipc_client.send(aura_ipc::Request::ResumeAll);
                    }
                });
            });
        });

        if prev_tab != self.active_tab {
            trim_ui_working_set();
        }

        egui::Panel::bottom("status_bar").show(ui, |ui| {
            self.status.show(
                ui,
                &self.ipc_client.status(),
                self.ipc_client.last_error().as_deref(),
            );
        });

        match self.active_tab {
            Tab::Dashboard => {
                self.dashboard.show(ui, &self.ipc_client);
            }
            Tab::Settings => {
                self.settings.show(ui, &self.ipc_client);
            }
        }

        if self.last_interaction.elapsed() > Duration::from_secs(3) {
            trim_ui_working_set();
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }
}
