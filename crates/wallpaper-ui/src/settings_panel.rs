pub struct SettingsPanel;
impl SettingsPanel {
    pub fn new() -> Self {
        Self
    }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Settings");
            ui.label("(Settings — stub)");
        });
    }
}
