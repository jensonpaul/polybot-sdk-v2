use crate::ui::PolymarketDashboardApp;
use crate::ui_types::{NotificationKind, UiCommand};
use eframe::egui;

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
    
                        let cmd = UiCommand::InitializeClient {
                            token: self.bearer_token.clone(),
                        };

                        match self.cmd_tx.try_send(cmd) {
                            Ok(_) => tracing::info!("UI Core: Sent initialization token to worker."),
                            Err(e) => tracing::error!("UI Error: Auth pipeline transmission failed: {:?}", e),
                        }
                        
                        self.push_toast("Access Granted.".to_string(), NotificationKind::Success);
                    }
                });
            });
        });
    }
}