use crate::error::StorageError;
use std::path::Path;

/// Write content to a target file atomically.
///
/// On Windows, if the destination file already exists, standard `rename` will fail
/// with `ERROR_ALREADY_EXISTS`. Using `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`
/// replaces the file atomically in a single system call without an un-safe `remove_file` deletion gap.
pub fn atomic_save_file(path: &Path, content: &str) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, content)?;

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

        let res = unsafe {
            MoveFileExW(
                PCWSTR(from_wide.as_ptr()),
                PCWSTR(to_wide.as_ptr()),
                MOVEFILE_REPLACE_EXISTING,
            )
        };

        if res.is_err() {
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp_path, path)?;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::fs::remove_file(path);
        std::fs::rename(&tmp_path, path)?;
    }

    Ok(())
}
