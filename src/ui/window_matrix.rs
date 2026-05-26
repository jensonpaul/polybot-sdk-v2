use crate::ui::theme::Theme;

use crate::ui::widgets::{
    compact_input,
    panel_frame,
    themed_button,
};

use crate::ui::PolymarketDashboardApp;

use crate::ui_types::{
    LocalOrderStatus,
    UiCommand,
};

use eframe::egui;

impl PolymarketDashboardApp {

    pub fn render_lifecycle_matrix(
        &mut self,
        ui: &mut egui::Ui,
    ) {

        ui.horizontal(|ui| {

            ui.heading(

                egui::RichText::new(
                    "📊 WINDOW LIFECYCLE MATRIX"
                )
                .color(
                    Theme::TEXT_PRIMARY
                )
            );

            ui.separator();

            ui.checkbox(
                &mut self.auto_refresh_active,
                "LIVE"
            );
        });

        ui.add_space(10.0);

        let mut windows_to_remove =
            Vec::new();

        for (
            w_idx,
            window
        ) in self.windows
            .iter_mut()
            .enumerate()
        {

            panel_frame()
                .show(ui, |ui| {

                    // =====================================================
                    // HEADER
                    // =====================================================

                    ui.horizontal(|ui| {

                        if themed_button(

                            ui,

                            if window.is_expanded {
                                "▼"
                            } else {
                                "▶"
                            },

                            Theme::BLUE_BG,
                            Theme::BLUE,
                        )
                        .clicked()
                        {
                            window.is_expanded =
                                !window.is_expanded;
                        }

                        ui.label(

                            egui::RichText::new(

                                format!(
                                    "WINDOW {} | {} | ORDERS {}",
                                    window.timestamp_5m,
                                    window.slug,
                                    window.orders.len(),
                                )
                            )
                            .strong()
                            .monospace()
                            .color(
                                Theme::TEXT_PRIMARY
                            )
                        );

                        ui.with_layout(

                            egui::Layout::right_to_left(
                                egui::Align::Center
                            ),

                            |ui| {

                                if themed_button(
                                    ui,
                                    "CLOSE",
                                    Theme::SELL_RED_BG,
                                    Theme::SELL_RED,
                                )
                                .clicked()
                                {
                                    windows_to_remove
                                        .push(w_idx);
                                }

                                if themed_button(
                                    ui,
                                    "CANCEL ALL",
                                    Theme::SELL_RED_BG,
                                    Theme::SELL_RED,
                                )
                                .clicked()
                                {
                                    let _ =
                                        self.cmd_tx.try_send(

                                            UiCommand::CancelAllInWindow {

                                                window_ts:
                                                    window.timestamp_5m,
                                            }
                                        );
                                }
                            }
                        );
                    });

                    // =====================================================
                    // MARKET SNAPSHOT
                    // =====================================================

                    if let Some(prices) =
                        &window.market_prices
                    {

                        let snap =
                            prices.load();

                        let up_cents =
                            snap.up_price * 100.0;

                        let down_cents =
                            snap.down_price * 100.0;

                        ui.add_space(8.0);

                        ui.horizontal(|ui| {

                            panel_frame()
                                .fill(
                                    Theme::BUY_GREEN_BG
                                )
                                .stroke(
                                    egui::Stroke::new(
                                        1.0,
                                        Theme::BUY_GREEN
                                    )
                                )
                                .show(ui, |ui| {

                                    ui.label(

                                        egui::RichText::new(

                                            format!(
                                                "▲ UP {:.1}¢",
                                                up_cents
                                            )
                                        )
                                        .strong()
                                        .monospace()
                                        .color(
                                            Theme::TEXT_PRIMARY
                                        )
                                    );
                                });

                            panel_frame()
                                .fill(
                                    Theme::SELL_RED_BG
                                )
                                .stroke(
                                    egui::Stroke::new(
                                        1.0,
                                        Theme::SELL_RED
                                    )
                                )
                                .show(ui, |ui| {

                                    ui.label(

                                        egui::RichText::new(

                                            format!(
                                                "▼ DOWN {:.1}¢",
                                                down_cents
                                            )
                                        )
                                        .strong()
                                        .monospace()
                                        .color(
                                            Theme::TEXT_PRIMARY
                                        )
                                    );
                                });

                            if snap.stale {

                                ui.colored_label(
                                    Theme::WARNING,
                                    "STALE"
                                );
                            }

                            if !snap.connected {

                                ui.colored_label(
                                    Theme::SELL_RED,
                                    "DISCONNECTED"
                                );
                            }
                        });
                    }

                    // =====================================================
                    // ORDERS
                    // =====================================================

                    if window.is_expanded {

                        ui.add_space(8.0);

                        egui::ScrollArea::vertical()

                            .id_salt(

                                format!(
                                    "orders_scroll_{}",
                                    window.timestamp_5m
                                )
                            )

                            .max_height(450.0)

                            .auto_shrink([false; 2])

                            .show(ui, |ui| {

                                for order in
                                    window.orders.iter_mut()
                                {

                                    let (
                                        fill,
                                        border
                                    ) = match &order.status {

                                        LocalOrderStatus::Canceled => (

                                            egui::Color32::from_rgba_unmultiplied(
                                                90,
                                                90,
                                                90,
                                                35,
                                            ),

                                            egui::Color32::from_rgb(
                                                120,
                                                120,
                                                120,
                                            ),
                                        ),

                                        LocalOrderStatus::Failed(_) => (

                                            Theme::SELL_RED_BG,

                                            Theme::SELL_RED,
                                        ),

                                        _ => {

                                            match (
                                                order.side.to_lowercase().as_str(),
                                                order.token.to_lowercase().as_str(),
                                            ) {

                                                ("buy", "up") => (

                                                    Theme::BUY_GREEN_BG,

                                                    Theme::BUY_GREEN,
                                                ),

                                                ("buy", "down") => (

                                                    Theme::SELL_RED_BG,

                                                    Theme::SELL_RED,
                                                ),

                                                ("sell", _) => (

                                                    Theme::BLUE_BG,

                                                    Theme::BLUE,
                                                ),

                                                _ => (

                                                    Theme::BG_ELEVATED,

                                                    Theme::BORDER,
                                                ),
                                            }
                                        }
                                    };

                                    panel_frame()

                                        .fill(fill)

                                        .stroke(

                                            egui::Stroke::new(
                                                1.0,
                                                border,
                                            )
                                        )

                                        .show(ui, |ui| {

                                            // =====================================================
                                            // ORDER HEADER
                                            // =====================================================

                                            ui.horizontal(|ui| {

                                                ui.label(

                                                    egui::RichText::new(

                                                        format!(
                                                            "[{}] {} {} @ {} x {}",
                                                            order.id,
                                                            order.side.to_uppercase(),
                                                            order.token.to_uppercase(),
                                                            order.price,
                                                            order.size,
                                                        )
                                                    )
                                                    .monospace()
                                                    .strong()
                                                    .color(
                                                        Theme::TEXT_PRIMARY
                                                    )
                                                );

                                                ui.separator();

                                                ui.label(

                                                    egui::RichText::new(

                                                        format!(
                                                            "MATCHED {}",
                                                            order.size_matched
                                                        )
                                                    )
                                                    .monospace()
                                                    .color(
                                                        Theme::TEXT_PRIMARY
                                                    )
                                                );

                                                ui.with_layout(

                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center
                                                    ),

                                                    |ui| {

                                                        if themed_button(
                                                            ui,
                                                            "CANCEL",
                                                            Theme::SELL_RED_BG,
                                                            Theme::SELL_RED,
                                                        )
                                                        .clicked()
                                                        {

                                                            let _ =
                                                                self.cmd_tx.try_send(

                                                                    UiCommand::CancelIndividual {

                                                                        order_id:
                                                                            order.id.clone(),

                                                                        window_ts:
                                                                            window.timestamp_5m,
                                                                    }
                                                                );
                                                        }

                                                        if themed_button(
                                                            ui,
                                                            "SYNC",
                                                            Theme::BLUE_BG,
                                                            Theme::BLUE,
                                                        )
                                                        .clicked()
                                                        {

                                                            let _ =
                                                                self.cmd_tx.try_send(

                                                                    UiCommand::CheckStatus {

                                                                        order_id:
                                                                            order.id.clone(),

                                                                        window_ts:
                                                                            window.timestamp_5m,
                                                                    }
                                                                );
                                                        }
                                                    }
                                                );
                                            });

                                            // =====================================================
                                            // INLINE EXIT DESK
                                            // =====================================================

                                            if order.side.eq_ignore_ascii_case("buy")
                                                && order
                                                    .size_matched
                                                    .trim()
                                                    .parse::<f64>()
                                                    .map(|n| n > 0.0)
                                                    .unwrap_or(false)
                                            {

                                                ui.add_space(8.0);

                                                panel_frame()

                                                    .fill(
                                                        Theme::BG_PANEL
                                                    )

                                                    .show(ui, |ui| {

                                                        ui.horizontal_wrapped(|ui| {

                                                            ui.label(

                                                                egui::RichText::new(
                                                                    "INLINE EXIT DESK"
                                                                )
                                                                .strong()
                                                                .color(
                                                                    Theme::BLUE
                                                                )
                                                            );

                                                            compact_input(
                                                                ui,
                                                                "Price",
                                                                &mut order.inline_sell_price,
                                                                60.0,
                                                            );

                                                            compact_input(
                                                                ui,
                                                                "Size",
                                                                &mut order.inline_sell_size,
                                                                60.0,
                                                            );

                                                            ui.radio_value(
                                                                &mut order.inline_sell_market_type,
                                                                "FAK".to_string(),
                                                                "FAK",
                                                            );

                                                            ui.radio_value(
                                                                &mut order.inline_sell_market_type,
                                                                "FOK".to_string(),
                                                                "FOK",
                                                            );

                                                            if themed_button(
                                                                ui,
                                                                "LIMIT EXIT",
                                                                Theme::SELL_RED_BG,
                                                                Theme::SELL_RED,
                                                            )
                                                            .clicked()
                                                            {

                                                                let _ =
                                                                    self.cmd_tx.try_send(

                                                                        UiCommand::PlaceLimit {

                                                                            side: "sell".into(),

                                                                            token:
                                                                                order.token.clone(),

                                                                            price:
                                                                                order.inline_sell_price.clone(),

                                                                            size:
                                                                                order.inline_sell_size.clone(),

                                                                            rapid_price:
                                                                                "0".into(),

                                                                            window_ts:
                                                                                window.timestamp_5m,
                                                                        }
                                                                    );
                                                            }

                                                            if themed_button(
                                                                ui,
                                                                "MARKET EXIT",
                                                                Theme::BLUE_BG,
                                                                Theme::BLUE,
                                                            )
                                                            .clicked()
                                                            {

                                                                let _ =
                                                                    self.cmd_tx.try_send(

                                                                        UiCommand::PlaceMarket {

                                                                            side: "sell".into(),

                                                                            token:
                                                                                order.token.clone(),

                                                                            usdc: None,

                                                                            shares:
                                                                                Some(
                                                                                    order.inline_sell_size.clone()
                                                                                ),

                                                                            order_type:
                                                                                Some(
                                                                                    order.inline_sell_market_type.clone()
                                                                                ),

                                                                            window_ts:
                                                                                window.timestamp_5m,
                                                                        }
                                                                    );
                                                            }
                                                        });
                                                    });
                                            }
                                        });

                                    ui.add_space(8.0);
                                }
                            });
                    }
                });

            ui.add_space(10.0);
        }

        windows_to_remove.sort_by(
            |a, b|
                b.cmp(a)
        );

        for idx in windows_to_remove {

            self.windows.remove(idx);
        }
    }
}