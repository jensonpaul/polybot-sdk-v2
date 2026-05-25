use crate::ui_types::{
    RapidSellState, LocalOrderStatus, NotificationKind, OrderLimitRequest, OrderMarketRequest, TrackedOrder, UiCommand, WorkerUpdate,
};
use eframe::egui;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc::{Receiver, Sender};
use std::sync::atomic::{AtomicU64, Ordering};

use rust_decimal_macros::dec;

use crate::worker_config::{SharedPollConfig, Queue};

// START
// SDK dependencies
use std::collections::HashMap;
use std::str::FromStr as _;
use std::time::{SystemTime, UNIX_EPOCH, Instant};

use alloy::signers::Signer as _;
use alloy::signers::local::LocalSigner;

use lazy_static::lazy_static;

use polybot_sdk_v2::auth::state::Authenticated;
use polybot_sdk_v2::auth::{Credentials, Normal};
use polybot_sdk_v2::clob::{Client as ClobClient, Config};
use polybot_sdk_v2::clob::types::{Side, SignatureType, OrderType, OrderStatusType, TradeStatusType, Amount, TickSize};
use polybot_sdk_v2::clob::types::request::{TradesRequest};
use polybot_sdk_v2::clob::types::response::{PostOrderResponse, OpenOrderResponse, CancelOrdersResponse, TradeResponse};
use polybot_sdk_v2::gamma::Client as GammaClient;
use polybot_sdk_v2::gamma::types::request::MarketBySlugRequest;
use polybot_sdk_v2::gamma::types::response::Market;
use polybot_sdk_v2::types::{Address, Decimal, U256};

use serde::Deserialize;
use tracing::{info, instrument};

pub use polybot_sdk_v2::error::Error;

const MAX_RAPID_RETRIES: u32 = 5;

const BASE_RETRY_DELAY_SECS: u64 = 2;

const MAX_RETRY_DELAY_SECS: u64 = 60;

lazy_static! {
    static ref MARKET_BY_SLUG_CACHE:
        std::sync::Mutex<HashMap<String, Market>>
        = std::sync::Mutex::new(HashMap::new());

    static ref API_CREDS_CACHE:
        std::sync::Mutex<HashMap<String, Credentials>>
        = std::sync::Mutex::new(HashMap::new());
}
// END

pub fn is_permanent_rapid_sell_error(
    err: &str,
) -> bool {

    let err_lower = err.to_lowercase();

    err_lower.contains("not enough balance")
        || err_lower.contains("allowance")
        || err_lower.contains("invalid order")
        || err_lower.contains("no market price")
        || err_lower.contains("order amount")
}

pub fn compute_retry_delay_secs(
    retry_count: u32,
) -> u64 {

    let exponential =
        BASE_RETRY_DELAY_SECS
            .saturating_mul(
                2u64.saturating_pow(retry_count)
            );

    exponential.min(MAX_RETRY_DELAY_SECS)
}

pub struct PolymarketWorker {
    pub cmd_rx: Receiver<UiCommand>,
    pub update_tx: Sender<WorkerUpdate>,
    pub ctx: egui::Context,
    pub clob_client: Arc<Mutex<Option<ClobClient<Authenticated<Normal>>>>>, // persistent client
    pub poll_config: SharedPollConfig,
}

impl PolymarketWorker {
    pub async fn init_clob_client(&self) -> anyhow::Result<ClobClient<Authenticated<Normal>>> {
        let private_key = std::env::var("PRIVATE_KEY_VAR")?;
        let host = std::env::var("CLOB_API_URL").unwrap_or_else(|_| "https://clob.polymarket.com".into());
        let deposit_wallet = Address::from_str(&std::env::var("DEPOSIT_WALLET")?)?;
        
        let signer = LocalSigner::from_str(&private_key)?.with_chain_id(Some(polybot_sdk_v2::POLYGON));
        let creds = get_or_fetch_api_creds(private_key.clone(), host.clone()).await?;
        
        let client = ClobClient::new(&host, Config::default())?
            .authentication_builder(&signer)
            .funder(deposit_wallet)
            .signature_type(SignatureType::Poly1271)
            .credentials(creds)
            .authenticate()
            .await?;

        Ok(client)
    }

    /*
    /// Spawns a Tokio task with a locked `clob_client` instance.
    pub fn spawn_with_client<F, Fut>(&self, f: F)
    where
        F: FnOnce(Client<polybot_sdk_v2::auth::state::Authenticated<polybot_sdk_v2::auth::Normal>>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let clob_client = self.clob_client.clone();

        tokio::spawn(async move {
            //let mut client_guard = clob_client.lock().await;
            //let client = client_guard.as_mut().ok_or_else(|| anyhow::anyhow!("CLOB client not initialized"))?;
            let client_guard = clob_client.lock().await;
            let client = match &*client_guard {
                Some(c) => c.clone(),
                None => {
                    tracing::error!("ClobClient not initialized");
                    return;
                }
            };

            f(client).await;
        });
    }
    */

