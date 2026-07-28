use aura_core::slideshow_state::SlideshowState;
use aura_core::wallpaper::WallpaperId;

#[test]
fn roundtrip_serialize_deserialize() {
    let mut state = SlideshowState::new();
    state.queue = vec![
        WallpaperId::from_path(std::path::Path::new("C:/wp1.png")),
        WallpaperId::from_path(std::path::Path::new("C:/wp2.png")),
    ];
    state.index = 1;
    state.last_cycle = 42;
    let json = serde_json::to_string(&state).unwrap();
    let restored: SlideshowState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.queue.len(), 2);
    assert_eq!(restored.index, 1);
    assert_eq!(restored.last_cycle, 42);
}

#[test]
fn empty_state_defaults() {
    let state = SlideshowState::new();
    assert!(state.queue.is_empty());
    assert_eq!(state.index, 0);
    assert_eq!(state.last_cycle, 0);
    assert!(state.last_wallpapers.is_empty());
}

#[test]
fn reset_clears_everything() {
    let mut state = SlideshowState::new();
    state.queue.push(WallpaperId::new());
    state.index = 5;
    state.last_cycle = 99;
    state = SlideshowState::reset();
    assert!(state.queue.is_empty());
    assert_eq!(state.index, 0);
    assert_eq!(state.last_cycle, 0);
}
