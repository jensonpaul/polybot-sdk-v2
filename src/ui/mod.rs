pub mod theme;
pub mod auth_gateway;
pub mod order_forms;
pub mod window_matrix;
pub mod check_interval;

use crate::worker_config::{SharedPollConfig, Queue};
use crate::ui_types::{NotificationKind, ToastNotification, UiCommand, WindowGroup, WorkerUpdate};
use crate::worker::{build_slug_for_timestamp, initiate_stamp_5m};
use eframe::egui;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{Receiver, Sender};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromStr;
use rust_decimal::RoundingStrategy;
use std::collections::HashMap;
use std::sync::{Arc};
use std::sync::atomic::AtomicU64;

pub struct IntervalInputs {
    pub orders: String,
    pub trades: String,
    pub rapid_sell: String,
}

impl IntervalInputs {
    fn get_mut(&mut self, queue: Queue) -> &mut String {
        match queue {
            Queue::Orders => &mut self.orders,
            Queue::Trades => &mut self.trades,
            Queue::RapidSell => &mut self.rapid_sell,
        }
    }

    fn get(&self, queue: Queue) -> &str {
        match queue {
            Queue::Orders => &self.orders,
            Queue::Trades => &self.trades,
            Queue::RapidSell => &self.rapid_sell,
        }
    }
}

pub struct PolymarketDashboardApp {
    pub bearer_token: String,
    pub is_authenticated: bool,
    pub auto_refresh_active: bool,
    
    pub interval_inputs: IntervalInputs,
    pub poll_config: SharedPollConfig,

    pub active_feed_window: Option<u64>,
    pub feed_started_for: std::collections::HashSet<u64>,

    // Limit Order State Variables
    pub limit_side_buy: bool,
    pub limit_token_up: bool,
    pub limit_price: String,
    pub limit_size: String,
    pub limit_rapid_price: String,

    // Market Order State Variables
    pub market_side_buy: bool,
    pub market_token_up: bool,
    pub market_usdc: String,
    pub market_shares: String,
    pub market_use_usdc: bool,
    pub market_type_fok: bool,

    // Live state tracking metrics containers
    pub windows: Vec<WindowGroup>,
    pub notifications: Vec<ToastNotification>,

    pub cmd_tx: Sender<UiCommand>,
    pub update_rx: Receiver<WorkerUpdate>,
}

impl PolymarketDashboardApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        cmd_tx: Sender<UiCommand>,
        update_rx: Receiver<WorkerUpdate>,
        poll_config: SharedPollConfig,
    ) -> Self {
        let test_tx = cmd_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let _ = test_tx
                .send(UiCommand::CheckStatus {
                    order_id: "INIT_PING".to_string(),
                    window_ts: 0,
                })
                .await;
        });

        let expected_token = std::env::var("API_BEARER_TOKEN").unwrap_or_default();
        Self {
            bearer_token: expected_token,
            is_authenticated: false,
            auto_refresh_active: true,
            interval_inputs: IntervalInputs {
                orders: poll_config.get(Queue::Orders).to_string(),
                trades: poll_config.get(Queue::Trades).to_string(),
                rapid_sell: poll_config.get(Queue::RapidSell).to_string(),
            },
            poll_config,
            active_feed_window: None,
            feed_started_for: std::collections::HashSet::new(),
            limit_side_buy: true,
            limit_token_up: true,
            limit_price: "0.50".to_string(),
            limit_size: "10".to_string(),
            limit_rapid_price: "0.00".to_string(),
            market_side_buy: true,
            market_token_up: true,
            market_usdc: "5.00".to_string(),
            market_shares: "0".to_string(),
            market_use_usdc: true,
            market_type_fok: true,
            windows: Vec::new(),
            notifications: Vec::new(),
            cmd_tx,
            update_rx,
        }
    }

    pub fn push_toast(&mut self, msg: String, kind: NotificationKind) {
        let duration = match kind {
            NotificationKind::Error => 8,
            _ => 4,
        };
        self.notifications.push(ToastNotification {
            message: msg,
            kind,
            expires_at: Instant::now() + Duration::from_secs(duration),
        });
    }
}

