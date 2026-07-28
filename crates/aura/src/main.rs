#![allow(dead_code)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
fn main() {
    // Step 0: DPI awareness — must be before thread spawn or run_native()
    if let Err(e) = aura_platform_windows::enable_dpi_awareness() {
        eprintln!("DPI awareness failed: {e}");
    }

    // Step 1: init logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aura=info,wallpaperd=info,aura_platform_windows=info".into()),
        )
        .init();

    // Step 2: parse args
    let daemon_only = std::env::args().any(|a| a == "--daemon-only");

    if daemon_only {
        // No singleton check — headless daemon mode
        let opts = wallpaperd::daemon::DaemonOptions::standalone(None);
        if let Err(e) = wallpaperd::daemon::run(opts) {
            tracing::error!("daemon exited with error: {e}");
            std::process::exit(1);
        }
        return;
    }

    // Step 3: check singleton — if running, bring existing window to front
    if aura_platform_windows::singleton::ProcessSingleton::is_running() {
        bring_existing_window_to_front();
        std::process::exit(0);
    }

    // Step 4: acquire singleton on main thread (eliminates TOCTOU race)
    let singleton = match aura_platform_windows::singleton::ProcessSingleton::acquire() {
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

    let opts = wallpaperd::daemon::DaemonOptions {
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
            if let Err(e) = wallpaperd::daemon::run(opts) {
                tracing::error!("daemon thread exited with error: {e}");
            }
        })
        .expect("failed to spawn daemon thread");

    // Step 7: wait for IPC readiness (bounded timeout)
    match ready_rx.recv_timeout(std::time::Duration::from_secs(2)) {
        Ok(()) => tracing::info!("IPC server ready — starting UI"),
        Err(_) => tracing::warn!("IPC readiness timeout — UI reconnect handles delay"),
    }

    // Step 8: run eframe (blocks until window closed)
    wallpaper_ui::run();

    // Step 9: signal daemon to stop
    let _ = shutdown_tx.send(());

    // Step 10: wait for daemon shutdown with timeout
    match done_rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(()) => tracing::info!("daemon shutdown complete"),
        Err(_) => {
            tracing::warn!("daemon shutdown timed out — restoring wallpaper directly");
            aura_platform_windows::workerw::restore_desktop_wallpaper();
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
    use windows::core::w;

    unsafe {
        let Ok(hwnd) = FindWindowW(None, w!("Aura Wallpaper")) else {
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
