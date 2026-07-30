use std::path::PathBuf;

use aura_ipc::protocol::Request;

/// Delegate file import to the hardened daemon orchestrator via IPC.
pub fn import_files_to_library(paths: Vec<PathBuf>, send_fn: impl FnOnce(Request)) {
    if !paths.is_empty() {
        send_fn(Request::ImportFiles { paths });
    }
}
