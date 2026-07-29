use aura_core::slideshow_state::SlideshowState;
use aura_storage::slideshow_store::SlideshowStore;

#[test]
fn load_missing_file_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let store = SlideshowStore::new(dir.path().join("does_not_exist.json"));
    let result = store.load().unwrap();
    assert!(result.is_none());
}

#[test]
fn save_and_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("slideshow_state.json");
    let store = SlideshowStore::new(&path);

    let mut state = SlideshowState::new();
    state.last_cycle = 7;
    store.save(&state).unwrap();

    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.last_cycle, 7);
    assert!(loaded.queue.is_empty());
}

#[test]
fn reset_clears_and_creates_fresh() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("slideshow_state.json");
    let store = SlideshowStore::new(&path);

    let mut state = SlideshowState::new();
    state.last_cycle = 42;
    store.save(&state).unwrap();

    store.reset().unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.last_cycle, 0);
    assert!(loaded.queue.is_empty());
}
