use std::mem;

use windows::{
    Win32::{
        Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{BeginPaint, CreateSolidBrush, EndPaint, FillRect, HBRUSH, PAINTSTRUCT},
        UI::WindowsAndMessaging::{
            CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, GetClientRect, IDC_ARROW,
            LoadCursorW, PostQuitMessage, RegisterClassExW, WINDOW_EX_STYLE, WM_DESTROY,
            WM_DISPLAYCHANGE, WM_PAINT, WNDCLASSEXW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_POPUP,
        },
    },
    core::{Error, Result, w},
};

use super::discovery::ensure_attached;
use super::{
    ATTACH_COUNT, CLASS_CONTROL, CLASS_RENDER, RENDER_HWND_RAW, TASKBAR_MSG_ID, process_hinstance,
    render_hwnd,
};

pub fn register_classes(hinstance: HINSTANCE) -> Result<()> {
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW)? };

    let control_wc = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(control_wnd_proc),
        hInstance: hinstance,
        hCursor: cursor,
        lpszClassName: CLASS_CONTROL,
        ..Default::default()
    };
    if unsafe { RegisterClassExW(&control_wc) } == 0 {
        return Err(Error::from_thread());
    }

    let brush: HBRUSH = unsafe { CreateSolidBrush(COLORREF(0x0000_00FF)) };
    if brush.is_invalid() {
        return Err(Error::from_thread());
    }

    let render_wc = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(render_wnd_proc),
        hInstance: hinstance,
        hCursor: cursor,
        hbrBackground: brush,
        lpszClassName: CLASS_RENDER,
        ..Default::default()
    };
    if unsafe { RegisterClassExW(&render_wc) } == 0 {
        return Err(Error::from_thread());
    }

    Ok(())
}

pub fn create_and_attach(hinstance: HINSTANCE) -> Result<HWND> {
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS_RENDER,
            w!("AuraProof_Render"),
            WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0,
            0,
            800,
            600,
            None,
            None,
            Some(hinstance),
            None,
        )?
    };
    println!("[+] Render window created : {:?}", hwnd.0);

    match ensure_attached(hwnd) {
        Ok(workerw) => {
            let n = ATTACH_COUNT.with(|c| {
                let v = c.get() + 1;
                c.set(v);
                v
            });
            println!(
                "[✓] Attached render {:?} → WorkerW {:?} (attach #{})",
                hwnd.0, workerw.0, n
            );
        }
        Err(e) => {
            eprintln!("[✗] Attachment failed: {}", e);
        }
    }

    Ok(hwnd)
}

unsafe extern "system" fn control_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let taskbar_id = TASKBAR_MSG_ID.with(std::cell::Cell::get);

    if msg == taskbar_id {
        println!("\n[!!] TaskbarCreated — Explorer restarted. Recreating render window…");
        let hinstance = process_hinstance();
        match create_and_attach(hinstance) {
            Ok(new_hwnd) => {
                RENDER_HWND_RAW.with(|c| c.set(new_hwnd.0 as isize));
                println!("[✓] Recovery complete.\n");
            }
            Err(e) => {
                eprintln!("[✗] Recovery failed: {}\n", e);
            }
        }
        return LRESULT(0);
    }

    match msg {
        WM_DISPLAYCHANGE => {
            println!("\n[!!] WM_DISPLAYCHANGE — repositioning render window…");
            let rh = render_hwnd();
            if !rh.0.is_null() {
                match ensure_attached(rh) {
                    Ok(_) => println!("[✓] Repositioned.\n"),
                    Err(e) => eprintln!("[✗] Reposition failed: {}\n", e),
                }
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

unsafe extern "system" fn render_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            if !hdc.is_invalid() {
                let mut rect = RECT::default();
                unsafe {
                    let _ = GetClientRect(hwnd, &mut rect);

                    let brush = CreateSolidBrush(COLORREF(0x0000_00FF));
                    if !brush.is_invalid() {
                        FillRect(hdc, &rect, brush);
                    }

                    let _ = EndPaint(hwnd, &ps);
                }
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            println!("[!] Render window destroyed ({:?}).", hwnd.0);
            LRESULT(0)
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
