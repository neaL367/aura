use crate::{
    library_panel::LibraryPanel, monitor_panel::MonitorPanel, settings_panel::SettingsPanel,
    status_bar::StatusBar,
};

pub struct AuraApp {
    library: LibraryPanel,
    monitor: MonitorPanel,
    settings: SettingsPanel,
    status: StatusBar,
    active_tab: Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Library,
    Monitors,
    Settings,
}

impl AuraApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            library: LibraryPanel::new(),
            monitor: MonitorPanel::new(),
            settings: SettingsPanel::new(),
            status: StatusBar::new(),
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
            });
        });

        egui::Panel::bottom("status_bar").show(ui, |ui| {
            self.status.show(ui);
        });

        match self.active_tab {
            Tab::Library => {
                self.library.show(ui);
            }
            Tab::Monitors => {
                self.monitor.show(ui);
            }
            Tab::Settings => {
                self.settings.show(ui);
            }
        }
    }
}