    pub async fn run(mut self) -> anyhow::Result<()> {
        tracing::info!("PolymarketWorker: running execution loop");

        let orders_polling_interval_ms =
            self.poll_config.get_atomic(Queue::Orders);

        let trades_polling_interval_ms =
            self.poll_config.get_atomic(Queue::Trades);

        let rapid_sell_polling_interval_ms =
            self.poll_config.get_atomic(Queue::RapidSell);

        let orders_to_poll = Arc::new(Mutex::new(Vec::<(u64, String)>::new()));
        let tracked_orders = Arc::new(Mutex::new(HashMap::<String, TrackedOrder>::new()));
        let tracked_trades = Arc::new(
            Mutex::new(HashMap::<String, TradeResponse>::new())
        );

        // Initialize the client
        let client = self.init_clob_client().await?;
        let clob_client = Arc::new(tokio::sync::Mutex::new(Some(client)));

        // -------------------------------------------------------------
        // TASK 1: Isolated Heartbeat Orders Polling Loop
        // -------------------------------------------------------------
        let poll_orders = Arc::clone(&orders_to_poll);
        let tracked_orders_for_polling = tracked_orders.clone();
        let tracked_trades_for_polling_clone = tracked_trades.clone();
        let poll_tx = self.update_tx.clone();
        let poll_ctx = self.ctx.clone();

        let clob_client_for_tracking = clob_client.clone(); // clone the Arc for this task

        // Clone for orders polling task
        let orders_interval_clone =
            orders_polling_interval_ms.clone();

        tokio::spawn(async move {
            let mut current_interval =
                orders_interval_clone.load(Ordering::Relaxed);

            let mut interval = tokio::time::interval(
                Duration::from_millis(
                    current_interval.max(1) // prevent panic
                ),
            );

            loop {
                let latest_interval =
                    orders_interval_clone.load(Ordering::Relaxed);

                // Detect interval changes
                if latest_interval != current_interval {
                    current_interval = latest_interval;

                    // Only rebuild timer if enabled
                    if current_interval > 0 {
                        interval = tokio::time::interval(
                            Duration::from_millis(current_interval),
                        );

                        tracing::info!(
                            "Orders Polling interval updated to {} ms",
                            current_interval
                        );
                    } else {
                        tracing::warn!(
                            "Orders polling disabled"
                        );
                    }
                }

                // ------------------------------------
                // ORDERS POLLING DISABLED
                // ------------------------------------
                if current_interval == 0 {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    continue;
                }

                interval.tick().await;

                let active_tracked_orders: Vec<(u64, String)> = {
                    let lock = poll_orders.lock().await;
                    lock.clone()
                };

                if active_tracked_orders.is_empty() {
                    continue;
                }

                let mut active_removals = Vec::new();

                for (window_ts, order_id) in active_tracked_orders {
                    // Assuming get_order_status now returns anyhow::Result<OpenOrderResponse>
                    if let Ok(order_info) = get_order_status(clob_client_for_tracking.clone(), &order_id).await {
                        
                        // 1. Calculate matching execution milestones
                        //let is_fully_filled = order_info.size_matched >= order_info.original_size;
                        let tolerance_pct = dec!(0.005); // 0.5%
                        let is_fully_filled =
                            order_info.size_matched >= order_info.original_size * (dec!(1.0) - tolerance_pct);
                        let matched_string = format!("{}/{}", order_info.size_matched, order_info.original_size);

                        let mut is_trade_fully_confirmed = false;

                        {
                            let trades_lock = tracked_trades_for_polling_clone.lock().await;

                            if !order_info.associate_trades.is_empty() {

                                let confirmed_size: Decimal =
                                    order_info
                                        .associate_trades
                                        .iter()
                                        .filter_map(|trade_id| {
                                            trades_lock.get(trade_id)
                                        })
                                        .filter(|trade| {
                                            matches!(
                                                trade.status,
                                                TradeStatusType::Confirmed
                                            )
                                        })
                                        .map(|trade| trade.size)
                                        .sum();

                                let tolerance_pct = dec!(0.005);

                                is_trade_fully_confirmed =
                                    confirmed_size >=
                                    order_info.original_size
                                        * (dec!(1.0) - tolerance_pct);
                            }
                        }

                        // 2. Map target internal statuses based on our new OrderStatusType enum variations
                        let target_status = match order_info.status {
                            OrderStatusType::Live => {
                                if order_info.size_matched > rust_decimal::Decimal::ZERO {
                                    LocalOrderStatus::PartiallyFilled {
                                        filled: order_info.size_matched.to_string(),
                                    }
                                } else {
                                    LocalOrderStatus::Open
                                }
                            }
                            OrderStatusType::Matched => {

                                // fully matched by engine
                                if is_fully_filled {

                                    // trades finalized
                                    if is_trade_fully_confirmed {

                                        active_removals.push(order_id.clone());

                                        LocalOrderStatus::TradeConfirmed
                                    } else {

                                        //LocalOrderStatus::TradeOpen
                                        LocalOrderStatus::FullyFilled
                                    }

                                } else {

                                    LocalOrderStatus::PartiallyFilled {
                                        filled: order_info.size_matched.to_string(),
                                    }
                                }
                            }
                            OrderStatusType::Canceled => {
                                // If dead, stop tracking this order completely on future iterations
                                active_removals.push(order_id.clone());
                                LocalOrderStatus::Canceled
                            }
                            OrderStatusType::Unknown(ref reason) => {
                                if reason == "INVALID" {
                                    // If dead, stop tracking this order completely on future iterations
                                    active_removals.push(order_id.clone());
                                    LocalOrderStatus::Canceled
                                } else {
                                    tracing::warn!(
                                        "Encountered unknown order status '{}' for order {}",
                                        reason,
                                        order_id
                                    );
                                    LocalOrderStatus::Canceled
                                }
                            }
                            _ => {
                                tracing::warn!("Encountered unknown non-exhaustive status type for order {}", order_id);
                                LocalOrderStatus::Canceled
                            }
                        };

                        // 3. Dispatch structured status packet down to your central state framework
                        let _ = poll_tx
                            .send(WorkerUpdate::OrderUpdated {
                                window_ts,
                                order_id: order_id.clone(),
                                status: target_status.clone(),
                                matched: order_info.size_matched.to_string(),
                            })
                            .await;

                        // 4. Also update the Worker State
                        if let Some(o) = tracked_orders_for_polling
                            .lock()
                            .await
                            .get_mut(&order_id)
                        {
                            // keep worker state in sync with API truth
                            o.status = target_status;
                            o.size_matched = order_info
                                .size_matched
                                .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::ToZero)
                                .to_string();

                            o.is_trade_fully_confirmed =
                                is_trade_fully_confirmed;

                            o.associate_trades =
                                order_info.associate_trades.clone();

                            o.open_order_response =
                                Some(order_info.clone());
                        }
                    }
                }

                // Remove fully filled/canceled orders from polling
                if !active_removals.is_empty() {
                    let mut lock = poll_orders.lock().await;
                    lock.retain(|(_, id)| !active_removals.contains(id));
                }

                poll_ctx.request_repaint();
            }
        });

        // -------------------------------------------------------------
        // TASK 1B: TRADE TRACKING LOOP
        // -------------------------------------------------------------
        let tracked_trades_for_polling =
            tracked_trades.clone();

        let tracked_orders_for_trades =
            tracked_orders.clone();

        let trades_interval_clone =
            trades_polling_interval_ms.clone();

        let clob_client_for_trades =
            clob_client.clone();

