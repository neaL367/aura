use aura_daemon::daemon::effective_paused_state;

#[test]
fn manual_pause_wins_over_game_mode_resume() {
    // User manually paused + game mode active -> paused.
    assert!(effective_paused_state(true, true));
    // User manually paused + game mode released -> still paused.
    assert!(effective_paused_state(true, false));
}

#[test]
fn game_mode_is_additional_pause_source() {
    // No manual pause + game mode active -> paused.
    assert!(effective_paused_state(false, true));
    // Nothing active -> running.
    assert!(!effective_paused_state(false, false));
}
