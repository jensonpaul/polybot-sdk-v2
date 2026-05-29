//! # Shared Application State
//!
//! Single source of truth used by both the UI thread and the background worker.
//!
//! ## Design
//! - [`AppState`] is wrapped in `Arc` and cloned into both sides at startup.
//! - Orders and trades live in `DashMap` — concurrent reads/writes with no
//!   `Mutex` overhead on the read path.
//! - `MarketPrices` per window live in `ArcSwap<MarketPrices>` — wait-free
//!   reads; the writer atomically replaces the pointer.
//! - Neither the UI nor the worker holds a `Mutex<HashMap>` of their own.
//!   **There is only one copy.**

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use arc_swap::ArcSwap;
use dashmap::DashMap;

use polymarket_client_sdk_v2::clob::types::response::{OpenOrderResponse, TradeResponse};

// ---------------------------------------------------------------------------
// Order lifecycle
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum LocalOrderStatus {
    Open,
    PartiallyFilled { filled: String },
    /// Engine matched all shares; trade settlement pending.
    FullyFilled,
    /// Engine matched; some trades still confirming on-chain.
    TradeOpen,
    /// All trades on-chain confirmed.
    TradeConfirmed,
    Canceled,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum RapidSellState {
    Idle,
    Pending,
    Completed,
    Failed(String),
}

/// A single tracked order — shared between the worker (writes) and the UI
/// (reads + inline-sell field mutations).
///
/// Inline sell fields (`inline_sell_price`, `inline_sell_size`,
/// `inline_sell_market_type`) are ephemeral UI state that lives here for
/// convenience; only the UI thread writes them and only while the order card
/// is rendered.
#[derive(Clone, Debug)]
pub struct TrackedOrder {
    pub id: String,
    pub side: String,
    pub token: String,
    pub price: String,
    pub size: String,

    /// Actual fill price from the exchange (populated after a status poll).
    pub executed_price: Option<String>,
    /// Actual fill size from the exchange.
    pub executed_size: Option<String>,

    pub status: LocalOrderStatus,
    pub size_matched: String,

    // ----- inline exit desk (UI-only fields, worker ignores) -----
    pub inline_sell_price: String,
    pub inline_sell_size: String,
    pub inline_sell_market_type: String,

    // ----- rapid-sell automation -----
    pub rapid_sell_price: String,
    pub rapid_sell_size: String,
    pub rapid_sell_state: RapidSellState,

    pub is_trade_fully_confirmed: bool,
    pub associate_trades: Vec<String>,
    pub open_order_response: Option<OpenOrderResponse>,

    /// The 5-minute window this order belongs to.
    pub window_ts: u64,
}

// ---------------------------------------------------------------------------
// Market feed
// ---------------------------------------------------------------------------

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

impl Default for MarketPrices {
    fn default() -> Self {
        Self {
            up_price: 0.0,
            down_price: 0.0,
            up_asset_id: Arc::from(""),
            down_asset_id: Arc::from(""),
            connected: false,
            stale: true,
            last_ts: 0,
            error: None,
        }
    }
}

pub type SharedMarketPrices = Arc<ArcSwap<MarketPrices>>;

#[derive(Clone, Debug)]
pub struct MarketFeedHandle {
    pub shutdown: Arc<tokio::sync::Notify>,
}

// ---------------------------------------------------------------------------
// Window group (UI display unit — purely derived, never source-of-truth)
// ---------------------------------------------------------------------------

/// A 5-minute trading window shown in the UI matrix.
///
/// `orders` here are *keys* into `AppState::orders`; the UI retrieves the
/// live data via `AppState::orders.get(id)` when rendering.
///
/// `market_prices` is a cloned `Arc<ArcSwap<MarketPrices>>` that the worker
/// wrote into `AppState::market_feeds` when it started the feed.
#[derive(Clone)]
pub struct WindowGroup {
    pub timestamp_5m: u64,
    pub slug: String,
    pub is_expanded: bool,
    /// Order IDs that belong to this window (insertion-ordered).
    pub order_ids: Vec<String>,
    pub market_prices: Option<SharedMarketPrices>,
}

// ---------------------------------------------------------------------------
// Notifications (ephemeral, UI-only)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum NotificationKind {
    Success,
    Error,
    Info,
    Warning,
    Debug,
    Trace,
}

#[derive(Clone, Debug)]
pub struct ToastNotification {
    pub message: String,
    pub kind: NotificationKind,
    pub expires_at: std::time::Instant,
}

// ---------------------------------------------------------------------------
// AppState — the single shared mutable graph
// ---------------------------------------------------------------------------

/// Clone the `Arc`; never clone the inner maps.
#[derive(Clone)]
pub struct AppState {
    /// All tracked orders keyed by `order_id`.
    pub orders: Arc<DashMap<String, TrackedOrder>>,

    /// All known trades keyed by `trade_id`.
    pub trades: Arc<DashMap<String, TradeResponse>>,

    /// Live market-price handles keyed by `window_ts`.
    pub market_feeds: Arc<DashMap<u64, MarketFeedHandle>>,

    /// Shared market price snapshots keyed by `window_ts`.
    pub market_prices: Arc<DashMap<u64, SharedMarketPrices>>,

    /// Monotonically increasing version counter — bump on every write so the
    /// UI can detect "did anything change since last frame?".
    pub version: Arc<std::sync::atomic::AtomicU64>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            orders: Arc::new(DashMap::new()),
            trades: Arc::new(DashMap::new()),
            market_feeds: Arc::new(DashMap::new()),
            market_prices: Arc::new(DashMap::new()),
            version: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Bump the version counter after any write.  The UI uses this to decide
    /// whether to request a repaint.
    #[inline]
    pub fn touch(&self) {
        self.version
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    #[inline]
    pub fn version(&self) -> u64 {
        self.version.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Shared handle — clone this everywhere.
pub type SharedAppState = Arc<AppState>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Snap a Unix timestamp to the start of its 5-minute window.
pub fn stamp_5m() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs();
    now - (now % 300)
}

pub fn slug_for_ts(ts: u64) -> String {
    format!("btc-updown-5m-{}", ts)
}
