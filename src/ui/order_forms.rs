use crate::ui::widgets::{
    compact_input,
    panel_frame,
    themed_button,
};

use crate::ui::theme::Theme;

use crate::ui::PolymarketDashboardApp;

use crate::ui_types::UiCommand;

use crate::worker_config::Queue;

use eframe::egui;

impl PolymarketDashboardApp {

    pub fn render_order_consoles(
        &mut self,
        ui: &mut egui::Ui,
        current_ts: u64,
    ) {

        //let width = ui.available_width();

        ui.horizontal_wrapped(|ui| {

            // =====================================================
            // LIMIT ORDER
            // =====================================================

            panel_frame()
                .show(ui, |ui| {

                    //ui.set_width(ui.available_width().min(520.0));
                    //ui.set_width((width / 3.0).max(280.0));
                    ui.set_min_width(520.0);

                    ui.horizontal(|ui| {

                        ui.label(
                            egui::RichText::new(
                                "📥 LIMIT"
                            )
                            .strong()
                            .color(
                                Theme::TEXT_PRIMARY
                            )
                        );

                        ui.separator();

                        ui.radio_value(
                            &mut self.limit_side_buy,
                            true,
                            "BUY"
                        );

                        ui.radio_value(
                            &mut self.limit_side_buy,
                            false,
                            "SELL"
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

                        ui.separator();

                        compact_input(
                            ui,
                            "Price",
                            &mut self.limit_price,
                            55.0,
                        );

                        compact_input(
                            ui,
                            "Size",
                            &mut self.limit_size,
                            55.0,
                        );

                        compact_input(
                            ui,
                            "Rapid",
                            &mut self.limit_rapid_price,
                            55.0,
                        );

                        let (
                            fill,
                            stroke
                        ) = if self.limit_side_buy {

                            (
                                Theme::BUY_GREEN_BG,
                                Theme::BUY_GREEN,
                            )

                        } else {

                            (
                                Theme::SELL_RED_BG,
                                Theme::SELL_RED,
                            )
                        };

                        if themed_button(
                            ui,
                            "EXECUTE",
                            fill,
                            stroke,
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

                            let _ =
                                self.cmd_tx.try_send(cmd);
                        }
                    });
                });

            // =====================================================
            // MARKET ORDER
            // =====================================================

            panel_frame()
                .show(ui, |ui| {

                    //ui.set_width(ui.available_width().min(500.0));
                    //ui.set_width((width / 3.0).max(280.0));
                    ui.set_min_width(500.0);

                    ui.horizontal(|ui| {

                        ui.label(
                            egui::RichText::new(
                                "⚡ MARKET"
                            )
                            .strong()
                            .color(
                                Theme::TEXT_PRIMARY
                            )
                        );

                        ui.separator();

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

                        ui.separator();

                        ui.checkbox(
                            &mut self.market_use_usdc,
                            "USDC"
                        );

                        if self.market_use_usdc {

                            compact_input(
                                ui,
                                "Amount",
                                &mut self.market_usdc,
                                60.0,
                            );

                        } else {

                            compact_input(
                                ui,
                                "Shares",
                                &mut self.market_shares,
                                60.0,
                            );
                        }

                        ui.separator();

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

                        if themed_button(
                            ui,
                            "EXECUTE",
                            Theme::BUY_GREEN_BG,
                            Theme::BUY_GREEN,
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

                            let _ =
                                self.cmd_tx.try_send(cmd);
                        }
                    });
                });

            // =====================================================
            // INTERVALS
            // =====================================================

            panel_frame()
                .show(ui, |ui| {

                    //ui.set_width(ui.available_width().min(320.0));
                    //ui.set_width((width / 3.0).max(280.0));
                    ui.set_min_width(320.0);

                    ui.horizontal(|ui| {

                        ui.label(
                            egui::RichText::new("⏱ POLL")
                                .strong()
                                .color(Theme::TEXT_PRIMARY)
                        );

                        ui.separator();

                        self.render_poll_interval_input(
                            ui,
                            "Orders",
                            Queue::Orders,
                        );

                        ui.separator();

                        self.render_poll_interval_input(
                            ui,
                            "Trades",
                            Queue::Trades,
                        );

                        ui.separator();

                        self.render_poll_interval_input(
                            ui,
                            "Rapid",
                            Queue::RapidSell,
                        );
                    });
                });
        });
    }
}