pub struct LibraryPanel;
impl LibraryPanel {
    pub fn new() -> Self {
        Self
    }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Wallpaper Library");
            ui.label("(Library browser — stub)");
        });
    }
}
