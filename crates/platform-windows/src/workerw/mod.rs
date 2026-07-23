pub mod attachment;
pub mod discovery;
pub mod manager;

pub use attachment::{attach_to_workerw, attach_topmost_bottom, restore_desktop_wallpaper};
pub use discovery::find_and_prepare_workerw;
pub use manager::WorkerWManager;
