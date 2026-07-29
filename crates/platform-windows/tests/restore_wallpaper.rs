#[cfg(target_os = "windows")]
#[test]
fn test_restore_desktop_wallpaper_does_not_panic() {
    // Calling restore_desktop_wallpaper on shutdown must not panic and must safely read current wallpaper path.
    aura_platform_windows::workerw::restore_desktop_wallpaper();
}
