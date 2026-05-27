use crate::ui::PolymarketDashboardApp;
use crate::ui_types::{NotificationKind, UiCommand};
use crate::worker_config::Queue;
use eframe::egui;
use crate::ui::theme::Theme;

impl PolymarketDashboardApp {

    pub fn render_poll_interval_input(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        queue: Queue,
    ) {

        ui.horizontal(|ui| {

            ui.label(
                egui::RichText::new(label)
                    .color(Theme::TEXT_MUTED)
            );

            ui.add_space(6.0);

            let response = ui.add(

                egui::TextEdit::singleline(
                    self.interval_inputs.get_mut(queue)
                )
                .desired_width(70.0)
            );

            ui.label(
                egui::RichText::new("ms")
                    .color(Theme::TEXT_MUTED)
            );

            if response.lost_focus()
                && ui.input(|i|
                    i.key_pressed(egui::Key::Enter)
                )
            {

                let value =
                    self.interval_inputs.get(queue);

                match value.parse::<u64>() {

                    Ok(ms) => {

                        let tx = self.cmd_tx.clone();

                        tokio::spawn(async move {

                            let _ =
                                tx.try_send(
                                    UiCommand::UpdatePollInterval {
                                        milliseconds: ms,
                                        queue,
                                    }
                                );
                        });

                        self.push_toast(
                            format!(
                                "{} updated to {} ms",
                                label,
                                ms
                            ),
                            NotificationKind::Success,
                        );
                    }

                    Err(_) => {

                        self.push_toast(
                            "Invalid interval".to_string(),
                            NotificationKind::Error,
                        );
                    }
                }
            }
        });
    }
}