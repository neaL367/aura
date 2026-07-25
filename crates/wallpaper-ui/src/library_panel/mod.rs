pub mod card;

use aura_core::wallpaper::MediaKind;
use aura_ipc::protocol::Request;

use crate::ipc_client::UiIpcClient;
use crate::theme;

pub struct LibraryPanel {
    search_query: String,
    filter_kind: Option<MediaKind>,
}

impl Default for LibraryPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryPanel {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            filter_kind: None,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ipc_client: &UiIpcClient) {
        ui.add_space(theme::SPACING_SM);

        // Header & Toolbar
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("📁 Wallpaper Library")
                    .strong()
                    .size(20.0)
                    .color(theme::TEXT_PRIMARY),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄 Refresh").clicked() {
                    ipc_client.send(Request::RefreshLibrary);
                }
                ui.add_space(theme::SPACING_XS);
                if ui.button("📄 Add File(s)...").clicked() {
                    self.pick_files(ipc_client);
                }
                ui.add_space(theme::SPACING_XS);
                if ui.button("➕ Add Folder...").clicked() {
                    self.pick_folder(ipc_client);
                }
            });
        });

        ui.add_space(theme::SPACING_MD);

        // Search & Category Filter Controls
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text("🔍 Search wallpapers...")
                    .desired_width(260.0),
            );

            if !self.search_query.is_empty() && ui.button("✖").clicked() {
                self.search_query.clear();
            }

            ui.add_space(theme::SPACING_MD);
            ui.separator();
            ui.add_space(theme::SPACING_MD);

            ui.label("Filter:");
            if ui
                .selectable_label(self.filter_kind.is_none(), "All")
                .clicked()
            {
                self.filter_kind = None;
            }
            if ui
                .selectable_label(self.filter_kind == Some(MediaKind::Image), "🖼 Images")
                .clicked()
            {
                self.filter_kind = Some(MediaKind::Image);
            }
            if ui
                .selectable_label(self.filter_kind == Some(MediaKind::Gif), "🎞 GIFs")
                .clicked()
            {
                self.filter_kind = Some(MediaKind::Gif);
            }
            if ui
                .selectable_label(self.filter_kind == Some(MediaKind::Video), "🎬 Videos")
                .clicked()
            {
                self.filter_kind = Some(MediaKind::Video);
            }
        });

        ui.add_space(theme::SPACING_MD);
        ui.separator();
        ui.add_space(theme::SPACING_MD);

        let wallpapers = ipc_client.wallpapers();

        // Apply filtering
        let filtered_wallpapers: Vec<_> = wallpapers
            .into_iter()
            .filter(|w| {
                if let Some(kind) = self.filter_kind
                    && w.kind != kind
                {
                    return false;
                }
                if !self.search_query.trim().is_empty() {
                    let q = self.search_query.to_lowercase();
                    let filename = w
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let full_path = w.path.to_string_lossy().to_lowercase();
                    if !filename.contains(&q) && !full_path.contains(&q) {
                        return false;
                    }
                }
                true
            })
            .collect();

        if filtered_wallpapers.is_empty() {
            theme::group_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    ui.add_space(theme::SPACING_XL);
                    ui.label(
                        egui::RichText::new("🖼 No wallpapers match filter")
                            .strong()
                            .size(16.0)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(theme::SPACING_SM);
                    ui.label(
                        egui::RichText::new("Add folders or media files using the buttons above.")
                            .small()
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(theme::SPACING_XL);
                });
            });
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(theme::SPACING_MD, theme::SPACING_MD);
                for entry in filtered_wallpapers {
                    card::render_card(ui, &entry, ipc_client);
                }
            });
        });
    }

    fn pick_folder(&self, ipc_client: &UiIpcClient) {
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            ipc_client.send(Request::AddScanPath { path: folder });
        }
    }

    fn pick_files(&self, ipc_client: &UiIpcClient) {
        let files = rfd::FileDialog::new()
            .add_filter(
                "Media Files",
                &["png", "jpg", "jpeg", "bmp", "webp", "gif", "mp4", "webm"],
            )
            .pick_files();
        if let Some(files) = files {
            for file in files {
                ipc_client.send(Request::AddScanPath { path: file });
            }
        }
    }
}
