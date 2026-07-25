use aura_core::playback::{PerformanceProfile, PlaybackCommand, PlaybackState};

#[test]
fn playback_state_default_is_buffering() {
    assert_eq!(PlaybackState::default(), PlaybackState::Buffering);
}

#[test]
fn playback_state_eq() {
    assert_eq!(PlaybackState::Playing, PlaybackState::Playing);
    assert_ne!(PlaybackState::Playing, PlaybackState::Paused);
}

#[test]
fn playback_command_variants() {
    let _ = PlaybackCommand::Play;
    let _ = PlaybackCommand::Pause;
    let _ = PlaybackCommand::Loop;
    let _ = PlaybackCommand::Stop;
}

#[test]
fn performance_profile_default_is_balanced() {
    assert_eq!(PerformanceProfile::default(), PerformanceProfile::Balanced);
}

#[test]
fn performance_profile_eq() {
    assert_eq!(PerformanceProfile::Maximum, PerformanceProfile::Maximum);
    assert_ne!(PerformanceProfile::Maximum, PerformanceProfile::Balanced);
}

#[test]
fn serde_roundtrip_playback_state() {
    for state in &[
        PlaybackState::Playing,
        PlaybackState::Paused,
        PlaybackState::Buffering,
    ] {
        let json = serde_json::to_string(state).unwrap();
        let back: PlaybackState = serde_json::from_str(&json).unwrap();
        assert_eq!(*state, back);
    }
}

#[test]
fn serde_roundtrip_playback_command() {
    for cmd in &[
        PlaybackCommand::Play,
        PlaybackCommand::Pause,
        PlaybackCommand::Loop,
        PlaybackCommand::Stop,
    ] {
        let json = serde_json::to_string(cmd).unwrap();
        let back: PlaybackCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(*cmd, back);
    }
}

#[test]
fn serde_roundtrip_performance_profile() {
    for prof in &[
        PerformanceProfile::Maximum,
        PerformanceProfile::Balanced,
        PerformanceProfile::Paused,
    ] {
        let json = serde_json::to_string(prof).unwrap();
        let back: PerformanceProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(*prof, back);
    }
}
