use std::collections::HashMap;
use std::path::PathBuf;
use egui::{Color32, RichText, TextureHandle, Ui, Vec2};
use crate::library::model::WallpaperLibraryEntry;
use crate::config::model::AppConfig;
use crate::platform::windows::file_dialog::pick_wallpaper_file;
use crate::utils::error::Result;

/// Cache mapping stable library entry IDs to egui-managed GPU TextureHandles.
pub struct ThumbnailCache {
    handles: HashMap<String, TextureHandle>,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        Self { handles: HashMap::new() }
    }

    /// Queries the cache for an entry's TextureHandle, loading and registering it if not present.
    /// If the thumbnail doesn't exist on disk, generates it on-demand first.
    pub fn get_or_load(&mut self, ctx: &egui::Context, entry: &WallpaperLibraryEntry) -> TextureHandle {
        if let Some(existing) = self.handles.get(&entry.id) {
            return existing.clone();
        }

        let handle = match load_thumbnail_as_color_image(entry) {
            Ok(color_image) => ctx.load_texture(&entry.id, color_image, egui::TextureOptions::default()),
            Err(e) => {
                tracing::warn!("Failed to load thumbnail for {}: {e}", entry.id);
                ctx.load_texture(&entry.id, placeholder_color_image(), egui::TextureOptions::default())
            }
        };

        self.handles.insert(entry.id.clone(), handle.clone());
        handle
    }

    /// Evicts an entry from the egui texture cache on deletion.
    pub fn invalidate(&mut self, entry_id: &str) {
        self.handles.remove(entry_id);
    }
}

/// Actions representing grid interaction events handled by the parent UI container.
pub enum LibraryGridAction {
    Picked(WallpaperLibraryEntry),
    RequestRemove(String),
}

/// Renders the scrollable grid of available wallpapers.
pub fn show(
    ui: &mut Ui,
    config: &mut AppConfig,
    thumbnails: &mut ThumbnailCache,
) -> Option<LibraryGridAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        ui.heading("Library");
        if ui.button("+ Add wallpaper...").clicked() {
            match pick_wallpaper_file(Default::default()) {
                Ok(Some(path)) => match crate::library::scanner::add_entry(config, path) {
                    Ok((entry, was_new)) => {
                        if !was_new {
                            tracing::info!("{} is already in the library", entry.path.display());
                        }
                        action = Some(LibraryGridAction::Picked(entry));
                    }
                    Err(e) => tracing::warn!("Failed to add library entry: {e}"),
                },
                Ok(None) => {}
                Err(e) => tracing::warn!("File picker failed: {e}"),
            }
        }
    });

    ui.add_space(8.0);

    if config.library.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No wallpapers yet — click \"Add wallpaper...\" to get started").weak());
        });
        return action;
    }

    const TILE_SIZE: Vec2 = Vec2::new(160.0, 90.0);
    const COLUMNS_MIN_WIDTH: f32 = 180.0;

    egui::ScrollArea::vertical().show(ui, |ui| {
        let available_width = ui.available_width();
        let columns = ((available_width / COLUMNS_MIN_WIDTH).floor() as usize).max(1);

        egui::Grid::new("library_grid")
            .num_columns(columns)
            .spacing(Vec2::new(12.0, 12.0))
            .show(ui, |ui| {
                for (i, entry) in config.library.iter().enumerate() {
                    if let Some(clicked_action) = grid_tile(ui, entry, thumbnails, TILE_SIZE) {
                        action = Some(clicked_action);
                    }
                    if (i + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
    });

    action
}

fn grid_tile(
    ui: &mut Ui,
    entry: &WallpaperLibraryEntry,
    thumbnails: &mut ThumbnailCache,
    tile_size: Vec2,
) -> Option<LibraryGridAction> {
    let mut action = None;

    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(tile_size.x);
        ui.vertical(|ui| {
            let handle = thumbnails.get_or_load(ui.ctx(), entry);
            let response = ui.add(
                egui::Button::image(egui::Image::new(&handle).fit_to_exact_size(tile_size))
                    .frame(false),
            );

            if response.clicked() {
                action = Some(LibraryGridAction::Picked(entry.clone()));
            }

            if !entry.path.exists() {
                ui.label(RichText::new("⚠ File missing").color(Color32::from_rgb(220, 120, 60)).small());
            }

            ui.horizontal(|ui| {
                let file_name = entry.path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "(unknown)".to_string());
                ui.label(RichText::new(truncate(&file_name, 18)).small());

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("🗑").clicked() {
                        action = Some(LibraryGridAction::RequestRemove(entry.id.clone()));
                    }
                });
            });
        });
    });

    action
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_chars.saturating_sub(1)).collect::<String>())
    }
}

fn load_thumbnail_as_color_image(entry: &WallpaperLibraryEntry) -> Result<egui::ColorImage> {
    let thumb_path: PathBuf = crate::library::thumbnail::cached_thumbnail_path(entry);
    if !thumb_path.exists() {
        crate::library::thumbnail::generate_thumbnail(&entry.path, &thumb_path, 256)?;
    }
    let (width, height, rgba) = crate::library::thumbnail::decode_png_rgba(&thumb_path)?;
    Ok(egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba))
}

fn placeholder_color_image() -> egui::ColorImage {
    egui::ColorImage::new([160, 90], vec![Color32::from_rgb(60, 60, 60); 160 * 90])
}
