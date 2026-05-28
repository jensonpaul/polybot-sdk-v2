use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use futures::{FutureExt, StreamExt};
use rust_decimal::prelude::ToPrimitive;
use tokio::sync::Notify;
use tracing::{error, info, warn};

use polymarket_client_sdk_v2::clob::ws::Client as WsClient;
use polymarket_client_sdk_v2::gamma::Client as GammaClient;
use polymarket_client_sdk_v2::types::U256;

use crate::worker::get_or_fetch_token_ids;

#[derive(Debug, Clone)]
pub struct MarketPrices {
    pub up_price: f64,
    pub down_price: f64,

    pub up_asset_id: Arc<str>,
    pub down_asset_id: Arc<str>,

    pub connected: bool,
    pub stale: bool,

    pub last_ts: u64,

    pub error: Option<Arc<str>>,
}

pub type SharedMarketPrices =
    Arc<ArcSwap<MarketPrices>>;

#[derive(Clone, Debug)]
pub struct MarketFeedHandle {
    pub shutdown: Arc<Notify>,
}

pub async fn spawn_market_feed(
    slug: String,
    prices: SharedMarketPrices,
    shutdown: Arc<Notify>,
    gamma_client: GammaClient,
) {
    tokio::spawn(async move {
        info!(%slug, "market feed started");

        // -----------------------------------------------------
        // FETCH TOKEN IDS (retry only here, NOT stream layer)
        // -----------------------------------------------------
        let token_ids = loop {
            tokio::select! {
                _ = shutdown.notified() => {
                    info!(%slug, "market feed shutdown before init");
                    return;
                }
                res = get_or_fetch_token_ids(&gamma_client, &slug) => {
                    match res {
                        Ok(ids) if ids.len() >= 2 => break ids,
                        Ok(_) => {
                            error!(%slug, "invalid token ids");
                        }
                        Err(e) => {
                            error!(%slug, error=%e, "failed fetching token ids");
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        };

        let asset_ids: Vec<U256> = match token_ids
            .iter()
            .map(|id| U256::from_str(id))
            .collect()
        {
            Ok(v) => v,
            Err(e) => {
                error!(%slug, error=%e, "invalid asset conversion");
                return;
            }
        };

        let up_asset_id = Arc::<str>::from(token_ids[0].clone());
        let down_asset_id = Arc::<str>::from(token_ids[1].clone());

        // -----------------------------------------------------
        // WS STREAM (SDK owns lifecycle)
        // -----------------------------------------------------
        let ws = WsClient::default();

        let stream = match ws.subscribe_last_trade_price(asset_ids) {
            Ok(s) => s,
            Err(e) => {
                error!(%slug, error=%e, "ws subscribe failed");
                return;
            }
        };

        let mut stream = Box::pin(stream);

        // -----------------------------------------------------
        // INIT STATE
        // -----------------------------------------------------
        prices.store(Arc::new(MarketPrices {
            up_price: 0.0,
            down_price: 0.0,
            up_asset_id: up_asset_id.clone(),
            down_asset_id: down_asset_id.clone(),
            connected: true,
            stale: false,
            last_ts: 0,
            error: None,
        }));

        // -----------------------------------------------------
        // STALE DETECTION (TIME BASED ONLY)
        // -----------------------------------------------------
        let mut last_update = tokio::time::Instant::now();
        let stale_timeout = Duration::from_secs(5);

        loop {
            tokio::select! {
                // -------------------------
                // SHUTDOWN (CORRECT)
                // -------------------------
                _ = shutdown.notified() => {
                    info!(%slug, "market feed shutdown");
                    return;
                }

                // -------------------------
                // STREAM EVENT
                // -------------------------
                maybe_msg = stream.next() => {
                    let msg = match maybe_msg {
                        Some(Ok(msg)) => msg,

                        Some(Err(e)) => {
                            warn!(%slug, error=%e, "stream error (sdk handles recovery)");

                            let mut snap = prices.load().as_ref().clone();
                            snap.error = Some(Arc::<str>::from(e.to_string()));
                            snap.stale = true;

                            prices.store(Arc::new(snap));
                            continue;
                        }

                        None => {
                            warn!(%slug, "stream ended (sdk will likely reconnect internally)");

                            let mut snap = prices.load().as_ref().clone();
                            snap.stale = true;
                            snap.error = Some("stream ended".into());

                            prices.store(Arc::new(snap));
                            continue;
                        }
                    };

                    let ts = msg.timestamp as u64;
                    let price = msg.price.to_f64().unwrap_or(0.0);
                    let asset = msg.asset_id.to_string();

                    let mut snap = prices.load().as_ref().clone();

                    // ignore old updates
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
                }

                // -------------------------
                // STALE CHECK (NO RECONNECT)
                // -------------------------
                _ = tokio::time::sleep(Duration::from_millis(500)) => {
                    if last_update.elapsed() > stale_timeout {
                        let mut snap = prices.load().as_ref().clone();
                        snap.stale = true;
                        snap.connected = false; // logical connectivity, not socket state
                        prices.store(Arc::new(snap));
                    }
                }
            }
        }
    });
}