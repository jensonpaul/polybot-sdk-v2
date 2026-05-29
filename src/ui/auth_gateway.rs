use eframe::egui;
use crate::messages::UiCommand;
use crate::state::NotificationKind;
use crate::ui::PolymarketDashboardApp;

impl PolymarketDashboardApp {
    pub fn render_auth_gateway(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.group(|ui| {
                    ui.set_width(400.0);
                    ui.heading("System Gateway Access Authorization");
                    ui.add_space(10.0);
                    ui.label("Enter Bearer Token to access local engine resources:");
                    ui.add(egui::TextEdit::singleline(&mut self.bearer_token).password(true));
                    ui.add_space(10.0);
                    if ui.button("Authorize Engine Console").clicked() {
                        self.is_authenticated = true;
                        let _ = self.cmd_tx.try_send(UiCommand::InitializeClient {
                            token: self.bearer_token.clone(),
                        });
                        self.push_toast("Access Granted.".into(), NotificationKind::Success);
                    }
                });
            });
        });
    }
}
