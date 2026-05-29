//! # Market Data Feed
//!
//! Spawns one Tokio task per 5-minute window.  The task subscribes to the
//! Polymarket WebSocket and atomically updates `AppState::market_prices[window_ts]`
//! on every trade-price tick.
//!
//! The caller signals shutdown via `MarketFeedHandle::shutdown` (a
//! `tokio::sync::Notify`).

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use futures::StreamExt;
use tracing::{error, info, warn};

use polymarket_client_sdk_v2::clob::ws::Client as WsClient;
use polymarket_client_sdk_v2::gamma::Client as GammaClient;
use polymarket_client_sdk_v2::types::U256;
use rust_decimal::prelude::ToPrimitive;

use crate::state::{AppState, MarketFeedHandle, MarketPrices, SharedAppState, SharedMarketPrices};
use crate::worker::get_or_fetch_token_ids;

const STALE_TIMEOUT: Duration = Duration::from_secs(5);
const STALE_CHECK_INTERVAL: Duration = Duration::from_millis(500);

/// Create and register a live price feed for `window_ts`.
///
/// 1. Inserts a default (stale) `SharedMarketPrices` into `state.market_prices`.
/// 2. Spawns a Tokio task that connects to the WS feed and updates the price
///    snapshot atomically.
/// 3. Stores a `MarketFeedHandle` in `state.market_feeds` so the caller can
///    shut it down later.
///
/// This function is intentionally `async fn` — the caller should `.await` it
/// but it returns immediately after spawning (the task runs independently).
pub async fn start_market_feed(
    window_ts: u64,
    slug: String,
    state: SharedAppState,
    ctx: egui::Context,
) {
    // Initialise a stale price snapshot in shared state.
    let prices: SharedMarketPrices = Arc::new(ArcSwap::from_pointee(MarketPrices::default()));
    state.market_prices.insert(window_ts, prices.clone());

    let shutdown = Arc::new(tokio::sync::Notify::new());
    state.market_feeds.insert(
        window_ts,
        MarketFeedHandle {
            shutdown: shutdown.clone(),
        },
    );
    state.touch();
    ctx.request_repaint();

    tokio::spawn(async move {
        info!(%window_ts, %slug, "market feed task started");

        // ------------------------------------------------------------------
        // Fetch token IDs (with shutdown-aware retry)
        // ------------------------------------------------------------------
        let gamma = GammaClient::default();

        let token_ids = loop {
            tokio::select! {
                biased;
                _ = shutdown.notified() => {
                    info!(%window_ts, "market feed cancelled before init");
                    return;
                }
                res = get_or_fetch_token_ids(&gamma, &slug) => {
                    match res {
                        Ok(ids) if ids.len() >= 2 => break ids,
                        Ok(_) => error!(%slug, "token IDs count < 2"),
                        Err(e) => error!(%slug, error=%e, "failed to fetch token IDs"),
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        };

        let asset_ids: Vec<U256> = match token_ids.iter().map(|id| U256::from_str(id)).collect() {
            Ok(v) => v,
            Err(e) => {
                error!(%slug, error=%e, "asset ID conversion failed");
                return;
            }
        };

        let up_asset_id = Arc::<str>::from(token_ids[0].as_str());
        let down_asset_id = Arc::<str>::from(token_ids[1].as_str());

        // Mark as connected with real asset IDs.
        prices.store(Arc::new(MarketPrices {
            up_asset_id: up_asset_id.clone(),
            down_asset_id: down_asset_id.clone(),
            connected: true,
            stale: false,
            ..Default::default()
        }));
        state.touch();
        ctx.request_repaint();

        // ------------------------------------------------------------------
        // Subscribe to WebSocket feed
        // ------------------------------------------------------------------
        let ws = WsClient::default();
        let stream = match ws.subscribe_last_trade_price(asset_ids) {
            Ok(s) => s,
            Err(e) => {
                error!(%slug, error=%e, "WS subscribe failed");
                return;
            }
        };
        let mut stream = Box::pin(stream);
        let mut last_update = tokio::time::Instant::now();

        // ------------------------------------------------------------------
        // Event loop
        // ------------------------------------------------------------------
        loop {
            tokio::select! {
                biased;

                _ = shutdown.notified() => {
                    info!(%window_ts, "market feed shut down");
                    // Clean up shared state so the UI doesn't show a stale
                    // price widget for a dead feed.
                    state.market_prices.remove(&window_ts);
                    state.touch();
                    ctx.request_repaint();
                    return;
                }

                maybe_msg = stream.next() => {
                    match maybe_msg {
                        Some(Ok(msg)) => {
                            let ts = msg.timestamp as u64;
                            let price = msg.price.to_f64().unwrap_or(0.0);
                            let asset = msg.asset_id.to_string();

                            let mut snap = prices.load().as_ref().clone();
                            if ts <= snap.last_ts {
                                continue;
                            }

                            snap.last_ts = ts;
                            snap.connected = true;
                            snap.stale = false;
                            snap.error = None;

                            if asset == snap.up_asset_id.as_ref() {
                                snap.up_price = price;
                            } else if asset == snap.down_asset_id.as_ref() {
                                snap.down_price = price;
                            }

                            prices.store(Arc::new(snap));
                            last_update = tokio::time::Instant::now();
                            state.touch();
                            ctx.request_repaint();
                        }

                        Some(Err(e)) => {
                            warn!(%slug, error=%e, "stream error (SDK may reconnect)");
                            let mut snap = prices.load().as_ref().clone();
                            snap.stale = true;
                            snap.error = Some(Arc::from(e.to_string().as_str()));
                            prices.store(Arc::new(snap));
                            state.touch();
                            ctx.request_repaint();
                        }

                        None => {
                            warn!(%slug, "stream ended unexpectedly");
                            let mut snap = prices.load().as_ref().clone();
                            snap.stale = true;
                            snap.error = Some(Arc::from("stream ended"));
                            prices.store(Arc::new(snap));
                            state.touch();
                            ctx.request_repaint();
                        }
                    }
                }

                _ = tokio::time::sleep(STALE_CHECK_INTERVAL) => {
                    if last_update.elapsed() > STALE_TIMEOUT {
                        let mut snap = prices.load().as_ref().clone();
                        if !snap.stale {
                            snap.stale = true;
                            snap.connected = false;
                            prices.store(Arc::new(snap));
                            state.touch();
                            ctx.request_repaint();
                        }
                    }
                }
            }
        }
    });
}
