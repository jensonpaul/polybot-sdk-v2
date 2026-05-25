use crate::ui::PolymarketDashboardApp;
use crate::ui_types::{LocalOrderStatus, UiCommand};
use eframe::egui;
use crate::ui::theme::Theme;

impl PolymarketDashboardApp {
    pub fn render_lifecycle_matrix(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("📊 Windowed Lifecycle Management Matrix Logs");
            ui.checkbox(&mut self.auto_refresh_active, "Keep-Alive Heartbeat Live Polling Worker State Sync");
        });
        ui.separator();

        let mut windows_to_remove = Vec::new();

        for (w_idx, window) in self.windows.iter_mut().enumerate() {
            let header_title = format!(
                "⏱️ Window Block Frame [ID: {}] | Slug: {} (Active Total Tracking: {})",
                window.timestamp_5m, window.slug, window.orders.len()
            );

            ui.horizontal(|ui| {
                if ui.button(if window.is_expanded { "🔽" } else { "▶" }).clicked() {
                    window.is_expanded = !window.is_expanded;
                }
                ui.colored_label(
                    Theme::TEXT_PRIMARY,

                    egui::RichText::new(&header_title)
                        .strong()
                        .size(15.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("❌ Close Frame").clicked() {
                        windows_to_remove.push(w_idx);
                        /*
                        let _ = self.cmd_tx.try_send(
                            UiCommand::StopMarketFeed {
                                window_ts: window.timestamp_5m,
                            }
                        );
                        */
                    }
                    if ui.button("🛑 Cancel Window Orders").clicked() {
                        let _ = self.cmd_tx.try_send(UiCommand::CancelAllInWindow {
                            window_ts: window.timestamp_5m,
                        });
                    }
                });
            });

            if let Some(prices) = &window.market_prices {
                let snap = prices.load();

                ui.horizontal(|ui| {

                    egui::Frame::none()
                        .fill(Theme::BUY_GREEN_BG)
                        .stroke(
                            egui::Stroke::new(
                                1.0,
                                Theme::BUY_GREEN
                            )
                        )
                        .corner_radius(6.0)
                        .inner_margin(10.0)
                        .show(ui, |ui| {

                            ui.label(
                                egui::RichText::new(
                                    format!(
                                        "▲ UP {:.3}",
                                        snap.up_price
                                    )
                                )
                                .strong()
                                .color(Theme::BUY_GREEN)
                            );
                        });

                    egui::Frame::none()
                        .fill(Theme::SELL_RED_BG)
                        .stroke(
                            egui::Stroke::new(
                                1.0,
                                Theme::SELL_RED
                            )
                        )
                        .corner_radius(6.0)
                        .inner_margin(10.0)
                        .show(ui, |ui| {

                            ui.label(
                                egui::RichText::new(
                                    format!(
                                        "▼ DOWN {:.3}",
                                        snap.down_price
                                    )
                                )
                                .strong()
                                .color(Theme::SELL_RED)
                            );
                        });

                    if snap.stale {
                        ui.colored_label(
                            Theme::WARNING,
                            "● STALE"
                        );
                    }

                    if !snap.connected {
                        ui.colored_label(
                            Theme::SELL_RED,
                            "● DISCONNECTED"
                        );
                    }
                });
            }

            if window.is_expanded {
                ui.indent(format!("window_indent_{}", window.timestamp_5m), |ui| {
                    if window.orders.is_empty() {
                        ui.label("No structural logs found inside this localized window iteration frame container.");
                    } else {
                        for order in window.orders.iter_mut() {
                            let (fill, border) = match &order.status {

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
                                    egui::Color32::from_rgba_unmultiplied(
                                        180,
                                        40,
                                        40,
                                        40,
                                    ),

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

                            egui::Frame::none()
                                .fill(fill)
                                .stroke(
                                    egui::Stroke::new(
                                        1.0,
                                        border,
                                    )
                                )
                                .corner_radius(4.0)
                                .inner_margin(10.0)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(
                                                format!(
                                                    "[ID: {}] Side: {} | Token: {} | Price: {} | Size: {}",
                                                    order.id, 
                                                    order.side, 
                                                    order.token, 
                                                    order.price, 
                                                    order.size
                                                )
                                            )
                                            .monospace()
                                            .color(Theme::TEXT_PRIMARY)
                                            .strong()
                                        );
                                        //ui.separator();
                                        ui.add_space(10.0);
                                        ui.label(
                                            egui::RichText::new(
                                                format!(
                                                    "Matched: {}", 
                                                    order.size_matched
                                                )
                                            )
                                            .monospace()
                                            .color(Theme::TEXT_PRIMARY)
                                            .strong()
                                        );

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button("🗑️ Cancel").clicked() {
                                                let _ = self.cmd_tx.try_send(UiCommand::CancelIndividual {
                                                    order_id: order.id.clone(),
                                                    window_ts: window.timestamp_5m,
                                                });
                                            }
                                            if ui.button("🔍 Sync").clicked() {
                                                let _ = self.cmd_tx.try_send(UiCommand::CheckStatus {
                                                    order_id: order.id.clone(),
                                                    window_ts: window.timestamp_5m,
                                                });
                                            }
                                        });
                                    });

                                    if order.side.to_lowercase() == "buy" && order.size_matched != "0" {
                                        ui.horizontal(|ui| {
                                            ui.label(

                                                egui::RichText::new(
                                                    "⚡ Inline Position Mitigation Desk"
                                                )
                                                .color(Theme::BLUE)
                                            );

                                            // --- Limit Sell Section ---
                                            ui.small("Price:");
                                            ui.add(egui::TextEdit::singleline(&mut order.inline_sell_price).desired_width(50.0));
                                            ui.small("Size:");
                                            ui.add(egui::TextEdit::singleline(&mut order.inline_sell_size).desired_width(50.0));
                                            if ui.small_button("Instant Counter Limit Sell").clicked() {
                                                let _ = self.cmd_tx.try_send(UiCommand::PlaceLimit {
                                                    side: "sell".into(),
                                                    token: order.token.clone(),
                                                    price: order.inline_sell_price.clone(),
                                                    size: order.inline_sell_size.clone(),
                                                    rapid_price: "0".to_string(),
                                                    window_ts: window.timestamp_5m,
                                                });
                                            }

                                            // --- Market Sell Section ---
                                            ui.separator(); // visual separation
                                            ui.small("Market Size:");
                                            ui.add(egui::TextEdit::singleline(&mut order.inline_sell_size).desired_width(50.0));

                                            // Order Type Selector
                                            ui.radio_value(&mut order.inline_sell_market_type, "FAK".to_string(), "FAK");
                                            ui.radio_value(&mut order.inline_sell_market_type, "FOK".to_string(), "FOK");

                                            if ui.small_button("Instant Counter Market Sell").clicked() {
                                                let _ = self.cmd_tx.try_send(UiCommand::PlaceMarket {
                                                    side: "sell".into(),
                                                    token: order.token.clone(),
                                                    usdc: None,
                                                    shares: Some(order.inline_sell_size.clone()),
                                                    order_type: Some(order.inline_sell_market_type.clone()),
                                                    window_ts: window.timestamp_5m,
                                                });
                                            }
                                        });
                                    }
                                });
                            ui.add_space(6.0);
                        }
                    }
                });
            }
            ui.separator();
        }

        // Sort indexes backwards to prevent array out-of-bounds panics on removals
        windows_to_remove.sort_by(|a, b| b.cmp(a));
        for idx in windows_to_remove {
            self.windows.remove(idx);
        }
    }
}