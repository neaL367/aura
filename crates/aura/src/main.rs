#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod tray;

#[cfg(target_os = "windows")]
fn main() {
    // Step 0: DPI awareness — must be before thread spawn or run_native()
    if let Err(e) = aura_win::enable_dpi_awareness() {
        eprintln!("DPI awareness failed: {e}");
    }

    // Step 1: init logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aura=info,aura_daemon=info,aura_win=info".into()),
        )
        .init();

    // Step 2: parse args
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("aura {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let daemon_only = args.iter().any(|a| a == "--daemon-only");

    if daemon_only {
        // No singleton check — headless daemon mode
        let opts = aura_daemon::daemon::DaemonOptions::standalone(None);
        if let Err(e) = aura_daemon::daemon::run(opts) {
            tracing::error!("daemon exited with error: {e}");
            std::process::exit(1);
        }
        return;
    }

    // Step 3: check singleton — if running, bring existing window to front
    if aura_win::singleton::ProcessSingleton::is_running() {
        bring_existing_window_to_front();
        std::process::exit(0);
    }

    // Step 4: acquire singleton on main thread (eliminates TOCTOU race)
    let singleton = match aura_win::singleton::ProcessSingleton::acquire() {
        Ok(s) => s,
        Err(_) => {
            // Race lost between is_running() and acquire()
            bring_existing_window_to_front();
            std::process::exit(0);
        }
    };

    // Step 5: create shutdown/ready/done channels
    let (shutdown_tx, shutdown_rx) = crossbeam_channel::bounded::<()>(1);
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<()>(1);
    let (done_tx, done_rx) = crossbeam_channel::bounded::<()>(1);

    let opts = aura_daemon::daemon::DaemonOptions {
        wallpaper_path: None,
        shutdown_rx,
        ready_tx,
        done_tx,
        _singleton: singleton,
    };

    // Step 6: spawn daemon on background thread
    std::thread::Builder::new()
        .name("daemon".into())
        .spawn(move || {
            if let Err(e) = aura_daemon::daemon::run(opts) {
                tracing::error!("daemon thread exited with error: {e}");
            }
        })
        .expect("failed to spawn daemon thread");

    // Step 7: wait for IPC readiness (bounded timeout)
    match ready_rx.recv_timeout(std::time::Duration::from_secs(2)) {
        Ok(()) => tracing::info!("IPC server ready — starting UI"),
        Err(_) => tracing::warn!("IPC readiness timeout — UI reconnect handles delay"),
    }

    // Step 8: spawn tray icon (background message loop)
    let (tray_quit_tx, tray_quit_rx) = crossbeam_channel::bounded::<()>(1);
    let _tray_handle = tray::TrayManager::new().spawn(tray_quit_tx);

    // Step 8b: quit-fallback listener — if FindWindowW fails inside the tray
    // thread (e.g. startup race), this retries with backoff and falls back
    // to process::exit as a last resort.
    let _quit_fallback = {
        use std::time::Duration;
        std::thread::Builder::new()
            .name("quit-fallback".into())
            .spawn(move || {
                if tray_quit_rx.recv().is_err() {
                    return;
                }
                let title = windows::core::HSTRING::from(aura_core::WINDOW_TITLE);
                for retry in 0u32..10 {
                    let hwnd = unsafe {
                        windows::Win32::UI::WindowsAndMessaging::FindWindowW(None, &title)
                    };
                    let Ok(hwnd) = hwnd else {
                        std::thread::sleep(Duration::from_millis(100));
                        tracing::warn!("quit-fallback: retry {} FindWindowW failed", retry + 1);
                        continue;
                    };
                    if !hwnd.is_invalid() {
                        unsafe {
                            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                                Some(hwnd),
                                windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                                windows::Win32::Foundation::WPARAM(0),
                                windows::Win32::Foundation::LPARAM(0),
                            );
                        }
                        return;
                    }
                }
                tracing::error!(
                    "quit-fallback: exhausted retries — restoring wallpaper then force-exiting"
                );
                aura_win::workerw::restore_desktop_wallpaper();
                std::process::exit(0);
            })
            .expect("failed to spawn quit-fallback thread")
    };

    // Step 9: run eframe (blocks until window closed)
    aura_ui::run();

    // Step 10: signal daemon to stop
    let _ = shutdown_tx.send(());

    // Step 11: wait for daemon shutdown with timeout
    match done_rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(()) => tracing::info!("daemon shutdown complete"),
        Err(_) => {
            tracing::warn!("daemon shutdown timed out — restoring wallpaper directly");
            aura_win::workerw::restore_desktop_wallpaper();
        }
    }
}

#[cfg(target_os = "windows")]
fn bring_existing_window_to_front() {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetWindowThreadProcessId, SW_RESTORE, SetForegroundWindow, ShowWindow,
    };
    unsafe {
        let title = windows::core::HSTRING::from(aura_core::WINDOW_TITLE);
        let Ok(hwnd) = FindWindowW(None, &title) else {
            return;
        };
        if hwnd == HWND(std::ptr::null_mut()) {
            return;
        }
        let foreground_thread = GetWindowThreadProcessId(hwnd, None);
        let current_thread = GetCurrentThreadId();
        if foreground_thread != current_thread {
            let _ = AttachThreadInput(current_thread, foreground_thread, true);
            let _ = SetForegroundWindow(hwnd);
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = AttachThreadInput(current_thread, foreground_thread, false);
        } else {
            let _ = SetForegroundWindow(hwnd);
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("aura is only supported on Windows");
    std::process::exit(1);
}

#[cfg(not(target_os = "windows"))]
fn bring_existing_window_to_front() {}
