use std::path::PathBuf;
use std::ffi::c_void;
use windows::core::{w, HRESULT, Interface};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{
    IFileOpenDialog, IFileDialog, FileOpenDialog, SIGDN_FILESYSPATH, IShellItem,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::System::Com::{
    CoCreateInstance, CLSCTX_INPROC_SERVER, CoTaskMemFree,
};
use windows::Win32::Foundation::ERROR_CANCELLED;
use crate::utils::error::{AppError, Result};

/// Prompts the user to select a wallpaper file using the native Windows Common Item Dialog (IFileOpenDialog).
/// Filters files by supported image and video formats.
/// Returns `Ok(None)` if the user cancelled the dialog.
pub fn pick_wallpaper_file(owner_hwnd: HWND) -> Result<Option<PathBuf>> {
    // Expected to run on a thread already initialized with CoInitializeEx (such as the main thread).
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(
            &FileOpenDialog,
            None,
            CLSCTX_INPROC_SERVER,
        )?;

        // Set filters: "All Supported" listed first so it's the default option
        let filters = [
            COMDLG_FILTERSPEC {
                pszName: w!("All Supported Wallpapers"),
                pszSpec: w!("*.png;*.jpg;*.jpeg;*.webp;*.bmp;*.mp4;*.webm;*.mov"),
            },
            COMDLG_FILTERSPEC {
                pszName: w!("Images (*.png; *.jpg; *.jpeg; *.webp; *.bmp)"),
                pszSpec: w!("*.png;*.jpg;*.jpeg;*.webp;*.bmp"),
            },
            COMDLG_FILTERSPEC {
                pszName: w!("Videos (*.mp4; *.webm; *.mov)"),
                pszSpec: w!("*.mp4;*.webm;*.mov"),
            },
        ];

        let file_dialog: IFileDialog = dialog.cast()?;
        file_dialog.SetFileTypes(&filters)
            .map_err(|e| AppError::Platform(format!("SetFileTypes failed: {}", e)))?;
        file_dialog.SetTitle(w!("Select Wallpaper"))
            .map_err(|e| AppError::Platform(format!("SetTitle failed: {}", e)))?;

        // Show the dialog modally
        let show_res = dialog.Show(Some(owner_hwnd));
        if let Err(e) = show_res {
            let cancel_hresult = HRESULT::from_win32(ERROR_CANCELLED.0);
            if e.code() == cancel_hresult {
                return Ok(None);
            }
            return Err(AppError::Platform(format!("IFileOpenDialog::Show failed: {}", e)));
        }

        let item: IShellItem = dialog.GetResult()
            .map_err(|e| AppError::Platform(format!("GetResult failed: {}", e)))?;
        let path_pwstr = item.GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|e| AppError::Platform(format!("GetDisplayName failed: {}", e)))?;

        let path_str = path_pwstr.to_string().map_err(|e| {
            AppError::Platform(format!("PWSTR conversion failed: {}", e))
        })?;

        // CoTaskMemFree on the wide string pointer allocated by SIGDN_FILESYSPATH to prevent memory leaks.
        CoTaskMemFree(Some(path_pwstr.0 as *const c_void));

        Ok(Some(PathBuf::from(path_str)))
    }
}
