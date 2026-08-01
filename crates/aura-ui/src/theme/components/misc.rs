use egui::Color32;

pub fn status_dot(ui: &mut egui::Ui, color: Color32, size: f32) {
    ui.label(egui::RichText::new("●").size(size).color(color));
}

pub fn connection_dot(ui: &mut egui::Ui, color: Color32) {
    status_dot(ui, color, 10.0);
}
