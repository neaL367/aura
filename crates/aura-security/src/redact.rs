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