        tokio::spawn(async move {

            let mut current_interval =
                trades_interval_clone.load(Ordering::Relaxed);

            let mut interval = tokio::time::interval(
                Duration::from_millis(
                    current_interval.max(1)
                )
            );

            loop {

                // -------------------------------------------------
                // HANDLE INTERVAL CHANGES
                // -------------------------------------------------
                let latest_interval =
                    trades_interval_clone.load(
                        Ordering::Relaxed
                    );

                if latest_interval != current_interval {

                    current_interval =
                        latest_interval;

                    interval = tokio::time::interval(
                        Duration::from_millis(
                            current_interval.max(1)
                        )
                    );

                    tracing::info!(
                        "Trades polling interval updated to {} ms",
                        current_interval
                    );
                }

                // -------------------------------------------------
                // POLLING DISABLED
                // -------------------------------------------------
                if current_interval == 0 {
                    tokio::time::sleep(
                        Duration::from_millis(250)
                    )
                    .await;

                    continue;
                }

                interval.tick().await;

                // -------------------------------------------------
                // SNAPSHOT TRACKED ORDERS
                // -------------------------------------------------
                let tracked_snapshot = {
                    tracked_orders_for_trades
                        .lock()
                        .await
                        .clone()
                };

                if tracked_snapshot.is_empty() {
                    continue;
                }

                // -------------------------------------------------
                // FETCH TRADES FOR EACH MARKET
                // -------------------------------------------------
                for (_, tracked_order) in tracked_snapshot {

                    let current_anchor = initiate_stamp_5m();
                    let slug = build_slug_for_timestamp(
                        //tracked_order.window_ts
                        current_anchor
                    );

                    // ---------------------------------------------
                    // FETCH MARKET CONDITION ID
                    // ---------------------------------------------
                    let condition_id = {
                        let cache = MARKET_BY_SLUG_CACHE.lock().unwrap();

                        cache
                            .get(&slug)
                            .and_then(|market| market.condition_id)
                    };

                    let Some(condition_id) = condition_id else {
                        continue;
                    };

                    // ---------------------------------------------
                    // BUILD REQUEST
                    // ---------------------------------------------
                    let mut request =
                        TradesRequest::builder()
                            .build();

                    request.market =
                        Some(condition_id);

                    // ---------------------------------------------
                    // FETCH TRADES
                    // ---------------------------------------------
                    let trades_page_result = {

                        let client_guard =
                            clob_client_for_trades
                                .lock()
                                .await;

                        let Some(client) =
                            client_guard.as_ref()
                        else {
                            tracing::warn!(
                                "Trades polling skipped: client unavailable"
                            );

                            continue;
                        };

                        client
                            .trades(&request, None)
                            .await
                    };

                    // ---------------------------------------------
                    // PROCESS RESPONSE
                    // ---------------------------------------------
                    match trades_page_result {

                        Ok(page) => {

                            let mut trades_lock =
                                tracked_trades_for_polling
                                    .lock()
                                    .await;

                            for trade in page.data {

                                tracing::debug!(
                                    "Tracked trade updated: {} ({:?})",
                                    trade.id,
                                    trade.status
                                );

                                trades_lock.insert(
                                    trade.id.clone(),
                                    trade,
                                );
                            }
                        }

                        Err(err) => {

                            tracing::error!(
                                "Trade polling failed for slug {}: {:?}",
                                slug,
                                err
                            );
                        }
                    }
                }
            }
        });


        // -------------------------------------------------------------
        // TASK 1C: RAPID SELL LOOP
        // -------------------------------------------------------------
        let tracked_orders_for_rapid =
            tracked_orders.clone();

        let rapid_interval_clone =
            rapid_sell_polling_interval_ms.clone();

        let clob_client_for_rapid =
            clob_client.clone();

        let rapid_tx =
            self.update_tx.clone();

        let rapid_poll_orders =
            orders_to_poll.clone();
        
