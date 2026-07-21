pub struct StatusBar;
impl StatusBar {
    pub fn new() -> Self {
        Self
    }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("🔴 Daemon: not connected");
        });
    }
}
