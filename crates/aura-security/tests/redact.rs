use std::path::PathBuf;

use aura_security::redact::redact_path;

#[test]
fn redact_replaces_username() {
    if let Ok(username) = std::env::var("USERNAME") {
        let path = PathBuf::from(format!("C:\\Users\\{username}\\Documents\\file.txt"));
        let result = redact_path(&path);
        assert!(
            result.contains("[USER]"),
            "Expected [USER] in redacted path, got: {result}",
        );
        assert!(!result.contains(&username), "Username should be redacted");
    }
}

#[test]
fn redact_no_username_env() {
    let path = PathBuf::from("C:\\Users\\someone\\Documents\\file.txt");
    let result = redact_path(&path);
    assert!(result.contains("someone") || result.contains("[USER]"));
}