        tokio::spawn(async move {

            let mut current_interval =
                rapid_interval_clone.load(
                    Ordering::Relaxed
                );

            let mut interval = tokio::time::interval(
                Duration::from_millis(
                    current_interval.max(1)
                )
            );

            loop {

                // -------------------------------------------------
                // HANDLE INTERVAL CHANGES
                // -------------------------------------------------
                let latest_interval =
                    rapid_interval_clone.load(
                        Ordering::Relaxed
                    );

                if latest_interval != current_interval {

                    current_interval =
                        latest_interval;

                    interval = tokio::time::interval(
                        Duration::from_millis(
                            current_interval.max(1)
                        )
                    );

                    tracing::info!(
                        "Rapid sell interval updated to {} ms",
                        current_interval
                    );
                }

                // -------------------------------------------------
                // POLLING DISABLED
                // -------------------------------------------------
                if current_interval == 0 {

                    tokio::time::sleep(
                        Duration::from_millis(250)
                    )
                    .await;

                    continue;
                }

                interval.tick().await;

                // -------------------------------------------------
                // SNAPSHOT TRACKED ORDERS
                // -------------------------------------------------
                let mut snapshot = {
                    tracked_orders_for_rapid
                        .lock()
                        .await
                };

                // -------------------------------------------------
                // PROCESS RAPID SELLS
                // -------------------------------------------------
                for snapshot_order in snapshot.values()  {

                    // ---------------------------------------------
                    // BUY ORDERS ONLY
                    // ---------------------------------------------
                    if snapshot_order.side.to_lowercase()
                        != "buy"
                    {
                        continue;
                    }

                    // ---------------------------------------------
                    // REQUIRE TRADE CONFIRMATION
                    // ---------------------------------------------
                    if !matches!(
                        snapshot_order.status,
                        LocalOrderStatus::TradeConfirmed
                    ) {
                        continue;
                    }

                    if !snapshot_order
                        .is_trade_fully_confirmed
                    {
                        continue;
                    }

                    // ---------------------------------------------
                    // PREVENT DUPLICATE ATTEMPTS
                    // ---------------------------------------------
                    match &snapshot_order.rapid_sell_state {

                        RapidSellState::Idle => {}

                        RapidSellState::Failed {
                            retry_count,
                            next_retry_at,
                            ..
                        } => {

                            // max retries exceeded
                            if *retry_count >= MAX_RAPID_RETRIES {
                                tracing::warn!(
                                    "Rapid sell permanently stopped after {} retries for {}",
                                    retry_count,
                                    snapshot_order.id
                                );
                                continue;
                            }

                            // cooldown not elapsed
                            if std::time::Instant::now()
                                < *next_retry_at
                            {
                                continue;
                            }
                        }

                        // ALL OTHER STATES BLOCK
                        _ => continue,
                    }

                    // ---------------------------------------------
                    // PARSE VALUES
                    // ---------------------------------------------
                    let matched =
                        Decimal::from_str(
                            &snapshot_order.size_matched
                        )
                        .unwrap_or_default();

                    let already_sold =
                        Decimal::from_str(
                            &snapshot_order.rapid_sell_size
                        )
                        .unwrap_or_default();

                    let sell_price =
                        Decimal::from_str(
                            &snapshot_order.rapid_sell_price
                        )
                        .unwrap_or_default();

                    let min_sell_size =
                        Decimal::from(5);

                    // ---------------------------------------------
                    // VALIDATE CONFIG
                    // ---------------------------------------------
                    if sell_price <= Decimal::ZERO {
                        continue;
                    }

                    let sell_amount =
                        (matched - already_sold)
                            .max(Decimal::ZERO);

                    if sell_amount < min_sell_size {
                        continue;
                    }

                    // ---------------------------------------------
                    // MARK PENDING IMMEDIATELY
                    // ---------------------------------------------
                    /*
                    {
                        let mut tracked =
                            tracked_orders_for_rapid
                                .lock()
                                .await;

                        if let Some(order) =
                            tracked.get_mut(
                                &snapshot_order.id
                            )
                        {
                            order.rapid_sell_state =
                                RapidSellState::Pending;
                        }
                    }
                    */
                    let should_spawn = {
                    let mut tracked =
                        tracked_orders_for_rapid
                            .lock()
                            .await;

                    if let Some(order) =
                        tracked.get_mut(
                            &snapshot_order.id
                        )
                    {
                        match order.rapid_sell_state {

                            RapidSellState::Idle
                            | RapidSellState::Failed { .. } => {

                                order.rapid_sell_state =
                                    RapidSellState::Pending;

                                true
                            }

                            _ => false,
                        }
                    }
                    else {
                        false
                    }
                };

                if !should_spawn {
                    continue;
                }

                    tracing::info!(
                        "Attempting Rapid Sell: {:?}",
                        snapshot_order
                    );

                    // ---------------------------------------------
                    // BUILD SELL REQUEST
                    // ---------------------------------------------
                    let sell_req =
                        OrderLimitRequest {
                            side: "sell".into(),

                            token:
                                snapshot_order.token.clone(),

                            price:
                                snapshot_order
                                    .rapid_sell_price
                                    .clone(),

                            size:
                                sell_amount.to_string(),
                        };

                    let clob_client_clone =
                        clob_client_for_rapid.clone();

                    let update_tx_clone =
                        rapid_tx.clone();

                    let tracked_orders_clone =
                        tracked_orders_for_rapid.clone();

                    let rapid_poll_orders_clone =
                        rapid_poll_orders.clone();

                    let window_ts_clone =
                        snapshot_order.window_ts;

                    let parent_order_id =
                        snapshot_order.id.clone();

                    let token_clone =
                        snapshot_order.token.clone();

                    let rapid_sell_price_clone =
                        snapshot_order
                            .rapid_sell_price
                            .clone();

                    tokio::spawn(async move {

                        let current_anchor = initiate_stamp_5m();
                        match place_order_limit(
                            clob_client_clone,
                            &sell_req,
                            &build_slug_for_timestamp(
                                //window_ts_clone
                                current_anchor
                            ),
                        )
                        .await
                        {

                            // -------------------------------------
                            // SUCCESS
                            // -------------------------------------
                            Ok(resp) => {

                                tracing::info!(
                                    "Rapid Sell Response: {:?}",
                                    resp
                                );

                                match parse_order_api_response(resp)
                                {

                                    Ok(new_order_id) => {

                                        // -------------------------
                                        // TRACK NEW SELL ORDER
                                        // -------------------------
                                        {
                                            let mut lock =
                                                rapid_poll_orders_clone
                                                    .lock()
                                                    .await;

                                            lock.push((
                                                window_ts_clone,
                                                new_order_id.clone(),
                                            ));
                                        }

                                        // -------------------------
                                        // CREATE TRACKED ORDER
                                        // -------------------------
                                        let new_tracked_order =
                                            TrackedOrder {
                                                id:
                                                    new_order_id.clone(),

                                                side:
                                                    "sell".into(),

                                                token:
                                                    token_clone.clone(),

                                                price:
                                                    rapid_sell_price_clone.clone(),

                                                size:
                                                    sell_amount.to_string(),

                                                status:
                                                    LocalOrderStatus::Open,

                                                size_matched:
                                                    "0".into(),

                                                inline_sell_price:
                                                    "0".into(),

                                                inline_sell_size:
                                                    "0".into(),

                                                inline_sell_market_type:
                                                    "FAK".to_string(),

                                                rapid_sell_price:
                                                    "0".to_string(),

                                                rapid_sell_size:
                                                    "0".into(),

                                                rapid_sell_state:
                                                    RapidSellState::Idle,

                                                is_trade_fully_confirmed:
                                                    false,

                                                associate_trades:
                                                    vec![],

                                                open_order_response:
                                                    None,

                                                window_ts:
                                                    window_ts_clone,

                                                rapid_sell_attempts: 0,
                                            };

                                        // -------------------------
                                        // UPDATE TRACKING STATE
                                        // -------------------------
                                        {
                                            let mut tracked =
                                                tracked_orders_clone
                                                    .lock()
                                                    .await;

                                            tracked.insert(
                                                new_order_id.clone(),
                                                new_tracked_order.clone(),
                                            );

                                            if let Some(parent) =
                                                tracked.get_mut(
                                                    &parent_order_id
                                                )
                                            {
                                                parent.rapid_sell_state =
                                                    RapidSellState::Completed;

                                                parent.rapid_sell_size =
                                                    matched.to_string();
                                            }
                                        }

                                        // -------------------------
                                        // UI UPDATE
                                        // -------------------------
                                        let _ =
                                            update_tx_clone
                                                .send(
                                                    WorkerUpdate::OrderAdded {
                                                        window_ts:
                                                            window_ts_clone,

                                                        order:
                                                            new_tracked_order,
                                                    }
                                                )
                                                .await;

                                        let _ =
                                            update_tx_clone
                                                .send(
                                                    WorkerUpdate::Notify {
                                                        message: format!(
                                                            "Rapid Sell Placed: {} {} @ {}",
                                                            sell_amount,
                                                            token_clone,
                                                            sell_price
                                                        ),

                                                        kind:
                                                            NotificationKind::Success,
                                                    }
                                                )
                                                .await;
                                    }

                                    // -------------------------
                                    // PARSE FAILURE
                                    // -------------------------
                                    Err(err) => {

                                        let mut tracked =
                                            tracked_orders_clone
                                                .lock()
                                                .await;

                                        let err_string = err.to_string();

                                        if let Some(parent) =
                                            tracked.get_mut(
                                                &parent_order_id
                                            )
                                        {
                                            // ---------------------------------
                                            // PERMANENT FAILURE
                                            // ---------------------------------
                                            if is_permanent_rapid_sell_error(
                                                &err_string
                                            ) {

                                                parent.rapid_sell_state =
                                                    RapidSellState::Disabled {
                                                        reason: err_string.clone(),
                                                    };

                                                tracing::warn!(
                                                    "Rapid sell permanently disabled for {}: {}",
                                                    parent_order_id,
                                                    err_string
                                                );
                                            }

                                            // ---------------------------------
                                            // RETRYABLE FAILURE
                                            // ---------------------------------
                                            else {

                                                let retry_count: u32 =
                                                    match &parent.rapid_sell_state {

                                                        RapidSellState::Failed {
                                                            retry_count,
                                                            ..
                                                        } => *retry_count + 1,

                                                        _ => 1u32,
                                                    };

                                                if retry_count >= MAX_RAPID_RETRIES {

                                                    parent.rapid_sell_state =
                                                        RapidSellState::Disabled {

                                                            reason: format!(
                                                                "Exceeded max retries: {}",
                                                                err_string
                                                            ),
                                                        };

                                                    tracing::error!(
                                                        "Rapid sell disabled after max retries for {}",
                                                        parent_order_id
                                                    );
                                                }
                                                else {

                                                    let delay_secs =
                                                        compute_retry_delay_secs(
                                                            retry_count
                                                        );

                                                    parent.rapid_sell_state =
                                                        RapidSellState::Failed {

                                                            reason:
                                                                err_string.clone(),

                                                            retry_count,

                                                            next_retry_at:
                                                                std::time::Instant::now()
                                                                    + Duration::from_secs(
                                                                        delay_secs
                                                                    ),
                                                        };

                                                    tracing::warn!(
                                                        "Rapid sell retry {} scheduled in {}s for {}",
                                                        retry_count,
                                                        delay_secs,
                                                        parent_order_id
                                                    );
                                                }
                                            }
                                        }

                                        let _ =
                                            update_tx_clone
                                                .send(
                                                    WorkerUpdate::Notify {
                                                        message: format!(
                                                            "Rapid Sell Parse Failed: {}",
                                                            err
                                                        ),

                                                        kind:
                                                            NotificationKind::Error,
                                                    }
                                                )
                                                .await;
                                    }
                                }
                            }

                            // -------------------------------------
                            // TRANSPORT FAILURE
                            // -------------------------------------
                            Err(err) => {

                                let mut tracked =
                                    tracked_orders_clone
                                        .lock()
                                        .await;

                                if let Some(parent) =
                                    tracked.get_mut(
                                        &parent_order_id
                                    )
                                {
                                    let retry_count: u32 =
                                        parent.rapid_sell_attempts as u32 + 1;

                                    parent.rapid_sell_attempts =
                                        retry_count as u8;

                                    parent.rapid_sell_state =
                                        RapidSellState::Failed {

                                            reason: err.to_string(),

                                            retry_count,

                                            next_retry_at:
                                                std::time::Instant::now()
                                                    + Duration::from_secs(
                                                        compute_retry_delay_secs(
                                                            retry_count
                                                        )
                                                    ),
                                        };
                                }

                                let _ =
                                    update_tx_clone
                                        .send(
                                            WorkerUpdate::Notify {
                                                message: format!(
                                                    "Rapid Sell Failed: {}",
                                                    err
                                                ),

                                                kind:
                                                    NotificationKind::Error,
                                            }
                                        )
                                        .await;
                            }
                        }
                    });
                }
            }
        });

