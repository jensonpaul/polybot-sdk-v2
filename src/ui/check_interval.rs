use crate::ui::PolymarketDashboardApp;
use crate::ui_types::{NotificationKind, UiCommand};
use eframe::egui;

impl PolymarketDashboardApp {
    pub fn render_polling_interval(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Order Check Interval (ms):");

            let response = ui.text_edit_singleline(&mut self.poll_interval_ms);

            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Ok(ms) = self.poll_interval_ms.parse::<u64>() {
                    let tx = self.cmd_tx.clone();

                    tokio::spawn(async move {
                        let _ = tx
                            .try_send(UiCommand::UpdatePollInterval {
                                milliseconds: ms,
                            });
                    });

                    self.push_toast(
                        format!("Polling interval updated to {} ms", ms),
                        NotificationKind::Success,
                    );
                } else {
                    self.push_toast(
                        "Invalid interval value".to_string(),
                        NotificationKind::Error,
                    );
                }
            }
        });
    }
}