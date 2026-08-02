use aura_ipc::protocol::WallpaperEntry;
use std::time::{Duration, Instant};

use crate::{
    gallery::GalleryPanel,
    inspector::InspectorPanel,
    ipc_client::UiIpcClient,
    settings_panel::SettingsPanel,
    sidebar::Tab,
    status_bar::StatusBar,
    toast::{ToastEvent, ToastManager},
};

mod layout;

#[cfg(target_os = "windows")]
fn trim_ui_working_set() {
    use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

#[cfg(not(target_os = "windows"))]
fn trim_ui_working_set() {}

pub struct AuraApp {
    pub(super) gallery: GalleryPanel,
    pub(super) inspector: InspectorPanel,
    pub(super) settings: SettingsPanel,
    pub(super) status: StatusBar,
    pub(super) ipc_client: UiIpcClient,
    pub(super) active_tab: Tab,
    pub(super) selected_wallpaper: Option<WallpaperEntry>,
    pub(super) last_interaction: Instant,
    pub(super) trimmed_since_idle: bool,
    pub(super) toasts: ToastManager,
    pub(super) toast_rx: std::sync::mpsc::Receiver<ToastEvent>,
    pub(super) dark_mode: bool,
    pub(super) sidebar_collapsed: bool,
}

impl AuraApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = aura_storage::config_store::ConfigStore::default_path();
        let config_store = aura_storage::config_store::ConfigStore::new(&config_path);
        let dark_mode = config_store
            .load()
            .ok()
            .map(|c| c.appearance.dark_mode)
            .unwrap_or(false);

        if dark_mode {
            crate::theme::setup_dark_theme(&cc.egui_ctx);
        } else {
            crate::theme::setup_theme(&cc.egui_ctx);
        }

        egui_extras::install_image_loaders(&cc.egui_ctx);
        let (toast_tx, toast_rx) = std::sync::mpsc::channel();

        Self {
            gallery: GalleryPanel::new(),
            inspector: InspectorPanel::new(),
            settings: SettingsPanel::new(),
            status: StatusBar::new(),
            ipc_client: UiIpcClient::new(cc.egui_ctx.clone(), toast_tx),
            active_tab: Tab::Gallery,
            selected_wallpaper: None,
            last_interaction: Instant::now(),
            trimmed_since_idle: false,
            toasts: ToastManager::new(),
            toast_rx,
            dark_mode,
            sidebar_collapsed: false,
        }
    }

    pub fn dispatch_action(&mut self, action: crate::action::UiAction) {
        use crate::action::UiAction;
        use aura_ipc::protocol::Request;
        match action {
            UiAction::AssignWallpaper {
                monitor_id,
                wallpaper_id,
                fit_mode,
            } => {
                self.ipc_client.send(Request::AssignWallpaper {
                    monitor_id,
                    wallpaper_id,
                    fit_mode,
                });
            }
            UiAction::RemoveAssignment { monitor_id } => {
                self.ipc_client
                    .send(Request::RemoveAssignment { monitor_id });
            }
            UiAction::DeleteWallpaper { wallpaper_id } => {
                self.ipc_client
                    .send(Request::DeleteWallpaper { id: wallpaper_id });
            }
            UiAction::RefreshLibrary => {
                self.ipc_client.send(Request::RefreshLibrary);
            }
            UiAction::ImportFiles { paths } => {
                self.ipc_client.import_files(paths);
            }
            UiAction::SetWallpaperLibrary { path } => {
                self.ipc_client.send(Request::SetWallpaperLibrary { path });
            }
            UiAction::PauseAll => {
                self.ipc_client.send(Request::PauseAll);
            }
            UiAction::ResumeAll => {
                self.ipc_client.send(Request::ResumeAll);
            }
        }
    }
}

impl eframe::App for AuraApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let prev_tab = self.active_tab;

        if let Some(ref config) = self.ipc_client.config()
            && config.appearance.dark_mode != self.dark_mode
        {
            self.dark_mode = config.appearance.dark_mode;
            if self.dark_mode {
                crate::theme::setup_dark_theme(ui.ctx());
            } else {
                crate::theme::setup_theme(ui.ctx());
            }
        }

        if ui.input(|i| {
            i.pointer.any_click()
                || i.pointer.any_down()
                || i.pointer.primary_down()
                || i.smooth_scroll_delta != egui::Vec2::ZERO
        }) {
            self.last_interaction = Instant::now();
            self.trimmed_since_idle = false;
        }

        // --- Bottom: Status Bar ---
        self.show_status_bar(ui);

        // --- Main content area ---
        self.show_central_layout(ui);

        // --- Global keyboard shortcuts (skipped while a text field is focused) ---
        self.handle_shortcuts(ui);

        // --- Handle file drag-and-drop import ---
        self.handle_dropped_files(ui);

        if prev_tab != self.active_tab {
            self.selected_wallpaper = None;
        }

        if self.last_interaction.elapsed() > Duration::from_secs(3) && !self.trimmed_since_idle {
            trim_ui_working_set();
            self.trimmed_since_idle = true;
        }

        self.toasts.drain_events(&self.toast_rx);
        self.toasts.show(ui.ctx());
    }
}

impl Drop for AuraApp {
    fn drop(&mut self) {
        self.ipc_client.shutdown();
    }
}
