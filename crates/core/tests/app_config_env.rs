use aura_core::config::AppConfig;

#[test]
fn app_config_default_with_userprofile() {
    let dir = std::env::temp_dir().join("aura-test-userprofile");
    let _ = std::fs::create_dir_all(&dir);
    let pics = dir.join("Pictures");
    std::fs::create_dir_all(&pics).unwrap();

    let prev = std::env::var("USERPROFILE").ok();
    // SAFETY: env var manipulation in a single-threaded test.
    unsafe {
        std::env::set_var("USERPROFILE", &dir);
    }

    let cfg = AppConfig::default();
    // Should point to the Pictures dir from our custom USERPROFILE.
    assert!(
        cfg.library.library_path.starts_with(&pics),
        "default library_path should be under USERPROFILE/Pictures, got {}",
        cfg.library.library_path.display()
    );

    // Restore original.
    // SAFETY: restoring env var in single-threaded test.
    unsafe {
        if let Some(p) = prev {
            std::env::set_var("USERPROFILE", p);
        } else {
            std::env::remove_var("USERPROFILE");
        }
    }

    std::fs::remove_dir_all(&dir).unwrap_or(());
}
