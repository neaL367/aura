use aura_ipc::protocol::WallpaperEntry;

use crate::ipc_client::UiIpcClient;
use crate::theme;

mod card;
mod grid;
mod modal;
use card::card;
use grid::show_grid;
use modal::show_delete_modal;

pub struct GalleryPanel {
    pub(super) search_query: String,
    pub(super) delete_target: Option<WallpaperEntry>,
    pub(super) focus_search: bool,
}

impl Default for GalleryPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl GalleryPanel {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            delete_target: None,
            focus_search: false,
        }
    }

    /// Ask the search field to take keyboard focus on the next frame
    /// (e.g. from the Ctrl+F shortcut).
    pub fn request_search_focus(&mut self) {
        self.focus_search = true;
    }

    /// Open the delete-confirmation dialog for the given entry.
    pub fn delete_selected(&mut self, entry: WallpaperEntry) {
        self.delete_target = Some(entry);
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ipc_client: &UiIpcClient,
        selected: &mut Option<WallpaperEntry>,
    ) {
        let status = ipc_client.status();
        let wallpapers = ipc_client.wallpapers();

        // --- Search + action button header ---
        // Right-to-left: buttons on right, search fills remaining left space.
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::button(ui, theme::ICON_REFRESH, theme::ButtonVariant::Ghost)
                    .on_hover_text("Refresh library (Ctrl+R)")
                    .clicked()
                {
                    ipc_client.send(aura_ipc::Request::RefreshLibrary);
                }
                ui.add_space(theme::SPACING_XS);
                if theme::button(ui, theme::ICON_IMPORT, theme::ButtonVariant::Ghost)
                    .on_hover_text("Import files (Ctrl+I)")
                    .clicked()
                    && let Some(files) = rfd::FileDialog::new()
                        .add_filter(
                            "Media Files",
                            &["png", "jpg", "jpeg", "bmp", "webp", "gif", "mp4", "webm"],
                        )
                        .pick_files()
                {
                    ipc_client.import_files(files);
                }
                // Search box fills remaining width (added last in rtl layout = leftmost).
                let search_resp = ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text(format!("{} Search wallpapers...", theme::ICON_SEARCH))
                        .desired_width(260.0),
                );
                if self.focus_search {
                    search_resp.request_focus();
                    self.focus_search = false;
                }
            });
        });

        ui.add_space(theme::SPACING_MD);

        show_grid(self, ui, ipc_client, selected, status, wallpapers);

        // Delete confirmation modal dialog
        show_delete_modal(self, ui, ipc_client, selected);
    }
}
