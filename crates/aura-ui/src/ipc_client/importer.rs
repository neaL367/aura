use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::toast::{ToastEvent, ToastKind};

pub fn import_files_to_library(
    paths: Vec<PathBuf>,
    library_path: Option<PathBuf>,
    toast_tx: &std::sync::mpsc::Sender<ToastEvent>,
    last_error: &Arc<Mutex<Option<String>>>,
    on_complete: impl FnOnce(),
) {
    let lib_path = match library_path {
        Some(p) => p,
        None => {
            let msg = "Library path not configured. Set a library folder first.";
            let _ = toast_tx.send((msg.into(), ToastKind::Error));
            *last_error.lock().unwrap_or_else(|e| e.into_inner()) = Some(msg.into());
            return;
        }
    };
    if !lib_path.is_dir() && std::fs::create_dir_all(&lib_path).is_err() {
        let msg = format!("Failed to create library directory: {}", lib_path.display());
        let _ = toast_tx.send((msg.clone(), ToastKind::Error));
        *last_error.lock().unwrap_or_else(|e| e.into_inner()) = Some(msg);
        return;
    }
    let mut copied = 0u32;
    let mut errors: Vec<String> = Vec::new();
    for src in &paths {
        let name = match src.file_name() {
            Some(n) => n,
            None => continue,
        };
        let dest = lib_path.join(name);
        if dest.exists() {
            continue;
        }
        match std::fs::copy(src, &dest) {
            Ok(_) => copied += 1,
            Err(e) => {
                errors.push(format!("{}: {}", src.display(), e));
            }
        }
    }
    if copied == 0 {
        let msg = if errors.is_empty() {
            "All selected files already exist in the library.".into()
        } else {
            format!("Could not copy files: {}", errors.join("; "))
        };
        *last_error.lock().unwrap_or_else(|e| e.into_inner()) = Some(msg.clone());
        let kind = if errors.is_empty() {
            ToastKind::Info
        } else {
            ToastKind::Error
        };
        let _ = toast_tx.send((msg, kind));
        return;
    }
    if !errors.is_empty() {
        tracing::warn!(
            "import_files: copied {copied} file(s), {} error(s): {:?}",
            errors.len(),
            errors
        );
        let _ = toast_tx.send((
            format!("Imported {copied} file(s), {} error(s)", errors.len()),
            ToastKind::Warning,
        ));
    } else {
        let _ = toast_tx.send((format!("Imported {copied} file(s)"), ToastKind::Success));
    }
    *last_error.lock().unwrap_or_else(|e| e.into_inner()) = None;
    on_complete();
}
