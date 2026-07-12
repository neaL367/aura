use std::sync::Mutex;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_RBUTTONDOWN, WM_RBUTTONUP,
};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext};

use crate::renderer::swapchain::Swapchain;
use crate::utils::error::{AppError, Result};

static PENDING_EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());
static POINTER_POS: Mutex<Pos2> = Mutex::new(Pos2::ZERO);

pub struct EguiBackend {
    ctx: egui::Context,
    painter: EguiD3D11Painter,
}

impl EguiBackend {
    pub fn new(hwnd: HWND, device: &ID3D11Device) -> Result<Self> {
        let ctx = egui::Context::default();
        let painter = EguiD3D11Painter::new(hwnd, device)?;
        Ok(Self { ctx, painter })
    }

    pub fn frame(
        &mut self,
        screen_rect: Rect,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        mut run_ui: impl FnMut(&egui::Context),
    ) -> Result<()> {
        let raw_input = self.take_raw_input(screen_rect);
        let output = self.ctx.run(raw_input, |ctx| run_ui(ctx));
        self.painter.paint(&self.ctx, output, device, context)?;
        Ok(())
    }

    fn take_raw_input(&self, screen_rect: Rect) -> RawInput {
        let events = std::mem::take(&mut *PENDING_EVENTS.lock().unwrap());
        RawInput {
            screen_rect: Some(screen_rect),
            events,
            ..Default::default()
        }
    }

    pub fn wants_repaint(&self) -> bool {
        self.ctx.has_requested_repaint()
    }

    pub fn resize(&mut self, device: &ID3D11Device, width: u32, height: u32) -> Result<()> {
        self.painter.swapchain.resize(device, width, height)
    }
}

pub fn forward_input_message(_hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) {
    let mut events = PENDING_EVENTS.lock().unwrap();

    match msg {
        WM_MOUSEMOVE => {
            let (x, y) = xy_from_lparam(lparam);
            let pos = Pos2::new(x as f32, y as f32);
            *POINTER_POS.lock().unwrap() = pos;
            events.push(Event::PointerMoved(pos));
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP => {
            let pos = *POINTER_POS.lock().unwrap();
            events.push(Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: msg == WM_LBUTTONDOWN,
                modifiers: current_modifiers(),
            });
        }
        WM_RBUTTONDOWN | WM_RBUTTONUP => {
            let pos = *POINTER_POS.lock().unwrap();
            events.push(Event::PointerButton {
                pos,
                button: PointerButton::Secondary,
                pressed: msg == WM_RBUTTONDOWN,
                modifiers: current_modifiers(),
            });
        }
        WM_MOUSEWHEEL => {
            let raw = ((wparam.0 >> 16) & 0xFFFF) as i16;
            let delta = raw as f32 / 120.0;
            events.push(Event::MouseWheel {
                delta: egui::vec2(0.0, delta * 20.0),
                unit: egui::MouseWheelUnit::Line,
                modifiers: current_modifiers(),
            });
        }
        WM_CHAR => {
            if let Some(c) = char::from_u32(wparam.0 as u32) {
                if !c.is_control() {
                    events.push(Event::Text(c.to_string()));
                }
            }
        }
        WM_KEYDOWN | WM_KEYUP => {
            if let Some(key) = vk_to_egui_key(wparam.0 as u32) {
                events.push(Event::Key {
                    key,
                    physical_key: None,
                    pressed: msg == WM_KEYDOWN,
                    repeat: false,
                    modifiers: current_modifiers(),
                });
            }
        }
        _ => {}
    }
}

fn xy_from_lparam(lparam: LPARAM) -> (i16, i16) {
    let x = (lparam.0 & 0xFFFF) as i16;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16;
    (x, y)
}

fn current_modifiers() -> Modifiers {
    let ctrl = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(windows::Win32::UI::Input::KeyboardAndMouse::VK_CONTROL.0 as i32) } < 0;
    let shift = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(windows::Win32::UI::Input::KeyboardAndMouse::VK_SHIFT.0 as i32) } < 0;
    let alt = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(windows::Win32::UI::Input::KeyboardAndMouse::VK_MENU.0 as i32) } < 0;
    Modifiers {
        alt,
        ctrl,
        shift,
        mac_cmd: false,
        command: ctrl,
    }
}

fn vk_to_egui_key(vk: u32) -> Option<Key> {
    match vk {
        0x08 => Some(Key::Backspace),
        0x0D => Some(Key::Enter),
        0x1B => Some(Key::Escape),
        0x2E => Some(Key::Delete),
        0x25 => Some(Key::ArrowLeft),
        0x26 => Some(Key::ArrowUp),
        0x27 => Some(Key::ArrowRight),
        0x28 => Some(Key::ArrowDown),
        _ => None,
    }
}

struct EguiD3D11Painter {
    swapchain: Swapchain,
    renderer: egui_directx11::Renderer,
}

impl EguiD3D11Painter {
    fn new(hwnd: HWND, device: &ID3D11Device) -> Result<Self> {
        let swapchain = Swapchain::create(device, hwnd, 1280, 800)?;
        let renderer = egui_directx11::Renderer::new(device)
            .map_err(|e| AppError::Renderer(format!("Failed to initialize egui-directx11 renderer: {:?}", e)))?;
        Ok(Self { swapchain, renderer })
    }

    fn paint(
        &mut self,
        ctx: &egui::Context,
        output: egui::FullOutput,
        _device: &ID3D11Device,
        context: &ID3D11DeviceContext,
    ) -> Result<()> {
        let (renderer_output, _platform_output, _viewport_output) = egui_directx11::split_output(output);

        if let Some(ref rtv) = self.swapchain.rtv {
            unsafe {
                context.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            }

            self.renderer.render(context, rtv, ctx, renderer_output)
                .map_err(|e| AppError::Renderer(format!("egui render failed: {:?}", e)))?;

            self.swapchain.present()?;
        }
        Ok(())
    }
}
