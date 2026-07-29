use aura_platform_windows::is_fullscreen_app_active;

#[test]
fn test_is_fullscreen_app_active_does_not_panic() {
    let _active = is_fullscreen_app_active();
}
