use crate::ui::PolymarketDashboardApp;
use crate::ui_types::{LocalOrderStatus, UiCommand};
use eframe::egui;

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
                ui.label(&header_title);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("❌ Close Frame").clicked() {
                        windows_to_remove.push(w_idx);
                    }
                    if ui.button("🛑 Cancel Window Orders").clicked() {
                        let _ = self.cmd_tx.try_send(UiCommand::CancelAllInWindow {
                            window_ts: window.timestamp_5m,
                        });
                    }
                });
            });

            if window.is_expanded {
                ui.indent(format!("window_indent_{}", window.timestamp_5m), |ui| {
                    if window.orders.is_empty() {
                        ui.label("No structural logs found inside this localized window iteration frame container.");
                    } else {
                        for order in window.orders.iter_mut() {
                            let frame_color = match &order.status {
                                LocalOrderStatus::FullyFilled => egui::Color32::from_rgba_unmultiplied(20, 120, 20, 30),
                                LocalOrderStatus::PartiallyFilled { .. } => egui::Color32::from_rgba_unmultiplied(140, 130, 10, 30),
                                LocalOrderStatus::Canceled => egui::Color32::from_rgba_unmultiplied(70, 70, 70, 30),
                                LocalOrderStatus::Failed(_) => egui::Color32::from_rgba_unmultiplied(120, 20, 20, 30),
                                _ => egui::Color32::from_rgba_unmultiplied(30, 30, 30, 30),
                            };

                            egui::Frame::none()
                                .fill(frame_color)
                                .corner_radius(4.0)
                                .inner_margin(6.0)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(format!(
                                            "[ID: {}] Side: {} | Token: {} | Price: {} | Size: {}",
                                            order.id, order.side, order.token, order.price, order.size
                                        ));
                                        ui.separator();
                                        ui.label(format!("Matched Fill Size Indicator: {}", order.size_matched));

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
                                            ui.small("⚡ Inline Position Mitigation Desk:");
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
                                                    window_ts: window.timestamp_5m,
                                                });
                                            }
                                        });
                                    }
                                });
                            ui.add_space(2.0);
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