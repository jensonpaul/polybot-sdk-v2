use crate::ui::PolymarketDashboardApp;
use crate::ui_types::UiCommand;
use crate::worker_config::Queue;
use eframe::egui;
use crate::ui::theme::Theme;

impl PolymarketDashboardApp {

    pub fn render_order_consoles(
        &mut self,
        ui: &mut egui::Ui,
        current_ts: u64,
    ) {

        ui.columns(3, |cols| {

            // =====================================================
            // LIMIT ORDER PANEL
            // =====================================================

            egui::Frame::none()
                .fill(Theme::BG_ELEVATED)
                .stroke(
                    egui::Stroke::new(
                        1.0,
                        Theme::BORDER
                    )
                )
                .corner_radius(8.0)
                .inner_margin(12.0)
                .show(&mut cols[0], |ui| {

                    ui.colored_label(
                        Theme::TEXT_PRIMARY,

                        egui::RichText::new(
                            "📥 LIMIT ORDER CONSOLE"
                        )
                        .strong()
                        .size(16.0)
                    );

                    ui.separator();

                    ui.horizontal(|ui| {

                        ui.radio_value(
                            &mut self.limit_side_buy,
                            true,
                            "Buy"
                        );

                        ui.radio_value(
                            &mut self.limit_side_buy,
                            false,
                            "Sell"
                        );

                        ui.separator();

                        ui.radio_value(
                            &mut self.limit_token_up,
                            true,
                            "UP"
                        );

                        ui.radio_value(
                            &mut self.limit_token_up,
                            false,
                            "DOWN"
                        );
                    });

                    ui.add_space(8.0);

                    labeled_input(
                        ui,
                        "Price",
                        &mut self.limit_price,
                    );

                    labeled_input(
                        ui,
                        "Size",
                        &mut self.limit_size,
                    );

                    labeled_input(
                        ui,
                        "Rapid Exit",
                        &mut self.limit_rapid_price,
                    );

                    ui.add_space(10.0);

                    let button_fill =
                        if self.limit_side_buy {
                            Theme::BUY_GREEN_BG
                        } else {
                            Theme::SELL_RED_BG
                        };

                    let button_stroke =
                        if self.limit_side_buy {
                            Theme::BUY_GREEN
                        } else {
                            Theme::SELL_RED
                        };

                    ui.scope(|ui| {

                        ui.visuals_mut()
                            .widgets
                            .inactive
                            .bg_fill = button_fill;

                        ui.visuals_mut()
                            .widgets
                            .inactive
                            .bg_stroke =
                                egui::Stroke::new(
                                    1.0,
                                    button_stroke
                                );

                        if ui.button(
                            egui::RichText::new(
                                "EXECUTE LIMIT ORDER"
                            )
                            .strong()
                        )
                        .clicked()
                        {

                            let cmd =
                                UiCommand::PlaceLimit {

                                    side:
                                        if self.limit_side_buy {
                                            "buy".into()
                                        } else {
                                            "sell".into()
                                        },

                                    token:
                                        if self.limit_token_up {
                                            "up".into()
                                        } else {
                                            "down".into()
                                        },

                                    price:
                                        self.limit_price.clone(),

                                    size:
                                        self.limit_size.clone(),

                                    rapid_price:
                                        self.limit_rapid_price.clone(),

                                    window_ts:
                                        current_ts,
                                };

                            match self.cmd_tx.try_send(cmd) {

                                Ok(_) => {
                                    tracing::info!(
                                        "PlaceLimit dispatched"
                                    );
                                }

                                Err(e) => {
                                    tracing::error!(
                                        "PlaceLimit failed: {:?}",
                                        e
                                    );
                                }
                            }
                        }
                    });
                });

            // =====================================================
            // MARKET ORDER PANEL
            // =====================================================

            egui::Frame::none()
                .fill(Theme::BG_ELEVATED)
                .stroke(
                    egui::Stroke::new(
                        1.0,
                        Theme::BORDER
                    )
                )
                .corner_radius(8.0)
                .inner_margin(12.0)
                .show(&mut cols[1], |ui| {

                    ui.colored_label(
                        Theme::TEXT_PRIMARY,

                        egui::RichText::new(
                            "⚡ MARKET ORDER CONSOLE"
                        )
                        .strong()
                        .size(16.0)
                    );

                    ui.separator();

                    ui.horizontal(|ui| {

                        ui.radio_value(
                            &mut self.market_token_up,
                            true,
                            "UP"
                        );

                        ui.radio_value(
                            &mut self.market_token_up,
                            false,
                            "DOWN"
                        );
                    });

                    ui.add_space(8.0);

                    ui.checkbox(
                        &mut self.market_use_usdc,
                        "Use USDC Input"
                    );

                    ui.add_space(6.0);

                    if self.market_use_usdc {

                        labeled_input(
                            ui,
                            "USDC",
                            &mut self.market_usdc,
                        );

                    } else {

                        labeled_input(
                            ui,
                            "Shares",
                            &mut self.market_shares,
                        );
                    }

                    ui.add_space(8.0);

                    ui.horizontal(|ui| {

                        ui.label(
                            egui::RichText::new(
                                "Execution"
                            )
                            .color(
                                Theme::TEXT_MUTED
                            )
                        );

                        ui.radio_value(
                            &mut self.market_type_fok,
                            true,
                            "FOK"
                        );

                        ui.radio_value(
                            &mut self.market_type_fok,
                            false,
                            "FAK"
                        );
                    });

                    ui.add_space(10.0);

                    ui.scope(|ui| {

                        ui.visuals_mut()
                            .widgets
                            .inactive
                            .bg_fill =
                                Theme::BUY_GREEN_BG;

                        ui.visuals_mut()
                            .widgets
                            .inactive
                            .bg_stroke =
                                egui::Stroke::new(
                                    1.0,
                                    Theme::BUY_GREEN
                                );

                        if ui.button(
                            egui::RichText::new(
                                "EXECUTE MARKET ORDER"
                            )
                            .strong()
                        )
                        .clicked()
                        {

                            let cmd =
                                UiCommand::PlaceMarket {

                                    side: "buy".into(),

                                    token:
                                        if self.market_token_up {
                                            "up".into()
                                        } else {
                                            "down".into()
                                        },

                                    usdc:
                                        if self.market_use_usdc {
                                            Some(
                                                self.market_usdc.clone()
                                            )
                                        } else {
                                            None
                                        },

                                    shares:
                                        if !self.market_use_usdc {
                                            Some(
                                                self.market_shares.clone()
                                            )
                                        } else {
                                            None
                                        },

                                    order_type:
                                        Some(
                                            if self.market_type_fok {
                                                "FOK".into()
                                            } else {
                                                "FAK".into()
                                            }
                                        ),

                                    window_ts:
                                        current_ts,
                                };

                            if let Err(e) =
                                self.cmd_tx.try_send(cmd)
                            {
                                tracing::error!(
                                    "PlaceMarket failed: {:?}",
                                    e
                                );
                            }
                        }
                    });
                });

            // =====================================================
            // POLL INTERVAL PANEL
            // =====================================================

            egui::Frame::none()
                .fill(Theme::BG_ELEVATED)
                .stroke(
                    egui::Stroke::new(
                        1.0,
                        Theme::BORDER
                    )
                )
                .corner_radius(8.0)
                .inner_margin(12.0)
                .show(&mut cols[2], |ui| {

                    ui.colored_label(
                        Theme::TEXT_PRIMARY,
                        egui::RichText::new(
                            "⏱ INTERVAL CONFIG"
                        )
                        .strong()
                        .size(16.0)
                    );

                    ui.separator();

                    self.render_poll_interval_input(
                        ui,
                        "Orders",
                        Queue::Orders,
                    );

                    ui.add_space(8.0);

                    self.render_poll_interval_input(
                        ui,
                        "Trades",
                        Queue::Trades,
                    );

                    ui.add_space(8.0);

                    self.render_poll_interval_input(
                        ui,
                        "Rapid Sell",
                        Queue::RapidSell,
                    );
                });
        });
    }
}

fn labeled_input(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
) {

    ui.horizontal(|ui| {

        ui.label(
            egui::RichText::new(label)
                .color(Theme::TEXT_MUTED)
        );

        ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(120.0)
        );
    });
}