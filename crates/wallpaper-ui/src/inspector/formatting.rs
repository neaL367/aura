use crate::theme;

pub fn meta_row(ui: &mut egui::Ui, label: &str, value: &str, _value_col_w: f32) {
    ui.label(
        egui::RichText::new(label)
            .size(theme::FONT_SECONDARY)
            .color(ui.visuals().weak_text_color()),
    );
    ui.add(egui::Label::new(egui::RichText::new(value).size(theme::FONT_BODY)).wrap());
    ui.end_row();
}

pub fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_duration(ms: u64) -> String {
    if ms >= 60_000 {
        format!("{}:{:02}", ms / 60_000, (ms % 60_000) / 1000)
    } else if ms >= 1_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        format!("{}ms", ms)
    }
}
