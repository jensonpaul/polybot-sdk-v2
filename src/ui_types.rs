use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub enum LocalOrderStatus {
    Open,
    PartiallyFilled { filled: String },
    FullyFilled,
    Canceled,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct TrackedOrder {
    pub id: String,
    pub side: String,
    pub token: String,
    pub price: String,
    pub size: String,
    pub status: LocalOrderStatus,
    pub size_matched: String,
    pub inline_sell_price: String,
    pub inline_sell_size: String,
}

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
    pub expires_at: Instant,
}

#[derive(Clone, Debug)]
pub struct WindowGroup {
    pub timestamp_5m: u64,
    pub slug: String,
    pub is_expanded: bool,
    pub orders: Vec<TrackedOrder>,
}

// Bounded UI Input signals passed from egui down to your network service worker
#[derive(Debug, Clone)]
pub enum UiCommand {
    InitializeClient {
        token: String,
    },
    PlaceLimit {
        side: String,
        token: String,
        price: String,
        size: String,
        window_ts: u64,
    },
    PlaceMarket {
        side: String,
        token: String,
        usdc: Option<String>,
        shares: Option<String>,
        order_type: Option<String>,
        window_ts: u64,
    },
    CheckStatus {
        order_id: String,
        window_ts: u64,
    },
    CancelIndividual {
        order_id: String,
        window_ts: u64,
    },
    CancelAllInWindow {
        window_ts: u64,
    },
}

// Responses broadcast back from the background engine worker up to the UI state layer
#[derive(Debug, Clone)]
pub enum WorkerUpdate {
    OrderAdded {
        window_ts: u64,
        order: TrackedOrder,
    },
    OrderUpdated {
        window_ts: u64,
        order_id: String,
        status: LocalOrderStatus,
        matched: String,
    },
    Notify {
        message: String,
        kind: NotificationKind,
    },
}

// Mock structures to maintain compiling interfaces for order requests
pub struct OrderLimitRequest {
    pub side: String,
    pub token: String,
    pub price: String,
    pub size: String,
}

pub struct OrderMarketRequest {
    pub side: String,
    pub token: String,
    pub usdc: Option<String>,
    pub shares: Option<String>,
    pub order_type: Option<String>,
}