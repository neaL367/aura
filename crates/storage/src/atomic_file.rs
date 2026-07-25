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

        // SAFETY: `from_wide` and `to_wide` are null-terminated wide string vectors derived via `to_wide`
        // that remain allocated and valid on the stack for the entire duration of the MoveFileExW Win32 FFI call.
        let mut res = unsafe {
            MoveFileExW(
                PCWSTR(from_wide.as_ptr()),
                PCWSTR(to_wide.as_ptr()),
                MOVEFILE_REPLACE_EXISTING,
            )
        };

        if res.is_err() {
            // Retry MoveFileExW once after a short 10ms delay for transient file locks (e.g., antivirus scan).
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
