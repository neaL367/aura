pub(crate) fn copy_file_robust(
    src: &std::path::Path,
    dest: &std::path::Path,
) -> std::io::Result<()> {
    if dest.exists() {
        // Clear stale leftover file: remove it entirely so we don't inherit
        // restrictive ACLs, lock state, or a readonly attribute from a
        // previous failed import.
        if let Ok(meta) = std::fs::metadata(dest) {
            let mut perms = meta.permissions();
            if perms.readonly() {
                #[allow(clippy::permissions_set_readonly_false)]
                perms.set_readonly(false);
                let _ = std::fs::set_permissions(dest, perms);
            }
        }
        let _ = std::fs::remove_file(dest);
    }

    let mut last_err = None::<std::io::Error>;

    #[cfg(target_os = "windows")]
    {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;

        for attempt in 0..3 {
            let result = (|| -> std::io::Result<()> {
                let mut src_file = OpenOptions::new().read(true).share_mode(7).open(src)?;
                let mut dest_file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(dest)?;
                std::io::copy(&mut src_file, &mut dest_file)?;
                Ok(())
            })();

            match result {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_err = Some(e);
                    std::thread::sleep(std::time::Duration::from_millis(50 * (attempt + 1)));
                }
            }
        }
    }

    for attempt in 0..3 {
        match std::fs::copy(src, dest) {
            Ok(_) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(50 * (attempt + 1)));
            }
        }
    }

    Err(last_err.unwrap_or_else(|| std::io::Error::other("Copy failed")))
}
