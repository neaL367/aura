use aura_core::wallpaper::{WallpaperId, WallpaperState};

#[test]
fn valid_transitions() {
    use WallpaperState::*;
    assert_eq!(Unloaded.transition(Loading).unwrap(), Loading);
    assert_eq!(Loading.transition(Ready).unwrap(), Ready);
    assert_eq!(Ready.transition(Rendering).unwrap(), Rendering);
    assert_eq!(Rendering.transition(Paused).unwrap(), Paused);
    assert_eq!(Paused.transition(Rendering).unwrap(), Rendering);
    assert_eq!(Rendering.transition(Unloaded).unwrap(), Unloaded);
}

#[test]
fn invalid_transitions() {
    use WallpaperState::*;
    assert!(Unloaded.transition(Rendering).is_err());
    assert!(Ready.transition(Paused).is_err());
    assert!(Paused.transition(Loading).is_err());
}

#[test]
fn wallpaper_id_is_unique() {
    let a = WallpaperId::new();
    let b = WallpaperId::new();
    assert_ne!(a, b);
}

#[test]
fn wallpaper_id_from_path_is_deterministic() {
    let path_a = std::path::Path::new("C:/Wallpapers/bg1.png");
    let path_b = std::path::Path::new("C:/Wallpapers/bg2.png");
    let id_a1 = WallpaperId::from_path(path_a);
    let id_a2 = WallpaperId::from_path(path_a);
    let id_b = WallpaperId::from_path(path_b);
    assert_eq!(id_a1, id_a2);
    assert_ne!(id_a1, id_b);
}
