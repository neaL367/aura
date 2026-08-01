use aura_ipc::protocol::{Request, WallpaperEntry};

use crate::ipc_client::UiIpcClient;
use crate::theme;

use super::GalleryPanel;

pub(super) fn show_delete_modal(
    panel: &mut GalleryPanel,
    ui: &mut egui::Ui,
    ipc_client: &UiIpcClient,
    selected: &mut Option<WallpaperEntry>,
) {
    if let Some(ref target) = panel.delete_target.clone() {
        let mut close = false;
        let file_name = target
            .path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_else(|| std::borrow::Cow::Borrowed("this wallpaper"));

        egui::Window::new("Delete Wallpaper?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ui.ctx(), |ui| {
                ui.add_space(theme::SPACING_SM);
                ui.label(format!(
                    "Are you sure you want to permanently delete \"{}\" from disk?",
                    file_name
                ));
                ui.add_space(theme::SPACING_MD);
                ui.horizontal(|ui| {
                    if theme::button(ui, "Cancel", theme::ButtonVariant::Secondary).clicked() {
                        close = true;
                    }
                    ui.add_space(theme::SPACING_SM);
                    if theme::button(ui, "Delete", theme::ButtonVariant::Primary).clicked() {
                        ipc_client.send(Request::DeleteWallpaper { id: target.id });
                        if selected.as_ref().map(|s| s.id) == Some(target.id) {
                            *selected = None;
                        }
                        close = true;
                    }
                });
            });

        if close {
            panel.delete_target = None;
        }
    }
}
