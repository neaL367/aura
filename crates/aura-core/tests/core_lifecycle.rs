use aura_core::wallpaper::{WallpaperId, WallpaperState};

#[test]
fn test_wallpaper_id_uniqueness() {
    let id1 = WallpaperId::new();
    let id2 = WallpaperId::new();
    assert_ne!(id1, id2);
}

#[test]
fn test_wallpaper_state_transitions() {
    use WallpaperState::*;

    assert_eq!(Unloaded.transition(Loading).unwrap(), Loading);
    assert_eq!(Loading.transition(Ready).unwrap(), Ready);
    assert_eq!(Ready.transition(Rendering).unwrap(), Rendering);
    assert_eq!(Rendering.transition(Paused).unwrap(), Paused);
    assert_eq!(Paused.transition(Rendering).unwrap(), Rendering);
    assert_eq!(Rendering.transition(Unloaded).unwrap(), Unloaded);

    assert!(Unloaded.transition(Rendering).is_err());
    assert!(Ready.transition(Paused).is_err());
    assert!(Paused.transition(Loading).is_err());
}
