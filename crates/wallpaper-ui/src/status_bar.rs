use crate::ipc_client::ConnectionStatus;

pub struct StatusBar;

impl StatusBar {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&mut self, ui: &mut egui::Ui, status: &ConnectionStatus, last_error: Option<&str>) {
        ui.horizontal(|ui| {
            match status {
                ConnectionStatus::Connected(s) => {
                    ui.label(format!(
                        "🟢 Daemon: Connected (v{}, {} monitor(s), {})",
                        s.protocol_version,
                        s.active_monitors,
                        if s.is_paused { "Paused" } else { "Active" }
                    ));
                }
                ConnectionStatus::Connecting => {
                    ui.label("🟡 Daemon: Connecting...");
                }
                ConnectionStatus::Disconnected => {
                    ui.label("🔴 Daemon: Disconnected (reconnecting...)");
                }
                ConnectionStatus::Error(reason) => {
                    ui.label(format!("🔴 Daemon Error: {}", reason));
                }
            }
            if let Some(reason) = last_error {
                ui.separator();
                ui.colored_label(
                    egui::Color32::from_rgb(220, 80, 80),
                    format!("⚠ {}", reason),
                );
            }
        });
    }
}
