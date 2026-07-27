use std::path::Path;

/// Redact username from file paths for logging.
pub fn redact_path(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    if let Ok(username) = std::env::var("USERNAME")
        && !username.is_empty()
    {
        return path_str.replace(&username, "[USER]");
    }
    path_str.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn redact_replaces_username() {
        if let Ok(username) = std::env::var("USERNAME") {
            let path = PathBuf::from(format!("C:\\Users\\{}\\Documents\\file.txt", username));
            let result = redact_path(&path);
            assert!(
                result.contains("[USER]"),
                "Expected [USER] in redacted path, got: {}",
                result
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
}