impl eframe::App for PolymarketDashboardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Drain incoming worker updates cleanly
        while let Ok(update) = self.update_rx.try_recv() {
            match update {
                WorkerUpdate::OrderAdded { window_ts, order } => {
                    if let Some(w) = self.windows.iter_mut().find(|w| w.timestamp_5m == window_ts) {
                        w.orders.push(order);
                    } else {
                        self.windows.push(WindowGroup {
                            timestamp_5m: window_ts,
                            slug: build_slug_for_timestamp(window_ts),
                            is_expanded: true,
                            orders: vec![order],

                            market_prices: None,
                            market_feed: None,
                        });
                    }
                }
                WorkerUpdate::OrderUpdated { window_ts, order_id, status, matched } => {
                    if let Some(w) = self.windows.iter_mut().find(|w| w.timestamp_5m == window_ts) {
                        if let Some(o) = w.orders.iter_mut().find(|o| o.id == order_id) {
                            o.status = status;
                            o.size_matched = matched.clone();
                            //o.inline_sell_size = matched;
                            // truncation to 2 decimals
                            o.inline_sell_size = round_to_two_dp(&matched);
                            //o.rapid_sell_size  = round_to_two_dp(&matched);
                        }
                    }
                }
                WorkerUpdate::Notify { message, kind } => {
                    self.push_toast(message, kind);
                }
                WorkerUpdate::MarketFeedStarted {
                    window_ts,
                    prices,
                } => {

                    if let Some(window) =
                        self.windows
                            .iter_mut()
                            .find(|w| w.timestamp_5m == window_ts)
                    {
                        window.market_prices =
                            Some(prices);
                    }
                }
            }
        }

        // 2. Clear old toast notification items
        self.notifications.retain(|n| Instant::now() < n.expires_at);

        // 3. Early render exit if auth token challenges fail
        if !self.is_authenticated {
            self.render_auth_gateway(ctx);
            return;
        }

        let current_ts = initiate_stamp_5m();
        let slug = build_slug_for_timestamp(current_ts);
        let current_time_raw = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let time_remaining = 300 - (current_time_raw % 300);

        // Ensure active lifecycle group exists safely
        if !self.windows.iter().any(|w| w.timestamp_5m == current_ts) {
            self.windows.insert(0, WindowGroup {
                timestamp_5m: current_ts,
                slug: slug.clone(),
                is_expanded: true,
                orders: Vec::new(),
                market_prices: None,
                market_feed: None,
            });
        }

        // SINGLE feed start guard
        if self.feed_started_for.insert(current_ts) {
            let _ = self.cmd_tx.try_send(UiCommand::StartMarketFeed {
                window_ts: current_ts,
                slug,
            });
        }

        // 4. Render Main Interface Panel Structures
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Polymarket Advanced Trading Node Client");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("🔄 Window Cycle Reset: {:02}:{:02}", time_remaining / 60, time_remaining % 60));
                });
            });
        });

        // Overlay notifications view
        if !self.notifications.is_empty() {
            egui::Window::new("System Broadcast Alerts")
                .anchor(egui::Align2::RIGHT_TOP, [-10.0, 40.0])
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    for toast in &self.notifications {
                        let color = match toast.kind {
                            NotificationKind::Success => egui::Color32::GREEN,
                            NotificationKind::Error => egui::Color32::LIGHT_RED,
                            NotificationKind::Warning => egui::Color32::YELLOW,
                            _ => egui::Color32::LIGHT_BLUE,
                        };
                        ui.colored_label(color, &toast.message);
                    }
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_order_consoles(ui, current_ts);
            ui.add_space(10.0);
            self.render_poll_interval_input(
                ui,
                "Orders Interval (ms):",
                Queue::Orders,
            );

            self.render_poll_interval_input(
                ui,
                "Trades Interval (ms):",
                Queue::Trades,
            );

            self.render_poll_interval_input(
                ui,
                "Rapid Sell Interval (ms):",
                Queue::RapidSell,
            );
            ui.add_space(10.0);
            self.render_lifecycle_matrix(ui);
        });

        if self.auto_refresh_active {
            //ctx.request_repaint_after(Duration::from_millis(500));
            ctx.request_repaint_after(
                Duration::from_millis(33)
            );
        }
    }
}

/// Rounds a numeric string to 2 decimal places using "ToZero" strategy.
/// If parsing fails, returns the original string.
fn round_to_two_dp(value: &str) -> String {
    Decimal::from_str(value)
        .map(|d| d.round_dp_with_strategy(2, RoundingStrategy::ToZero).to_string())
        .unwrap_or_else(|_| value.to_string())
}