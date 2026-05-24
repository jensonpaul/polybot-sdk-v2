use crate::ui::PolymarketDashboardApp;
use crate::ui_types::{NotificationKind, UiCommand};
use crate::worker_config::Queue;
use eframe::egui;

impl PolymarketDashboardApp {
    pub fn render_poll_interval_input(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        queue: Queue,
    ) {
        ui.horizontal(|ui| {
            ui.label(label);

            let response = ui.text_edit_singleline(
                self.interval_inputs.get_mut(queue)
            );

            if response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
            {
                let value = self.interval_inputs.get(queue);

                match value.parse::<u64>() {
                    Ok(ms) => {
                        let tx = self.cmd_tx.clone();

                        tokio::spawn(async move {
                            let _ = tx.try_send(
                                UiCommand::UpdatePollInterval {
                                    milliseconds: ms,
                                    queue,
                                },
                            );
                        });

                        self.push_toast(
                            format!(
                                "{} interval updated to {} ms",
                                label,
                                ms
                            ),
                            NotificationKind::Success,
                        );
                    }

                    Err(_) => {
                        self.push_toast(
                            "Invalid interval value".to_string(),
                            NotificationKind::Error,
                        );
                    }
                }
            }
        });
    }
}