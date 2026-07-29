use std::time::{Duration, Instant};

use crate::theme;
use egui::{Align2, Area, Color32, Frame, Id, Order, Vec2};

pub type ToastEvent = (String, ToastKind);

#[derive(Clone, Copy, PartialEq)]
pub enum ToastKind {
    Success,
    Error,
    #[allow(dead_code)]
    Info,
    Warning,
}

impl ToastKind {
    fn color(&self) -> Color32 {
        match self {
            ToastKind::Success => Color32::from_rgb(76, 175, 80),
            ToastKind::Error => Color32::from_rgb(244, 67, 54),
            ToastKind::Info => Color32::from_rgb(33, 150, 243),
            ToastKind::Warning => Color32::from_rgb(255, 152, 0),
        }
    }

    fn icon(&self) -> &str {
        match self {
            ToastKind::Success => theme::ICON_CHECK,
            ToastKind::Error => theme::ICON_CLOSE,
            ToastKind::Info => theme::ICON_INFO,
            ToastKind::Warning => theme::ICON_WARNING,
        }
    }

    fn duration(&self) -> Duration {
        match self {
            ToastKind::Success | ToastKind::Info => Duration::from_secs(3),
            ToastKind::Warning => Duration::from_secs(4),
            ToastKind::Error => Duration::from_secs(6),
        }
    }
}

pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    pub created_at: Instant,
}

impl Toast {
    fn remaining(&self) -> f32 {
        let elapsed = Instant::now().duration_since(self.created_at).as_secs_f32();
        let total = self.kind.duration().as_secs_f32();
        (total - elapsed).max(0.0) / total
    }
}

pub struct ToastManager {
    toasts: Vec<Toast>,
}

impl ToastManager {
    pub fn new() -> Self {
        Self { toasts: Vec::new() }
    }

    pub fn add(&mut self, message: impl Into<String>, kind: ToastKind) {
        self.toasts.push(Toast {
            message: message.into(),
            kind,
            created_at: Instant::now(),
        });
    }

    pub fn success(&mut self, message: impl Into<String>) {
        self.add(message, ToastKind::Success);
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.add(message, ToastKind::Error);
    }

    pub fn info(&mut self, message: impl Into<String>) {
        self.add(message, ToastKind::Info);
    }

    pub fn warning(&mut self, message: impl Into<String>) {
        self.add(message, ToastKind::Warning);
    }

    pub fn drain_events(&mut self, rx: &std::sync::mpsc::Receiver<ToastEvent>) {
        while let Ok((msg, kind)) = rx.try_recv() {
            match kind {
                ToastKind::Success => self.success(msg),
                ToastKind::Error => self.error(msg),
                ToastKind::Info => self.info(msg),
                ToastKind::Warning => self.warning(msg),
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.toasts
            .retain(|t| now.duration_since(t.created_at) < t.kind.duration());

        if self.toasts.is_empty() {
            return;
        }

        // Request continuous repaints for smooth alpha fade animation.
        ctx.request_repaint();

        Area::new(Id::new("aura_toast_overlay"))
            .anchor(Align2::RIGHT_TOP, Vec2::new(-16.0, 16.0))
            .order(Order::Foreground)
            .show(ctx, |ui| {
                ui.set_min_width(340.0);
                ui.set_max_width(340.0);
                let max_toasts = 5;
                let start = self.toasts.len().saturating_sub(max_toasts);
                for toast in &self.toasts[start..] {
                    draw_toast(ui, toast);
                }
            });
    }
}

impl Default for ToastManager {
    fn default() -> Self {
        Self::new()
    }
}

fn draw_toast(ui: &mut egui::Ui, toast: &Toast) {
    let alpha = toast.remaining().clamp(0.0, 1.0);
    let bg = Color32::from_rgba_premultiplied(30, 30, 30, (180.0 * alpha) as u8);
    let accent = toast.kind.color();
    let accent_faded =
        Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), (255.0 * alpha) as u8);
    let text_faded = Color32::from_rgba_premultiplied(255, 255, 255, (255.0 * alpha) as u8);

    let icon = toast.kind.icon();

    Frame::new()
        .fill(bg)
        .corner_radius(theme::RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(
            theme::SPACING_MD as i8,
            theme::SPACING_SM as i8,
        ))
        .show(ui, |ui| {
            ui.set_max_width(316.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(accent_faded, icon);
                    ui.add(
                        egui::Label::new(egui::RichText::new(&toast.message).color(text_faded))
                            .wrap(),
                    );
                });
            });
        });

    ui.add_space(theme::SPACING_SM);
}
