use crate::ui::theme::Theme;

use crate::ui::widgets::{
    compact_input,
    panel_frame,
    themed_button,
};

use crate::ui::PolymarketDashboardApp;

use crate::ui_types::{
    LocalOrderStatus,
    TrackedOrder,
    UiCommand,
    WindowGroup,
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

                        ui.horizontal_wrapped(|ui| {

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

                        ui.add_space(12.0);

                        // =====================================================
                        // SPLIT ORDERS
                        // =====================================================

                        let mut bought =
                            Vec::<usize>::new();

                        let mut sold =
                            Vec::<usize>::new();

                        let mut others =
                            Vec::<usize>::new();

                        for (
                            idx,
                            order
                        ) in window.orders
                            .iter()
                            .enumerate()
                        {

                            let side =
                                order.side
                                    .to_lowercase();

                            let invalid_status =
                                matches!(
                                    order.status,

                                    LocalOrderStatus::Canceled
                                    | LocalOrderStatus::Failed(_)
                                );

                            if invalid_status {

                                others.push(idx);

                                continue;
                            }

                            match side.as_str() {

                                "buy" =>
                                    bought.push(idx),

                                "sell" =>
                                    sold.push(idx),

                                _ =>
                                    others.push(idx),
                            }
                        }

                        // =====================================================
                        // RESPONSIVE 3 COLUMN LAYOUT
                        // =====================================================

                        ui.columns(3, |columns| {

                            // =====================================================
                            // BOUGHT
                            // =====================================================

                            Self::render_order_column(

                                &mut columns[0],

                                "🟢 BOUGHT",

                                &bought,

                                window,

                                &self.cmd_tx,
                            );

                            // =====================================================
                            // SOLD
                            // =====================================================

                            Self::render_order_column(

                                &mut columns[1],

                                "🔵 SOLD",

                                &sold,

                                window,

                                &self.cmd_tx,
                            );

                            // =====================================================
                            // OTHERS
                            // =====================================================

                            Self::render_order_column(

                                &mut columns[2],

                                "⚫ OTHERS",

                                &others,

                                window,

                                &self.cmd_tx,
                            );
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

    fn render_order_column(
        ui: &mut egui::Ui,
        title: &str,
        indices: &[usize],
        window: &mut WindowGroup,
        cmd_tx: &tokio::sync::mpsc::Sender<UiCommand>,
    ) {

        panel_frame()
            .show(ui, |ui| {

                ui.horizontal(|ui| {

                    ui.heading(

                        egui::RichText::new(title)
                            .color(
                                Theme::TEXT_PRIMARY
                            )
                    );

                    ui.separator();

                    ui.label(

                        egui::RichText::new(
                            indices.len().to_string()
                        )
                        .color(
                            Theme::TEXT_MUTED
                        )
                    );
                });

                ui.add_space(8.0);

                egui::ScrollArea::vertical()

                    .id_salt(format!(
                        "{}_scroll_{}",
                        title,
                        window.timestamp_5m
                    ))

                    .max_height(550.0)

                    .auto_shrink([false; 2])

                    .show(ui, |ui| {

                        for idx in indices {

                            if let Some(order) =
                                window.orders.get_mut(*idx)
                            {

                                Self::render_order_card(
                                    ui,
                                    order,
                                    window.timestamp_5m,
                                    cmd_tx,
                                );

                                ui.add_space(8.0);
                            }
                        }
                    });
            });
    }

    fn render_order_card(
        ui: &mut egui::Ui,
        order: &mut TrackedOrder,
        window_ts: u64,
        cmd_tx: &tokio::sync::mpsc::Sender<UiCommand>,
    ) {

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

                let display_price =
                    order.executed_price
                        .as_deref()
                        .unwrap_or(&order.price);

                let display_size =
                    order.executed_size
                        .as_deref()
                        .unwrap_or(&order.size);

                // =====================================================
                // HEADER
                // =====================================================

                ui.horizontal_wrapped(|ui| {

                    ui.label(

                        egui::RichText::new(

                            format!(
                                "{} {} @ {} x {}",
                                order.side.to_uppercase(),
                                order.token.to_uppercase(),
                                display_price,
                                display_size,
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

                    if matches!(
                        order.status,
                        LocalOrderStatus::Open
                        | LocalOrderStatus::PartiallyFilled { .. }
                    ) {

                        if themed_button(
                            ui,
                            "CANCEL",
                            Theme::SELL_RED_BG,
                            Theme::SELL_RED,
                        )
                        .clicked()
                        {

                            let _ =
                                cmd_tx.try_send(

                                    UiCommand::CancelIndividual {

                                        order_id:
                                            order.id.clone(),

                                        window_ts,
                                    }
                                );
                        }
                    }
                });

                // =====================================================
                // STATUS
                // =====================================================

                ui.add_space(4.0);

                ui.label(

                    egui::RichText::new(
                        format!("{:?}", order.status)
                    )
                    .monospace()
                    .color(
                        Theme::TEXT_MUTED
                    )
                );

                // =====================================================
                // INLINE EXIT DESK
                // =====================================================

                if matches!(
                    order.status,
                    LocalOrderStatus::FullyFilled
                    | LocalOrderStatus::PartiallyFilled { .. }
                )
                && order.side.eq_ignore_ascii_case("buy")
                {

                    ui.add_space(8.0);

                    panel_frame()

                        .fill(
                            Theme::BG_PANEL
                        )

                        .show(ui, |ui| {

                            ui.vertical(|ui| {

                                ui.horizontal_wrapped(|ui| {

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
                                });

                                ui.add_space(6.0);

                                ui.horizontal_wrapped(|ui| {

                                    if themed_button(
                                        ui,
                                        "LIMIT EXIT",
                                        Theme::SELL_RED_BG,
                                        Theme::SELL_RED,
                                    )
                                    .clicked()
                                    {

                                        let _ =
                                            cmd_tx.try_send(

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

                                                    window_ts,
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
                                            cmd_tx.try_send(

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
                                                        Some("FAK".to_string()),

                                                    window_ts,
                                                }
                                            );
                                    }
                                });
                            });
                        });
                }
            });
    }
}