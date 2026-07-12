use std::path::PathBuf;
use aura::utils::logging;
use aura::storage::json_store::JsonConfigStore;
use aura::domain::traits::ConfigStore;
use tracing::{info, error, warn};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize logging
    logging::init();
    info!("Aura Wallpaper Engine starting...");

    unsafe {
        aura::utils::hresult::check(windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
        ))?;
    }

    // 2. Optional --pick flag to test file dialog and WIC thumbnail generation interactively
    if std::env::args().any(|arg| arg == "--pick") {
        info!("Running interactive --pick verification...");
        match aura::platform::windows::file_dialog::pick_wallpaper_file(windows::Win32::Foundation::HWND::default()) {
            Ok(Some(path)) => {
                info!("Selected path: {:?}", path);
                let appdata = std::env::var("APPDATA")?;
                let thumb_dir = PathBuf::from(appdata).join("Aura").join("thumbnails");
                let hash = aura::library::thumbnail::get_metadata_hash(&path);
                let id = aura::library::model::derive_id(&path);
                let dest = thumb_dir.join(format!("{}_{}.png", id, hash));
                info!("Generating lossless PNG thumbnail at: {:?}", dest);
                if let Err(e) = aura::library::thumbnail::generate_thumbnail(&path, &dest, 256) {
                    error!("Failed to generate thumbnail: {:?}", e);
                } else {
                    info!("Thumbnail generated successfully!");
                }
            }
            Ok(None) => {
                info!("Dialog was cancelled by the user.");
            }
            Err(e) => {
                error!("File dialog error: {:?}", e);
            }
        }
        unsafe { windows::Win32::System::Com::CoUninitialize(); }
        return Ok(());
    }

    // 3. Load or create default configuration in %APPDATA%/Aura/config.json
    let appdata = std::env::var("APPDATA")?;
    let config_dir = PathBuf::from(appdata).join("Aura");
    let config_path = config_dir.join("config.json");
    let store = JsonConfigStore::new(config_path);

    let mut config = match store.load() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load configuration: {:?}", e);
            unsafe { windows::Win32::System::Com::CoUninitialize(); }
            return Err(e.into());
        }
    };

    // If configuration has no monitors defined, add a default primary monitor configuration
    if config.monitors.is_empty() {
        info!("No monitor settings found. Creating default configuration with primary monitor...");
        let default_wallpaper = PathBuf::from(r"C:\Windows\Web\Wallpaper\Windows\img0.jpg");
        let mut wallpaper_id = None;
        if let Ok((entry, _)) = aura::library::scanner::add_entry(&mut config, default_wallpaper) {
            wallpaper_id = Some(entry.id);
        }
        config.monitors.push(aura::config::model::MonitorConfig {
            monitor_id: "primary".to_string(),
            wallpaper_id,
            fit_mode: aura::domain::fit_mode::FitMode::Fill,
            ..Default::default()
        });
        if let Err(e) = store.save(&config) {
            warn!("Failed to save default config: {:?}", e);
        }
    }

    // 4. Initialize Composition Root (wires up Win32, D3D11, and rendering for all monitors)
    let mut composition = match aura::app::composition_root::CompositionRoot::new(&config) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to initialize composition root: {:?}", e);
            unsafe { windows::Win32::System::Com::CoUninitialize(); }
            return Err(e.into());
        }
    };

    // 5. Enter Win32 Message Loop
    info!("Entering Win32 message loop...");
    unsafe {
        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        while windows::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_USER + 100 {
                info!("Main loop: Syncing monitors after display change event...");
                if let Err(e) = composition.sync_monitors(&config) {
                    error!("Failed to sync monitors: {:?}", e);
                }
                // Drain any duplicate sync messages posted concurrently by multiple windows
                let mut dummy = windows::Win32::UI::WindowsAndMessaging::MSG::default();
                while windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
                    &mut dummy,
                    None,
                    windows::Win32::UI::WindowsAndMessaging::WM_USER + 100,
                    windows::Win32::UI::WindowsAndMessaging::WM_USER + 100,
                    windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
                ).as_bool() {}
            }
            let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
        }
    }

    info!("Aura Wallpaper Engine shutting down...");
    unsafe {
        windows::Win32::System::Com::CoUninitialize();
    }

    Ok(())
}
