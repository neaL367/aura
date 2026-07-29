pub const FONT_WINDOW_TITLE: f32 = 18.0;
pub const FONT_SECTION_HEADER: f32 = 16.0;
pub const FONT_CARD_TITLE: f32 = 14.0;
pub const FONT_BODY: f32 = 13.0;
pub const FONT_SECONDARY: f32 = 12.0;
pub const FONT_LABEL: f32 = 11.0;
pub const FONT_CAPTION: f32 = 10.0;

pub fn font_page_title() -> egui::FontId {
    egui::FontId::proportional(FONT_WINDOW_TITLE)
}

pub fn font_section() -> egui::FontId {
    egui::FontId::proportional(FONT_SECTION_HEADER)
}

pub fn font_body() -> egui::FontId {
    egui::FontId::proportional(FONT_BODY)
}

pub fn font_caption() -> egui::FontId {
    egui::FontId::proportional(FONT_SECONDARY)
}

pub fn font_label() -> egui::FontId {
    egui::FontId::proportional(FONT_LABEL)
}
