use egui::{Color32, Frame, Margin};

use super::super::{palette::*, spacing::*, typography::*};

pub enum BadgeVariant {
    Image,
    Gif,
    Video,
}

pub fn badge(ui: &mut egui::Ui, label: &str, variant: BadgeVariant) {
    let (bg, fg) = match variant {
        BadgeVariant::Image => (BADGE_IMAGE_BG, BADGE_IMAGE_TEXT),
        BadgeVariant::Gif => (BADGE_GIF_BG, BADGE_GIF_TEXT),
        BadgeVariant::Video => (BADGE_VIDEO_BG, BADGE_VIDEO_TEXT),
    };

    let frame = Frame::new()
        .fill(bg)
        .corner_radius(RADIUS_SM)
        .inner_margin(Margin::symmetric(SPACING_SM as i8, SPACING_XS as i8));

    frame.show(ui, |ui| {
        ui.label(
            egui::RichText::new(label)
                .size(FONT_LABEL)
                .strong()
                .color(fg),
        );
    });
}

pub fn badge_frame(bg: Color32) -> Frame {
    Frame::new()
        .fill(bg)
        .corner_radius(RADIUS_SM)
        .inner_margin(Margin::symmetric(SPACING_SM as i8, SPACING_XS as i8))
}