        // -------------------------------------------------------------
        // TASK 2: High-Speed UI Command Dispatcher Loop
        // -------------------------------------------------------------
        tracing::info!("PolymarketWorker: Listening for UI Commands...");

        while let Some(cmd) = self.cmd_rx.recv().await {
            tracing::info!("WORKER: Received command from UI channel: {:?}", cmd);
            let update_tx = self.update_tx.clone();
            let ctx = self.ctx.clone();
            let cmd_orders_list = Arc::clone(&orders_to_poll);
            let cmd_tracked_orders = Arc::clone(&tracked_orders);

            let clob_client_for_ui_command = clob_client.clone(); // clone the Arc for this task

            match cmd {
                UiCommand::InitializeClient { token } => {
                    tracing::info!("Worker received API token initialization request: {}", token);
                    // Configure your API client instances with the fresh authorization token here
                    // Optional: If your background worker manages an HTTP client wrapper, 
                    // you would pass the token to it here. 
                    // e.g., *self.client.lock().unwrap() = Some(PolymarketClient::new(token));
                }
                UiCommand::UpdatePollInterval {
                    milliseconds,
                    queue,
                } => {
                    self.poll_config.set(queue, milliseconds);

                    tracing::info!(
                        "Updated polling interval to {} ms for {:?}",
                        milliseconds,
                        queue
                    );
                }
                UiCommand::PlaceLimit { side, token, price, size, rapid_price, window_ts } => {
                    tracing::info!("Worker caught PlaceLimit command!");
                    tokio::spawn(async move {
                        /*
                        let client_guard = client.lock().await;
                        let client = match &*client_guard {
                            Some(c) => c,
                            None => {
                                tracing::error!("ClobClient not initialized");
                                ctx.request_repaint();
                                return;
                            }
                        };
                        */

                        let current_anchor = initiate_stamp_5m();
                        let slug = build_slug_for_timestamp(current_anchor);
                        let req = OrderLimitRequest {
                            side: side.clone(),
                            token: token.clone(),
                            price: price.clone(),
                            size: size.clone(),
                        };

                        match place_order_limit(clob_client_for_ui_command.clone(), &req, &slug).await {
                            Ok(order_response) => {
                                match parse_order_api_response(order_response) {
                                    Ok(order_id) => {
                                        let new_order = TrackedOrder {
                                            id: order_id.clone(),
                                            side,
                                            token,
                                            price,
                                            size,
                                            status: LocalOrderStatus::Open,
                                            size_matched: "0".to_string(),
                                            inline_sell_price: "0.10".to_string(),
                                            inline_sell_size: "0".to_string(),
                                            inline_sell_market_type: "FAK".to_string(),
                                            rapid_sell_price: rapid_price.to_string(),
                                            rapid_sell_size: "0".into(),
                                            rapid_sell_state: RapidSellState::Idle,
                                            is_trade_fully_confirmed: false,
                                            associate_trades: vec![],
                                            open_order_response: None,
                                            window_ts,
                                            rapid_sell_attempts: 0,
                                        };

                                        {
                                            let mut lock = cmd_orders_list.lock().await;
                                            lock.push((window_ts, order_id.clone()));
                                        }

                                        {
                                            let mut tracked =
                                                cmd_tracked_orders
                                                    .lock()
                                                    .await;

                                            tracked.insert(
                                                order_id.clone(),
                                                new_order.clone(),
                                            );
                                        }

                                        /*
                                        tracked_orders
                                            .lock()
                                            .await
                                            .insert(order_id.clone(), new_order.clone());
                                            */

                                        let _ = update_tx.send(WorkerUpdate::OrderAdded { window_ts, order: new_order }).await;
                                        let _ = update_tx
                                            .send(WorkerUpdate::Notify {
                                                message: "Limit Order Placed Successfully!".into(),
                                                kind: NotificationKind::Success,
                                            })
                                            .await;
                                    }
                                    Err(domain_err) => {
                                        // Catches internal engine rejections (success: false) or API errors (400, 401, etc.)
                                        let _ = update_tx
                                            .send(WorkerUpdate::Notify {
                                                message: format!("Limit Processing Failed: {}", domain_err),
                                                kind: NotificationKind::Error,
                                            })
                                            .await;
                                    }
                                }
                            }
                            Err(e) => {
                                // Catches transport/network layer errors returned directly by place_order_limit
                                let _ = update_tx
                                    .send(WorkerUpdate::Notify {
                                        message: format!("Limit Transport Failed: {}", e),
                                        kind: NotificationKind::Error,
                                    })
                                    .await;
                            }
                        }
                        ctx.request_repaint();
                    });
                }

                UiCommand::PlaceMarket { side, token, usdc, shares, order_type, window_ts } => {
                    tracing::info!("Worker caught PlaceMarket command!");
                    tokio::spawn(async move {
                        let current_anchor = initiate_stamp_5m();
                        let slug = build_slug_for_timestamp(current_anchor);
                        let req = OrderMarketRequest {
                            side: side.clone(),
                            token: token.clone(),
                            usdc,
                            shares,
                            order_type,
                        };

                        match place_order_market(clob_client_for_ui_command.clone(), &req, &slug).await {
                            Ok(order_response) => {
                                match parse_order_api_response(order_response) {
                                    Ok(order_id) => {
                                        let new_order = TrackedOrder {
                                            id: order_id.clone(),
                                            side,
                                            token,
                                            price: "Market".into(),
                                            size: "Market".into(),
                                            status: LocalOrderStatus::FullyFilled,
                                            size_matched: "Full".to_string(),
                                            inline_sell_price: "0.50".to_string(),
                                            inline_sell_size: "0".to_string(),
                                            inline_sell_market_type: "FAK".to_string(),
                                            rapid_sell_price: "0.00".to_string(),
                                            rapid_sell_size: "0".into(),
                                            rapid_sell_state: RapidSellState::Idle,
                                            is_trade_fully_confirmed: false,
                                            associate_trades: vec![],
                                            open_order_response: None,
                                            window_ts,
                                            rapid_sell_attempts: 0,
                                        };

                                        {
                                            let mut lock = cmd_orders_list.lock().await;
                                            lock.push((window_ts, order_id.clone()));
                                        }

                                        {
                                            let mut tracked =
                                                cmd_tracked_orders
                                                    .lock()
                                                    .await;

                                            tracked.insert(
                                                order_id.clone(),
                                                new_order.clone(),
                                            );
                                        }

                                        let _ = update_tx.send(WorkerUpdate::OrderAdded { window_ts, order: new_order }).await;
                                        let _ = update_tx
                                            .send(WorkerUpdate::Notify {
                                                message: "Market Order Filled Successfully!".into(),
                                                kind: NotificationKind::Success,
                                            })
                                            .await;
                                    }
                                    Err(domain_err) => {
                                        // Catches internal engine rejections (success: false) or API errors (400, 401, etc.)
                                        let _ = update_tx
                                            .send(WorkerUpdate::Notify {
                                                message: format!("Market Processing Failed: {}", domain_err),
                                                kind: NotificationKind::Error,
                                            })
                                            .await;
                                    }
                                }
                            }
                            Err(e) => {
                                // Catches transport/network layer errors returned directly by place_order_market
                                let _ = update_tx
                                    .send(WorkerUpdate::Notify {
                                        message: format!("Market Transport Failed: {}", e),
                                        kind: NotificationKind::Error,
                                    })
                                    .await;
                            }
                        }
                        ctx.request_repaint();
                    });
                }

                UiCommand::CheckStatus { order_id, window_ts } => {
                    tracing::info!("Worker caught CheckStatus command!");
                    let update_tx = update_tx.clone();
                    let ctx = ctx.clone();

                    tokio::spawn(async move {
                        // Assuming get_order_status returns anyhow::Result<OpenOrderResponse>
                        if let Ok(order_info) = get_order_status(clob_client_for_ui_command.clone(), &order_id).await {
                            
                            // 1. Math check to differentiate Full vs Partial execution states
                            //let is_fully_filled = order_info.size_matched >= order_info.original_size;
                            let tolerance_pct = dec!(0.005); // 0.5%
                            let is_fully_filled =
                                order_info.size_matched >= order_info.original_size * (dec!(1.0) - tolerance_pct);
                            let matched_string = format!("{}/{}", order_info.size_matched, order_info.original_size);

                            // 2. Map structural response conditions to local UI lifecycle tokens
                            let target_status = match order_info.status {
                                OrderStatusType::Live => {
                                    if order_info.size_matched > rust_decimal::Decimal::ZERO {
                                        LocalOrderStatus::PartiallyFilled {
                                            filled: order_info.size_matched.to_string(),
                                        }
                                    } else {
                                        LocalOrderStatus::Open
                                    }
                                }
                                OrderStatusType::Matched => {
                                    if is_fully_filled {
                                        LocalOrderStatus::FullyFilled
                                    } else {
                                        LocalOrderStatus::PartiallyFilled {
                                            filled: order_info.size_matched.to_string(),
                                        }
                                    }
                                }
                                OrderStatusType::Canceled => {
                                    LocalOrderStatus::Canceled
                                }
                                OrderStatusType::Unknown(ref reason) => {
                                    if reason == "INVALID" {
                                        LocalOrderStatus::Canceled
                                    } else {
                                        tracing::warn!(
                                            "Encountered unknown order status '{}' for order {}",
                                            reason,
                                            order_id
                                        );
                                        LocalOrderStatus::Canceled
                                    }
                                }
                                _ => {
                                    tracing::warn!("Encountered unknown non-exhaustive status type during manual check for order {}", order_id);
                                    LocalOrderStatus::Canceled
                                }
                            };

                            // 3. Dispatch uniform status down to the egui view model
                            let _ = update_tx
                                .send(WorkerUpdate::OrderUpdated {
                                    window_ts,
                                    order_id: order_id.clone(),
                                    status: target_status.clone(),
                                    matched: order_info.size_matched.to_string(),
                                })
                                .await;

                            // 4. Also update the Worker State
                            if let Some(o) = cmd_tracked_orders
                                .lock()
                                .await
                                .get_mut(&order_id)
                            {
                                // keep worker state in sync with API truth
                                o.status = target_status;
                                o.size_matched = order_info
                                    .size_matched
                                    .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::ToZero)
                                    .to_string();
                            }
                        }
                        ctx.request_repaint();
                    });
                }

                UiCommand::CancelIndividual { order_id, window_ts } => {
                    tracing::info!("Worker caught CancelIndividual command!");
                    tokio::spawn(async move {
                        match cancel_order(clob_client_for_ui_command.clone(), &order_id).await {
                            Ok(cancel_data) => {
                                // Verify that our specific single order ID successfully made it into the canceled list
                                if cancel_data.canceled.contains(&order_id) {
                                    let _ = update_tx
                                        .send(WorkerUpdate::OrderUpdated {
                                            window_ts,
                                            order_id: order_id.clone(),
                                            status: LocalOrderStatus::Canceled,
                                            matched: "Canceled".to_string(),
                                        })
                                        .await;

                                    {
                                        let mut lock = cmd_orders_list.lock().await;
                                        lock.retain(|(_, id)| id != &order_id);
                                    }

                                    let _ = update_tx
                                        .send(WorkerUpdate::Notify {
                                            message: "Order Cancelled Successfully".into(),
                                            kind: NotificationKind::Success,
                                        })
                                        .await;
                                } else {
                                    // If it failed, extract the exact error string from the HashMap
                                    let reason = cancel_data.not_canceled.get(&order_id)
                                        .map(|s| s.as_str())
                                        .unwrap_or("Unknown exchange rejection reason");

                                    let _ = update_tx
                                        .send(WorkerUpdate::Notify {
                                            message: format!("Cancellation Rejected: {}", reason),
                                            kind: NotificationKind::Error,
                                        })
                                        .await;
                                }
                            }
                            Err(e) => {
                                let _ = update_tx
                                    .send(WorkerUpdate::Notify {
                                        message: format!("Cancellation Transport Failed: {}", e),
                                        kind: NotificationKind::Error,
                                    })
                                    .await;
                            }
                        }
                        ctx.request_repaint();
                    });
                }

                UiCommand::CancelAllInWindow { window_ts } => {
                    tracing::info!("Worker caught CancelAllInWindow command!");
                    let cmd_orders_list = Arc::clone(&cmd_orders_list);
                    let update_tx = update_tx.clone();
                    let ctx = ctx.clone();

                    tokio::spawn(async move {
                        // 1. Gather all local order IDs inside this window_ts block
                        let local_window_ids: Vec<String> = {
                            let lock = cmd_orders_list.lock().await;
                            lock.iter()
                                .filter(|(ts, _)| *ts == window_ts)
                                .map(|(_, id)| id.clone())
                                .collect()
                        };

                        // 2. Dispatch the single batch cancellation request directly to Polymarket
                        // cancel_all_orders() returns anyhow::Result<CancelOrdersResponse>
                        match cancel_all_orders(clob_client_for_ui_command.clone()).await {
                            Ok(api_summary) => {
                                // api_summary.canceled is natively a Vec<String>
                                let canceled_count = api_summary.canceled.len();

                                // 3. Update local egui status for orders successfully dropped by the API
                                for order_id in local_window_ids {
                                    // Direct lookup inside the standard Vec<String>
                                    if api_summary.canceled.contains(&order_id) {
                                        let _ = update_tx
                                            .send(WorkerUpdate::OrderUpdated {
                                                window_ts,
                                                order_id: order_id.clone(),
                                                status: LocalOrderStatus::Canceled,
                                                matched: "Canceled".to_string(),
                                            })
                                            .await;

                                        // Remove this specific confirmed order from your local master tracking list
                                        {
                                            let mut lock = cmd_orders_list.lock().await;
                                            lock.retain(|(_, id)| id != &order_id);
                                        }
                                    } else {
                                        // Check if the API explicitly provided an execution rejection reason string
                                        if let Some(reason) = api_summary.not_canceled.get(&order_id) {
                                            tracing::warn!(
                                                "Order {} in window {} not canceled. Reason: {}", 
                                                order_id, window_ts, reason
                                            );
                                        } else {
                                            tracing::warn!(
                                                "Order {} in window {} was omitted from API response fields", 
                                                order_id, window_ts
                                            );
                                        }
                                    }
                                }

                                // 4. Provide feedback using the straight length of the native vector
                                let _ = update_tx
                                    .send(WorkerUpdate::Notify {
                                        message: format!("Batch Cancel Finished! Canceled {} orders.", canceled_count),
                                        kind: NotificationKind::Success,
                                    })
                                    .await;
                            }
                            Err(e) => {
                                let _ = update_tx
                                    .send(WorkerUpdate::Notify {
                                        message: format!("Batch Cancel Failed: {}", e),
                                        kind: NotificationKind::Error,
                                    })
                                    .await;
                            }
                        }
                        ctx.request_repaint();
                    });    
                }
            }
        }
        tracing::warn!("Worker command channel was dropped/closed.");

        Ok(())
    }
}

