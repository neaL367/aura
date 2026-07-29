pub mod discovery;
pub mod window;

use std::cell::Cell;

use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DispatchMessageW, GetMessageW, MSG, RegisterWindowMessageW,
            TranslateMessage, WINDOW_EX_STYLE, WS_CLIPCHILDREN, WS_POPUP,
        },
    },
    core::{Error, Result, w},
};

use window::{create_and_attach, register_classes};

const CLASS_CONTROL: windows::core::PCWSTR = w!("AuraProof_Control");
const CLASS_RENDER: windows::core::PCWSTR = w!("AuraProof_Render");

thread_local! {
    static TASKBAR_MSG_ID: Cell<u32> = const { Cell::new(0) };
    static RENDER_HWND_RAW: Cell<isize> = const { Cell::new(0) };
    static HINSTANCE_RAW: Cell<isize> = const { Cell::new(0) };
    static ATTACH_COUNT: Cell<u32> = const { Cell::new(0) };
}

#[inline]
pub fn render_hwnd() -> HWND {
    HWND(RENDER_HWND_RAW.with(Cell::get) as *mut _)
}

#[inline]
pub fn process_hinstance() -> HINSTANCE {
    HINSTANCE(HINSTANCE_RAW.with(Cell::get) as *mut _)
}

pub fn main() -> Result<()> {
    unsafe {
        let _ = windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }

    println!("╔══════════════════════════════════════════════════════╗");
    println!("║  Aura — WorkerW Desktop Integration Proof (Phase 0)  ║");
    println!("╚══════════════════════════════════════════════════════╝");
    println!();
    println!("Expected : A solid RED rectangle behind desktop icons.");
    println!("Recovery : Restart Explorer (Task Manager → restart) to");
    println!("           test automatic re-attachment.");
    println!("Exit     : Close this console window.\n");

    let hmodule = unsafe { GetModuleHandleW(None)? };
    let hinstance = HINSTANCE(hmodule.0);
    HINSTANCE_RAW.with(|c| c.set(hinstance.0 as isize));

    let taskbar_msg = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
    if taskbar_msg == 0 {
        return Err(Error::from_thread());
    }
    TASKBAR_MSG_ID.with(|c| c.set(taskbar_msg));
    println!("TaskbarCreated message ID : 0x{:04X}\n", taskbar_msg);

    register_classes(hinstance)?;

    let _control_hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS_CONTROL,
            w!("AuraProof_Control"),
            WS_POPUP | WS_CLIPCHILDREN,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(hinstance),
            None,
        )?
    };
    println!("Control window : {:?}", _control_hwnd.0);

    let render_hwnd = create_and_attach(hinstance)?;
    RENDER_HWND_RAW.with(|c| c.set(render_hwnd.0 as isize));

    println!("\nMessage loop running…");
    let mut msg = MSG::default();
    loop {
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        match r.0 {
            -1 => return Err(Error::from_thread()),
            0 => break,
            _ => unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }
    }

    println!(
        "\nExited. Total successful attachments: {}",
        ATTACH_COUNT.with(Cell::get)
    );
    Ok(())
}
