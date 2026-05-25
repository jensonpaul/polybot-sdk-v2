use std::str::FromStr as _;
use std::sync::Arc;
use std::time::Duration;

use rust_decimal::prelude::ToPrimitive;

use arc_swap::ArcSwap;
use futures::{FutureExt, StreamExt};
use tokio::sync::Notify;
use tracing::{error, info, warn};

use polybot_sdk_v2::clob::ws::Client as WsClient;
use polybot_sdk_v2::types::U256;

use crate::worker::get_or_fetch_token_ids;
use polybot_sdk_v2::gamma::Client as GammaClient;

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
        let mut reconnect_backoff_ms = 500u64;

        loop {
            // ---------------------------------------------------------
            // SHUTDOWN CHECK
            // ---------------------------------------------------------
            if shutdown.notified().now_or_never().is_some() {
                info!("market feed shutdown before connect");
                return;
            }

            // ---------------------------------------------------------
            // FETCH TOKEN IDS
            // ---------------------------------------------------------
            let token_ids = match get_or_fetch_token_ids(
                &gamma_client,
                &slug,
            )
            .await
            {
                Ok(ids) if ids.len() >= 2 => ids,

                Ok(ids) => {
                    error!(
                        "invalid token ids count: {}",
                        ids.len()
                    );

                    tokio::time::sleep(
                        Duration::from_secs(2)
                    )
                    .await;

                    continue;
                }

                Err(e) => {
                    error!(
                        "failed fetching token ids: {}",
                        e
                    );

                    tokio::time::sleep(
                        Duration::from_secs(2)
                    )
                    .await;

                    continue;
                }
            };

            let up_asset_id =
                Arc::<str>::from(token_ids[0].clone());

            let down_asset_id =
                Arc::<str>::from(token_ids[1].clone());

            // ---------------------------------------------------------
            // CREATE WS CLIENT
            // ---------------------------------------------------------
            let ws_client = WsClient::default();

            let asset_ids = match token_ids
                .iter()
                .map(|id| U256::from_str(id))
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(v) => v,

                Err(e) => {
                    error!(
                        "u256 parse failure: {}",
                        e
                    );

                    tokio::time::sleep(
                        Duration::from_secs(2)
                    )
                    .await;

                    continue;
                }
            };

            info!(
                "subscribing last_trade_price: {:?}",
                token_ids
            );

            // ---------------------------------------------------------
            // SUBSCRIBE
            // ---------------------------------------------------------
            let stream_result =
                ws_client.subscribe_last_trade_price(
                    asset_ids.clone(),
                );

            let stream = match stream_result {
                Ok(stream) => stream,

                Err(e) => {
                    error!(
                        "subscription failed: {}",
                        e
                    );

                    tokio::time::sleep(
                        Duration::from_millis(
                            reconnect_backoff_ms
                        ),
                    )
                    .await;

                    reconnect_backoff_ms =
                        (reconnect_backoff_ms * 2)
                            .min(10_000);

                    continue;
                }
            };

            reconnect_backoff_ms = 500;

            let mut stream = Box::pin(stream);

            // ---------------------------------------------------------
            // LOCAL SNAPSHOT
            // ---------------------------------------------------------
            let mut local_snapshot = prices
                .load()
                .as_ref()
                .clone();

            local_snapshot.up_asset_id = up_asset_id.clone();
            local_snapshot.down_asset_id = down_asset_id.clone();

            local_snapshot.connected = true;
            local_snapshot.stale = false;
            local_snapshot.error = None;

            prices.store(
                Arc::new(local_snapshot.clone())
            );

            // ---------------------------------------------------------
            // STREAM LOOP
            // ---------------------------------------------------------
            loop {
                tokio::select! {

                    // -------------------------------------------------
                    // GRACEFUL SHUTDOWN
                    // -------------------------------------------------
                    _ = shutdown.notified() => {

                        info!("market feed shutdown");

                        let _ = ws_client
                            .unsubscribe_orderbook(
                                &asset_ids
                            );

                        drop(stream);

                        return;
                    }

                    // -------------------------------------------------
                    // WS EVENT
                    // -------------------------------------------------
                    result = tokio::time::timeout(
                        Duration::from_secs(45),
                        stream.next()
                    ) => {

                        match result {

                            // -----------------------------------------
                            // MESSAGE RECEIVED
                            // -----------------------------------------
                            Ok(Some(Ok(trade))) => {

                                let ts =
                                    trade.timestamp.max(0) as u64;

                                // stale protection
                                if ts < local_snapshot.last_ts {
                                    continue;
                                }

                                let price =
                                    trade.price
                                        .to_f64()
                                        .unwrap_or(0.0);

                                local_snapshot.last_ts = ts;
                                local_snapshot.connected = true;
                                local_snapshot.stale = false;
                                local_snapshot.error = None;

                                let asset_id =
                                    trade.asset_id.to_string();

                                if asset_id
                                    == local_snapshot.up_asset_id.as_ref()
                                {
                                    local_snapshot.up_price =
                                        price;
                                }
                                else if asset_id
                                    == local_snapshot.down_asset_id.as_ref()
                                {
                                    local_snapshot.down_price =
                                        price;
                                }

                                prices.store(
                                    Arc::new(
                                        local_snapshot.clone()
                                    )
                                );
                            }

                            // -----------------------------------------
                            // STREAM ERROR
                            // -----------------------------------------
                            Ok(Some(Err(e))) => {

                                error!(
                                    step = 1,
                                    error = %e
                                );

                                local_snapshot.connected =
                                    false;

                                local_snapshot.stale = true;

                                local_snapshot.error =
                                    Some(
                                        Arc::<str>::from(
                                            e.to_string()
                                        )
                                    );

                                prices.store(
                                    Arc::new(
                                        local_snapshot.clone()
                                    )
                                );

                                let _ = ws_client
                                    .unsubscribe_orderbook(
                                        &asset_ids
                                    );

                                drop(stream);

                                break;
                            }

                            // -----------------------------------------
                            // STREAM ENDED
                            // -----------------------------------------
                            Ok(None) => {

                                error!(
                                    step = 1,
                                    "stream ended"
                                );

                                local_snapshot.connected =
                                    false;

                                local_snapshot.stale = true;

                                prices.store(
                                    Arc::new(
                                        local_snapshot.clone()
                                    )
                                );

                                let _ = ws_client
                                    .unsubscribe_orderbook(
                                        &asset_ids
                                    );

                                drop(stream);

                                break;
                            }

                            // -----------------------------------------
                            // TIMEOUT
                            // -----------------------------------------
                            Err(_) => {

                                warn!(
                                    step = 1,
                                    "timeout"
                                );

                                local_snapshot.connected =
                                    false;

                                local_snapshot.stale = true;

                                prices.store(
                                    Arc::new(
                                        local_snapshot.clone()
                                    )
                                );

                                let _ = ws_client
                                    .unsubscribe_orderbook(
                                        &asset_ids
                                    );

                                drop(stream);

                                break;
                            }
                        }
                    }
                }
            }

            // ---------------------------------------------------------
            // EXPONENTIAL BACKOFF
            // ---------------------------------------------------------
            tokio::time::sleep(
                Duration::from_millis(
                    reconnect_backoff_ms
                ),
            )
            .await;

            reconnect_backoff_ms =
                (reconnect_backoff_ms * 2)
                    .min(10_000);
        }
    });
}