// ==========================================
// Core Business Logic (Your SDK Code)
// ==========================================

pub struct Timer {
    label: String,
    start: Instant,
}

impl Timer {
    fn start(label: &str) -> Self {
        Timer {
            label: label.to_string(),
            start: Instant::now(),
        }
    }

    fn done(&self) {
        let elapsed = self.start.elapsed();
        info!("Step '{}' completed in {:?}", self.label, elapsed);
    }
}

macro_rules! timed {
    ($label:expr, $block:block) => {{
        let timer = Timer::start($label);
        let result = { $block };
        timer.done();
        result
    }};
}

pub fn initiate_stamp_5m() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();
    now - (now % 300)
}

pub fn build_slug_for_timestamp(ts: u64) -> String {
    format!("btc-updown-5m-{}", ts)
}

pub fn build_slug() -> String {
    build_slug_for_timestamp(initiate_stamp_5m())
}

#[derive(Deserialize, Debug)]
pub struct ApiErrorResponse {
    pub error: String,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum ApiResponsePayload {
    Success(PostOrderResponse),
    Failure(ApiErrorResponse),
}

/// Parses the JSON response cleanly by inspecting keys explicitly
pub fn parse_order_api_response(
    order_data: PostOrderResponse,
) -> Result<String, Error> {
    // 1. Check the internal business logic success flag
    if !order_data.success {
        let msg = order_data.error_msg.unwrap_or_else(|| "Order rejected".into());
        return Err(Error::validation(format!("Engine Reject: {}", msg)));
    }
    
    // 2. Return the clean order ID
    Ok(order_data.order_id)
}

#[instrument(skip(client))]
async fn get_or_fetch_market_by_slug(
    client: &GammaClient,
    slug: &str,
) -> anyhow::Result<Market> {
    {
        let markets = MARKET_BY_SLUG_CACHE.lock().unwrap();

        if let Some(market) = markets.get(slug) {
            return Ok(market.clone());
        }
    }

    let market_request = MarketBySlugRequest::builder()
        .slug(slug)
        .build();

    let market = client.market_by_slug(&market_request).await?;

    {
        let mut markets = MARKET_BY_SLUG_CACHE.lock().unwrap();

        // optional: keep only latest cached market
        markets.clear();

        markets.insert(slug.to_string(), market.clone());
    }

    Ok(market)
}

/*
#[instrument(skip(client))]
async fn get_or_fetch_token_ids(client: &GammaClient, slug: &str) -> anyhow::Result<Vec<String>> {
    {
        let tokens = TOKEN_IDS_CACHE.lock().unwrap();
        if let Some(ids) = tokens.get(slug) {
            return Ok(ids.clone());
        }
    }
    let market_request = MarketBySlugRequest::builder().slug(slug).build();
    let market = client.market_by_slug(&market_request).await?;
    let token_ids: Vec<String> = if let Some(clob_tokens) = &market.clob_token_ids {
        clob_tokens.iter().map(|t| t.to_string()).collect()
    } else {
        vec![]
    };
    {
        let mut tokens = TOKEN_IDS_CACHE.lock().unwrap();
        tokens.clear();
        tokens.insert(slug.to_string(), token_ids.clone());
    }
    Ok(token_ids)
}
*/
async fn get_or_fetch_token_ids(
    client: &GammaClient,
    slug: &str,
) -> anyhow::Result<Vec<String>> {
    // This internally checks MARKET_BY_SLUG_CACHE first
    let market = get_or_fetch_market_by_slug(client, slug).await?;

    let token_ids: Vec<String> = market
        .clob_token_ids
        .as_ref()
        .map(|tokens| {
            tokens
                .iter()
                .map(|t| t.to_string())
                .collect()
        })
        .unwrap_or_default();

    Ok(token_ids)
}

pub fn get_or_fetch_api_creds(
    private_key: String,
    host: String,
) -> impl std::future::Future<Output = anyhow::Result<Credentials>> {
    async move {
        let cache_key = format!("{}@{}", private_key, host);
        {
            let cache = API_CREDS_CACHE.lock().unwrap();
            if let Some(creds) = cache.get(&cache_key) {
                return Ok(creds.clone());
            }
        }
        let signer = LocalSigner::from_str(&private_key)?.with_chain_id(Some(polybot_sdk_v2::POLYGON));
        let client = ClobClient::new(&host, Config::default())?;
        let creds: Credentials = client.create_or_derive_api_key(&signer, None).await?;
        {
            let mut cache = API_CREDS_CACHE.lock().unwrap();
            cache.insert(cache_key, creds.clone());
        }
        Ok(creds)
    }
}

async fn place_order_limit(
    clob_client: Arc<Mutex<Option<ClobClient<Authenticated<Normal>>>>>,
    payload: &OrderLimitRequest,
    target_slug: &str,
) -> anyhow::Result<PostOrderResponse> {
    let total_timer = Timer::start("place_order_limit_total");

    // Load signer
    let private_key = std::env::var("PRIVATE_KEY_VAR")?;
    let signer = LocalSigner::from_str(&private_key)?
        .with_chain_id(Some(polybot_sdk_v2::POLYGON));

    // Fetch token IDs
    let gamma_client = GammaClient::default();
    let token_ids = get_or_fetch_token_ids(&gamma_client, target_slug).await?;
    if token_ids.len() < 2 {
        anyhow::bail!("No token IDs available for slug {}", target_slug);
    }

    // Determine token ID using case-insensitive match
    let token_id = if payload.token.eq_ignore_ascii_case("up") {
        U256::from_str(&token_ids[0])?
    } else if payload.token.eq_ignore_ascii_case("down") {
        U256::from_str(&token_ids[1])?
    } else {
        anyhow::bail!("Invalid token: {}. Must be 'up' or 'down'", payload.token);
    };

    // Parse price and size
    let price = Decimal::from_str(&payload.price)?;
    let size = Decimal::from_str(&payload.size)?;

    // Determine side using case-insensitive match
    let side = if payload.side.eq_ignore_ascii_case("buy") {
        Side::Buy
    } else if payload.side.eq_ignore_ascii_case("sell") {
        Side::Sell
    } else {
        anyhow::bail!("Invalid side: {}", payload.side);
    };

    // Acquire CLOB client
    let mut client_guard = clob_client.lock().await;
    let client = client_guard
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("CLOB client not initialized"))?;

