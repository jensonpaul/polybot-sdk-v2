use crate::ui_types::{
    LocalOrderStatus, NotificationKind, OrderLimitRequest, OrderMarketRequest, TrackedOrder, UiCommand, WorkerUpdate,
};
use eframe::egui;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc::{Receiver, Sender};

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
use polybot_sdk_v2::clob::types::{Side, SignatureType, OrderType, OrderStatusType, Amount, TickSize};
use polybot_sdk_v2::clob::types::response::{PostOrderResponse, OpenOrderResponse, CancelOrdersResponse};
use polybot_sdk_v2::gamma::Client as GammaClient;
use polybot_sdk_v2::gamma::types::request::MarketBySlugRequest;
use polybot_sdk_v2::types::{Address, Decimal, U256};

use serde::Deserialize;
use tracing::{info, instrument};

pub use polybot_sdk_v2::error::Error;

lazy_static! {
    static ref TOKEN_IDS_CACHE: std::sync::Mutex<HashMap<String, Vec<String>>> = std::sync::Mutex::new(HashMap::new());
    static ref API_CREDS_CACHE: std::sync::Mutex<HashMap<String, Credentials>> = std::sync::Mutex::new(HashMap::new());
}
// END

pub struct PolymarketWorker {
    pub cmd_rx: Receiver<UiCommand>,
    pub update_tx: Sender<WorkerUpdate>,
    pub ctx: egui::Context,
    pub clob_client: Arc<Mutex<Option<ClobClient<Authenticated<Normal>>>>>, // persistent client
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

        let orders_to_poll = Arc::new(Mutex::new(Vec::<(u64, String)>::new()));

        // Initialize the client
        let client = self.init_clob_client().await?;
        let clob_client = Arc::new(tokio::sync::Mutex::new(Some(client)));

        // -------------------------------------------------------------
        // TASK 1: Isolated Heartbeat Polling Loop
        // -------------------------------------------------------------
        let poll_orders = Arc::clone(&orders_to_poll);
        let poll_tx = self.update_tx.clone();
        let poll_ctx = self.ctx.clone();

        let clob_client_for_tracking = clob_client.clone(); // clone the Arc for this task

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(4));
            loop {
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
                        let is_fully_filled = order_info.size_matched >= order_info.original_size;
                        let matched_string = format!("{}/{}", order_info.size_matched, order_info.original_size);

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
                                if is_fully_filled {
                                    active_removals.push(order_id.clone());
                                    LocalOrderStatus::FullyFilled
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
                            _ => {
                                tracing::warn!("Encountered unknown non-exhaustive status type for order {}", order_id);
                                LocalOrderStatus::Open
                            }
                        };

                        // 3. Dispatch structured status packet down to your central state framework
                        let _ = poll_tx
                            .send(WorkerUpdate::OrderUpdated {
                                window_ts,
                                order_id: order_id.clone(),
                                status: target_status,
                                matched: matched_string,
                            })
                            .await;
                    }
                }

                if !active_removals.is_empty() {
                    let mut lock = poll_orders.lock().await;
                    lock.retain(|(_, id)| !active_removals.contains(id));
                }

                poll_ctx.request_repaint();
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

            let clob_client_for_ui_command = clob_client.clone(); // clone the Arc for this task

            match cmd {
                UiCommand::InitializeClient { token } => {
                    tracing::info!("Worker received API token initialization request: {}", token);
                    // Configure your API client instances with the fresh authorization token here
                    // Optional: If your background worker manages an HTTP client wrapper, 
                    // you would pass the token to it here. 
                    // e.g., *self.client.lock().unwrap() = Some(PolymarketClient::new(token));
                }
                UiCommand::PlaceLimit { side, token, price, size, window_ts } => {
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
                                            inline_sell_price: "0.50".to_string(),
                                            inline_sell_size: "0".to_string(),
                                        };

                                        {
                                            let mut lock = cmd_orders_list.lock().await;
                                            lock.push((window_ts, order_id.clone()));
                                        }

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
                                        };
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
                            let is_fully_filled = order_info.size_matched >= order_info.original_size;
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
                                _ => {
                                    tracing::warn!("Encountered unknown non-exhaustive status type during manual check for order {}", order_id);
                                    LocalOrderStatus::Open
                                }
                            };

                            // 3. Dispatch uniform status down to the egui view model
                            let _ = update_tx
                                .send(WorkerUpdate::OrderUpdated {
                                    window_ts,
                                    order_id,
                                    status: target_status,
                                    matched: matched_string,
                                })
                                .await;
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