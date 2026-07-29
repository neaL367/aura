use crate::theme;

pub fn show_placeholder(ui: &mut egui::Ui) {
    let frame = egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .corner_radius(0.0)
        .stroke(egui::Stroke::NONE)
        .inner_margin(egui::Margin::same(theme::SPACING_LG as i8));

    frame.show(ui, |ui| {
        ui.set_min_height(ui.available_height());
        ui.set_min_width(ui.available_width());
        egui::ScrollArea::vertical()
            .id_salt("inspector_placeholder_scroll")
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);
                    ui.label(
                        egui::RichText::new("No Selection")
                            .size(theme::FONT_SECTION_HEADER)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(theme::SPACING_SM);
                    ui.label(
                        egui::RichText::new(
                            "Click a wallpaper to see\ndetails and assign it to\na monitor.",
                        )
                        .size(theme::FONT_SECONDARY)
                        .color(theme::TEXT_MUTED),
                    );
                });
            });
    });
}