    // Configure tick size
    client.set_tick_size(token_id, TickSize::Hundredth);

    // Build order
    let order = client
        .limit_order()
        .token_id(token_id)
        .size(size)
        .price(price)
        .side(side)
        .build()
        .await?;

    // Sign and post order
    let signed_order = timed!("sign order", { client.sign(&signer, order).await? });
    let response = timed!("post order", { client.post_order(signed_order).await? });

    total_timer.done();
    Ok(response)
}

async fn place_order_market(
    clob_client: Arc<Mutex<Option<ClobClient<Authenticated<Normal>>>>>,
    payload: &OrderMarketRequest, 
    target_slug: &str
) -> anyhow::Result<PostOrderResponse> {
    let private_key = std::env::var("PRIVATE_KEY_VAR")?;
    let signer = LocalSigner::from_str(&private_key)?.with_chain_id(Some(polybot_sdk_v2::POLYGON));

    let gamma_client = GammaClient::default();
    let token_ids = get_or_fetch_token_ids(&gamma_client, target_slug).await?;
    if token_ids.len() < 2 { anyhow::bail!("No token IDs available for slug {}", target_slug); }

    let token_id = match payload.token.to_lowercase().as_str() {
        "up" => U256::from_str(&token_ids[0])?,
        "down" => U256::from_str(&token_ids[1])?,
        _ => anyhow::bail!("Invalid token"),
    };
    let side = match payload.side.to_lowercase().as_str() {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        _ => anyhow::bail!("Invalid side"),
    };
    let order_type = match payload.order_type.as_deref() {
        Some("FAK") => OrderType::FAK,
        _ => OrderType::FOK,
    };

    let mut client_guard = clob_client.lock().await;
    let client = client_guard.as_mut().ok_or_else(|| anyhow::anyhow!("CLOB client not initialized"))?;

    client.set_tick_size(token_id, TickSize::Hundredth);

    let mut order_builder = client.market_order().token_id(token_id).side(side).order_type(order_type);
    if let Some(usdc) = &payload.usdc {
        order_builder = order_builder.amount(Amount::usdc(Decimal::from_str(usdc)?)?);
    } else if let Some(shares) = &payload.shares {
        order_builder = order_builder.amount(Amount::shares(Decimal::from_str(shares)?)?);
    } else {
        anyhow::bail!("Missing usdc or shares context");
    }

    let order = order_builder.build().await?;
    let signed_order = client.sign(&signer, order).await?;
    let response = client.post_order(signed_order).await?;

    Ok(response)
}

