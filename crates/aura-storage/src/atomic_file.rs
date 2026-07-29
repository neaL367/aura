use crate::error::StorageError;
use std::path::Path;

/// Write string content to a target file atomically.
pub fn atomic_save_file(path: &Path, content: &str) -> Result<(), StorageError> {
    atomic_save_bytes(path, content.as_bytes())
}

/// Write raw byte slice to a target file atomically.
///
/// On Windows, if the destination file already exists, standard `rename` will fail
/// with `ERROR_ALREADY_EXISTS`. Using `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`
/// replaces the file atomically in a single system call without an un-safe `remove_file` deletion gap.
pub fn atomic_save_bytes(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp_path = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    std::fs::write(&tmp_path, bytes)?;

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MoveFileExW};
        use windows::core::PCWSTR;

        fn to_wide(p: &Path) -> Vec<u16> {
            use std::os::windows::ffi::OsStrExt;
            p.as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        }

        let from_wide = to_wide(&tmp_path);
        let to_wide = to_wide(path);

        let mut res = unsafe {
            MoveFileExW(
                PCWSTR(from_wide.as_ptr()),
                PCWSTR(to_wide.as_ptr()),
                MOVEFILE_REPLACE_EXISTING,
            )
        };

        if res.is_err() {
            std::thread::sleep(std::time::Duration::from_millis(10));
            res = unsafe {
                MoveFileExW(
                    PCWSTR(from_wide.as_ptr()),
                    PCWSTR(to_wide.as_ptr()),
                    MOVEFILE_REPLACE_EXISTING,
                )
            };
        }

        if let Err(e) = res {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(StorageError::Io(std::io::Error::from_raw_os_error(
                e.code().0,
            )));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Err(e) = std::fs::rename(&tmp_path, path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(StorageError::Io(e));
        }
    }

    Ok(())
}

/// Clean up stale temporary files older than the given duration.
pub fn cleanup_stale_temp_files(
    dir: &Path,
    max_age: std::time::Duration,
) -> Result<usize, std::io::Error> {
    let mut cleaned = 0;
    let now = std::time::SystemTime::now();

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        let is_temp = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.starts_with("tmp-"))
            .unwrap_or(false);

        if is_temp
            && let Ok(metadata) = std::fs::metadata(&path)
            && let Ok(modified) = metadata.modified()
            && now.duration_since(modified).unwrap_or_default() > max_age
        {
            let _ = std::fs::remove_file(&path);
            cleaned += 1;
        }
    }

    Ok(cleaned)
}
