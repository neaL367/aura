use crate::{
    ipc_client::UiIpcClient, library_panel::LibraryPanel, monitor_panel::MonitorPanel,
    settings_panel::SettingsPanel, status_bar::StatusBar,
};

pub struct AuraApp {
    library: LibraryPanel,
    monitor: MonitorPanel,
    settings: SettingsPanel,
    status: StatusBar,
    ipc_client: UiIpcClient,
    active_tab: Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Library,
    Monitors,
    Settings,
}

impl AuraApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        Self {
            library: LibraryPanel::new(),
            monitor: MonitorPanel::new(),
            settings: SettingsPanel::new(),
            status: StatusBar::new(),
            ipc_client: UiIpcClient::new(cc.egui_ctx.clone()),
            active_tab: Tab::Library,
        }
    }
}

impl eframe::App for AuraApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("tab_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("✨ Aura Control Panel");
                ui.separator();
                ui.selectable_value(&mut self.active_tab, Tab::Library, "📁 Library");
                ui.selectable_value(&mut self.active_tab, Tab::Monitors, "🖥 Monitors");
                ui.selectable_value(&mut self.active_tab, Tab::Settings, "⚙ Settings");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⏸ Pause").clicked() {
                        self.ipc_client.send(aura_ipc::Request::PauseAll);
                    }
                    if ui.button("▶ Resume").clicked() {
                        self.ipc_client.send(aura_ipc::Request::ResumeAll);
                    }
                });
            });
        });

        egui::Panel::bottom("status_bar").show(ui, |ui| {
            self.status.show(
                ui,
                &self.ipc_client.status(),
                self.ipc_client.last_error().as_deref(),
            );
        });

        match self.active_tab {
            Tab::Library => {
                self.library.show(ui, &self.ipc_client);
            }
            Tab::Monitors => {
                self.monitor.show(ui, &self.ipc_client);
            }
            Tab::Settings => {
                self.settings.show(ui, &self.ipc_client);
            }
        }
    }
}
