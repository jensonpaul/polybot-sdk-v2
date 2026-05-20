use crate::ui::PolymarketDashboardApp;
use crate::ui_types::UiCommand;
use eframe::egui;

impl PolymarketDashboardApp {
    pub fn render_order_consoles(&mut self, ui: &mut egui::Ui, current_ts: u64) {
        ui.columns(2, |cols| {
            // COLUMN 1: Limit Orders Console Placement
            cols[0].group(|ui| {
                ui.heading("📥 Buy Limit Order Console");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.limit_side_buy, true, "Buy");
                    ui.radio_value(&mut self.limit_side_buy, false, "Sell");
                    ui.separator();
                    ui.radio_value(&mut self.limit_token_up, true, "UP Contract");
                    ui.radio_value(&mut self.limit_token_up, false, "DOWN Contract");
                });
                ui.horizontal(|ui| {
                    ui.label("Price Target ($):");
                    ui.text_edit_singleline(&mut self.limit_price);
                });
                ui.horizontal(|ui| {
                    ui.label("Contract Size:   ");
                    ui.text_edit_singleline(&mut self.limit_size);
                });
                if ui.button("Execute Limit Placement").clicked() {
                    let cmd = UiCommand::PlaceLimit {
                        side: if self.limit_side_buy { "buy".into() } else { "sell".into() },
                        token: if self.limit_token_up { "up".into() } else { "down".into() },
                        price: self.limit_price.clone(),
                        size: self.limit_size.clone(),
                        window_ts: current_ts,
                    };

                    match self.cmd_tx.try_send(cmd) {
                        Ok(_) => tracing::info!("UI Core: Dispatched PlaceLimit event to channel."),
                        Err(e) => tracing::error!("UI Error: Channel submission failed! Reason: {:?}", e),
                    }
                }
            });

            // COLUMN 2: Market Instant Orders Panel Row View Layout
            cols[1].group(|ui| {
                ui.heading("⚡ Buy Market Instant Console");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.market_token_up, true, "UP Contract");
                    ui.radio_value(&mut self.market_token_up, false, "DOWN Contract");
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.market_use_usdc, "Utilize Total USDC Collateral Pool Input Allocation");
                });
                if self.market_use_usdc {
                    ui.horizontal(|ui| {
                        ui.label("USDC Amount ($):");
                        ui.text_edit_singleline(&mut self.market_usdc);
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label("Target Size (Shares):");
                        ui.text_edit_singleline(&mut self.market_shares);
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Execution Rules Validation Type: ");
                    ui.radio_value(&mut self.market_type_fok, true, "FOK (Fill or Kill)");
                    ui.radio_value(&mut self.market_type_fok, false, "FAK (Fill and Kill)");
                });
                if ui.button("Dispatch Market Transaction").clicked() {
                    let cmd = UiCommand::PlaceMarket {
                        side: "buy".into(),
                        token: if self.market_token_up { "up".into() } else { "down".into() },
                        usdc: if self.market_use_usdc { Some(self.market_usdc.clone()) } else { None },
                        shares: if !self.market_use_usdc { Some(self.market_shares.clone()) } else { None },
                        order_type: Some(if self.market_type_fok { "FOK".into() } else { "FAK".into() }),
                        window_ts: current_ts,
                    };

                    if let Err(e) = self.cmd_tx.try_send(cmd) {
                        tracing::error!("UI Error: PlaceMarket dispatch failed: {:?}", e);
                    }
                }
            });
        });
    }
}