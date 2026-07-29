use aura_win::is_fullscreen_app_active;

#[test]
fn test_is_fullscreen_app_active_does_not_panic() {
    let _active = is_fullscreen_app_active();
}
