use aura_storage::config_store::ConfigStore;
use aura_storage::library_store::LibraryStore;
use tempfile::tempdir;

#[test]
fn config_store_corrupt_toml_returns_error() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("aura.toml");
    std::fs::write(&path, b"this is not valid toml = {{").unwrap();

    let store = ConfigStore::new(&path);
    let result = store.load();
    assert!(result.is_err(), "corrupt TOML should produce an error");
}

#[test]
fn library_store_corrupt_json_returns_empty() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("library.json");
    std::fs::write(&path, b"not valid json at all {{{").unwrap();

    let store = LibraryStore::new(&path);
    let loaded = store.load().unwrap();
    assert!(
        loaded.is_empty(),
        "corrupt JSON should produce empty library"
    );
}

#[test]
fn library_store_missing_parent_dir_creates_it() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("subdir").join("library.json");
    let store = LibraryStore::new(&nested);
    let entries: Vec<aura_core::wallpaper::WallpaperMeta> = vec![];

    // Save should create the parent directory and succeed.
    store.save(&entries).unwrap();
    assert!(
        nested.exists(),
        "save should create parent dirs and write file"
    );
}

#[test]
fn scanner_empty_directory_produces_empty_results() {
    let dir = tempdir().unwrap();
    let scanned = aura_storage::LibraryScanner::scan_paths(&[dir.path().to_path_buf()]);
    assert!(
        scanned.is_empty(),
        "empty directory should produce no results"
    );
}

#[test]
fn scanner_unknown_extension_skipped() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("document.xyz");
    std::fs::write(&file_path, b"random data").unwrap();

    let scanned = aura_storage::LibraryScanner::scan_paths(&[dir.path().to_path_buf()]);
    assert!(scanned.is_empty(), ".xyz files should be skipped");
}