async fn get_order_status(
    clob_client: Arc<Mutex<Option<ClobClient<Authenticated<Normal>>>>>,
    order_id: &str
) -> anyhow::Result<OpenOrderResponse> {
    let mut client_guard = clob_client.lock().await;
    let client = client_guard.as_mut().ok_or_else(|| anyhow::anyhow!("CLOB client not initialized"))?;

    let order_response = client.order(order_id).await?;

    Ok(order_response)
}

async fn cancel_order(
    clob_client: Arc<Mutex<Option<ClobClient<Authenticated<Normal>>>>>,
    order_id: &str
) -> anyhow::Result<CancelOrdersResponse> {
    let mut client_guard = clob_client.lock().await;
    let client = client_guard.as_mut().ok_or_else(|| anyhow::anyhow!("CLOB client not initialized"))?;

    let resp = client.cancel_order(order_id).await?;

    Ok(resp)
}

async fn cancel_all_orders(
    clob_client: Arc<Mutex<Option<ClobClient<Authenticated<Normal>>>>>,
) -> anyhow::Result<CancelOrdersResponse> {
    let mut client_guard = clob_client.lock().await;
    let client = client_guard.as_mut().ok_or_else(|| anyhow::anyhow!("CLOB client not initialized"))?;

    let resp = client.cancel_all_orders().await?;

    Ok(resp)
}