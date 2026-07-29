use std::path::{Path, PathBuf};

const MAX_SYMLINK_DEPTH: usize = 5;

#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("Path traversal detected: {0}")]
    Traversal(PathBuf),
    #[error("Path outside allowed directories: {0}")]
    OutsideAllowed(PathBuf),
    #[error("Symlink depth limit exceeded: {0}")]
    SymlinkDepthExceeded(PathBuf),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Validate a path is within allowed directories and resolve symlinks.
pub fn validate_path(path: &Path) -> Result<PathBuf, PathError> {
    let canonical = std::fs::canonicalize(path)?;

    if canonical
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(PathError::Traversal(canonical));
    }

    let allowed_dirs = get_allowed_directories();
    if !allowed_dirs.iter().any(|dir| canonical.starts_with(dir)) {
        return Err(PathError::OutsideAllowed(canonical));
    }

    Ok(canonical)
}

/// Get allowed directories for wallpaper library paths.
pub fn get_allowed_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(appdata) = dirs::config_dir() {
        dirs.push(appdata.join("Aura"));
    }

    if let Some(pictures) = dirs::picture_dir() {
        dirs.push(pictures);
    }

    if let Some(home) = dirs::home_dir() {
        dirs.push(home);
    }

    dirs
}

/// Check if a path is a symlink or junction.
pub fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// Validate a path is not a symlink.
pub fn validate_not_symlink(path: &Path) -> Result<(), PathError> {
    if is_symlink(path) {
        return Err(PathError::OutsideAllowed(path.to_path_buf()));
    }
    Ok(())
}

/// Check symlink depth during recursive scan.
pub fn check_symlink_depth(path: &Path, current_depth: usize) -> Result<(), PathError> {
    if is_symlink(path) && current_depth >= MAX_SYMLINK_DEPTH {
        return Err(PathError::SymlinkDepthExceeded(path.to_path_buf()));
    }
    Ok(())
}
