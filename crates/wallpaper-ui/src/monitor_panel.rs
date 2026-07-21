pub struct MonitorPanel;
impl MonitorPanel {
    pub fn new() -> Self { Self }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Monitor Assignment");
            ui.label("(Monitor layout — stub)");
        });
    }
}
