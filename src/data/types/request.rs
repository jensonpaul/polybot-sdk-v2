
#![allow(
    clippy::module_name_repetitions,
    reason = "Request suffix is intentional for clarity"
)]

use bon::Builder;
use serde::Serialize;
use serde_with::{StringWithSeparator, formats::CommaSeparator, serde_as, skip_serializing_none};

use super::{
    ActivitySortBy, ActivityType, BoundedIntError, ClosedPositionSortBy, LeaderboardCategory,
    LeaderboardOrderBy, MarketFilter, PositionSortBy, Side, SortDirection, TimePeriod, TradeFilter,
};
use crate::types::{Address, B256, Decimal};

fn validate_bound(
    value: i32,
    min: i32,
    max: i32,
    param_name: &'static str,
) -> Result<i32, BoundedIntError> {
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(BoundedIntError::new(value, min, max, param_name))
    }
}

/// Request parameters for the `/positions` endpoint.
///
/// Fetches current (open) positions for a user. Positions represent holdings
/// of outcome tokens in prediction markets.
///
/// # Required Parameters
///
/// - `user`: The Ethereum address of the user whose positions to retrieve.
///
/// # Optional Parameters
///
/// - `filter`: Filter by specific markets (condition IDs) or events.
///   Cannot specify both markets and events.
/// - `size_threshold`: Minimum position size to include (default: 1).
/// - `redeemable`: If true, only return positions that can be redeemed.
/// - `mergeable`: If true, only return positions that can be merged.
/// - `limit`: Maximum positions to return (0-500, default: 100).
/// - `offset`: Pagination offset (0-10000, default: 0).
/// - `sort_by`: Sort criteria (default: TOKENS).
/// - `sort_direction`: Sort order (default: DESC).
/// - `title`: Filter by market title substring.
///
/// # Example
///
/// ```
/// use polymarket_client_sdk_v2::types::address;
/// use polymarket_client_sdk_v2::data::{types::request::PositionsRequest, types::{PositionSortBy, SortDirection}};
///
/// let request = PositionsRequest::builder()
///     .user(address!("56687bf447db6ffa42ffe2204a05edaa20f55839"))
///     .sort_by(PositionSortBy::CashPnl)
///     .sort_direction(SortDirection::Desc)
///     .build();
/// ```
#[skip_serializing_none]
#[derive(Debug, Clone, Builder, Serialize)]
#[non_exhaustive]
pub struct PositionsRequest {
    /// User address (required).
    #[builder(into)]
    pub user: Address,
    /// Filter by markets or events. Mutually exclusive options.
    #[serde(flatten, skip_serializing_if = "filter_is_none_or_empty")]
    pub filter: Option<MarketFilter>,
    /// Minimum position size to include (default: 1).
    #[serde(rename = "sizeThreshold")]
    pub size_threshold: Option<Decimal>,
    /// Only return positions that can be redeemed (default: false).
    pub redeemable: Option<bool>,
    /// Only return positions that can be merged (default: false).
    pub mergeable: Option<bool>,
    /// Maximum number of positions to return (0-500, default: 100).
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 0, 500, "limit") })]
    pub limit: Option<i32>,
    /// Pagination offset (0-10000, default: 0).
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 0, 10000, "offset") })]
    pub offset: Option<i32>,
    /// Sort criteria (default: TOKENS).
    #[serde(rename = "sortBy")]
    pub sort_by: Option<PositionSortBy>,
    /// Sort direction (default: DESC).
    #[serde(rename = "sortDirection")]
    pub sort_direction: Option<SortDirection>,
    /// Filter by market title substring (max 100 chars).
    #[builder(into)]
    pub title: Option<String>,
}

#[expect(clippy::ref_option, reason = "Need an explicit reference for serde")]
fn filter_is_none_or_empty(f: &Option<MarketFilter>) -> bool {
    match f {
        None => true,
        Some(MarketFilter::Markets(v)) => v.is_empty(),
        Some(MarketFilter::EventIds(v)) => v.is_empty(),
    }
}

/// Request parameters for the `/trades` endpoint.
///
/// Fetches trade history for a user or markets. Trades represent executed
/// orders where outcome tokens were bought or sold.
///
/// # Optional Parameters
///
/// - `user`: Filter by user address.
/// - `filter`: Filter by specific markets (condition IDs) or events.
/// - `limit`: Maximum trades to return (0-10000, default: 100).
/// - `offset`: Pagination offset (0-10000, default: 0).
/// - `taker_only`: If true, only return taker trades (default: true).
/// - `trade_filter`: Filter by minimum trade size (cash or tokens).
/// - `side`: Filter by trade side (BUY or SELL).
///
/// # Example
///
/// ```
/// use polymarket_client_sdk_v2::types::address;
/// use polymarket_client_sdk_v2::data::{types::request::TradesRequest, types::{Side, TradeFilter}};
/// use rust_decimal_macros::dec;
///
/// let request = TradesRequest::builder()
///     .user(address!("56687bf447db6ffa42ffe2204a05edaa20f55839"))
///     .side(Side::Buy)
///     .trade_filter(TradeFilter::cash(dec!(100)).unwrap())
///     .build();
/// ```
#[skip_serializing_none]
#[derive(Debug, Clone, Builder, Default, Serialize)]
#[non_exhaustive]
pub struct TradesRequest {
    /// Filter by user address.
    #[builder(into)]
    pub user: Option<Address>,
    /// Filter by markets or events. Mutually exclusive options.
    #[serde(flatten)]
    pub filter: Option<MarketFilter>,
    /// Maximum number of trades to return (0-10000, default: 100).
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 0, 10000, "limit") })]
    pub limit: Option<i32>,
    /// Pagination offset (0-10000, default: 0).
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 0, 10000, "offset") })]
    pub offset: Option<i32>,
    /// Only return taker trades (default: true).
    #[serde(rename = "takerOnly")]
    pub taker_only: Option<bool>,
    /// Filter by minimum trade size. Must provide both type and amount.
    #[serde(flatten)]
    pub trade_filter: Option<TradeFilter>,
    /// Filter by trade side (BUY or SELL).
    pub side: Option<Side>,
}

/// Request parameters for the `/activity` endpoint.
///
/// Fetches on-chain activity for a user, including trades, splits, merges,
/// redemptions, rewards, and conversions.
///
/// # Required Parameters
///
/// - `user`: The Ethereum address of the user whose activity to retrieve.
///
/// # Optional Parameters
///
/// - `filter`: Filter by specific markets (condition IDs) or events.
/// - `activity_types`: Filter by activity types (TRADE, SPLIT, MERGE, etc.).
/// - `limit`: Maximum activities to return (0-500, default: 100).
/// - `offset`: Pagination offset (0-10000, default: 0).
/// - `start`: Start timestamp filter (Unix timestamp).
/// - `end`: End timestamp filter (Unix timestamp).
/// - `sort_by`: Sort criteria (default: TIMESTAMP).
/// - `sort_direction`: Sort order (default: DESC).
/// - `side`: Filter by trade side (only applies to TRADE activities).
///
/// # Example
///
/// ```
/// use polymarket_client_sdk_v2::types::address;
/// use polymarket_client_sdk_v2::data::{types::request::ActivityRequest, types::ActivityType};
///
/// let request = ActivityRequest::builder()
///     .user(address!("56687bf447db6ffa42ffe2204a05edaa20f55839"))
///     .activity_types(vec![ActivityType::Trade, ActivityType::Redeem])
///     .build();
/// ```
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Clone, Builder, Serialize)]
#[non_exhaustive]
pub struct ActivityRequest {
    /// User address (required).
    #[builder(into)]
    pub user: Address,
    /// Filter by markets or events. Mutually exclusive options.
    #[serde(flatten)]
    pub filter: Option<MarketFilter>,
    /// Filter by activity types.
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, ActivityType>")]
    #[builder(default)]
    #[serde(rename = "type", skip_serializing_if = "Vec::is_empty")]
    pub activity_types: Vec<ActivityType>,
    /// Maximum number of activities to return (0-500, default: 100).
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 0, 500, "limit") })]
    pub limit: Option<i32>,
    /// Pagination offset (0-10000, default: 0).
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 0, 10000, "offset") })]
    pub offset: Option<i32>,
    /// Start timestamp filter (Unix timestamp, minimum: 0).
    pub start: Option<u64>,
    /// End timestamp filter (Unix timestamp, minimum: 0).
    pub end: Option<u64>,
    /// Sort criteria (default: TIMESTAMP).
    #[serde(rename = "sortBy")]
    pub sort_by: Option<ActivitySortBy>,
    /// Sort direction (default: DESC).
    #[serde(rename = "sortDirection")]
    pub sort_direction: Option<SortDirection>,
    /// Filter by trade side (only applies to TRADE activities).
    pub side: Option<Side>,
}

/// Request parameters for the `/holders` endpoint.
///
/// Fetches top token holders for specified markets. Returns holders grouped
/// by token (outcome) for each market.
///
/// # Required Parameters
///
/// - `markets`: List of condition IDs (market identifiers) to query.
///
/// # Optional Parameters
///
/// - `limit`: Maximum holders to return per token (0-20, default: 20).
/// - `min_balance`: Minimum balance to include (0-999999, default: 1).
///
/// # Example
///
/// ```
/// use polymarket_client_sdk_v2::data::types::request::HoldersRequest;
/// use polymarket_client_sdk_v2::types::b256;
///
/// let request = HoldersRequest::builder()
///     .markets(vec![b256!("dd22472e552920b8438158ea7238bfadfa4f736aa4cee91a6b86c39ead110917")])
///     .build();
/// ```
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Clone, Builder, Serialize)]
#[non_exhaustive]
pub struct HoldersRequest {
    /// Condition IDs of markets to query (required).
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, B256>")]
    #[serde(rename = "market", skip_serializing_if = "Vec::is_empty")]
    pub markets: Vec<B256>,
    /// Maximum holders to return per token (0-20, default: 20).
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 0, 20, "limit") })]
    pub limit: Option<i32>,
    /// Minimum balance to include (0-999999, default: 1).
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 0, 999_999, "min_balance") })]
    #[serde(rename = "minBalance")]
    pub min_balance: Option<i32>,
}

/// Request parameters for the `/traded` endpoint.
///
/// Fetches the total count of unique markets a user has traded.
///
/// # Required Parameters
///
/// - `user`: The Ethereum address of the user to query.
#[derive(Debug, Clone, Builder, Serialize)]
#[non_exhaustive]
pub struct TradedRequest {
    /// User address (required).
    #[builder(into)]
    pub user: Address,
}

/// Request parameters for the `/value` endpoint.
///
/// Fetches the total value of a user's positions, optionally filtered by markets.

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Clone, Builder, Serialize)]
#[non_exhaustive]
pub struct ValueRequest {
    
    #[builder(into)]
    pub user: Address,
    
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, B256>")]
    #[builder(default)]
    #[serde(rename = "market", skip_serializing_if = "Vec::is_empty")]
    pub markets: Vec<B256>,
}

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Clone, Builder, Default, Serialize)]
#[non_exhaustive]
pub struct OpenInterestRequest {
    
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, B256>")]
    #[builder(default)]
    #[serde(rename = "market", skip_serializing_if = "Vec::is_empty")]
    pub markets: Vec<B256>,
}

#[derive(Debug, Clone, Builder, Serialize)]
#[non_exhaustive]
pub struct LiveVolumeRequest {
    
    pub id: u64,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Builder, Serialize)]
#[non_exhaustive]
pub struct ClosedPositionsRequest {
    
    #[builder(into)]
    pub user: Address,
    
    #[serde(flatten)]
    pub filter: Option<MarketFilter>,
    
    #[builder(into)]
    pub title: Option<String>,
    
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 0, 50, "limit") })]
    pub limit: Option<i32>,
    
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 0, 100_000, "offset") })]
    pub offset: Option<i32>,
    
    #[serde(rename = "sortBy")]
    pub sort_by: Option<ClosedPositionSortBy>,
    
    #[serde(rename = "sortDirection")]
    pub sort_direction: Option<SortDirection>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Builder, Default, Serialize)]
#[non_exhaustive]
pub struct BuilderLeaderboardRequest {
    
    #[serde(rename = "timePeriod")]
    pub time_period: Option<TimePeriod>,
    
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 0, 50, "limit") })]
    pub limit: Option<i32>,
    
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 0, 1000, "offset") })]
    pub offset: Option<i32>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Builder, Default, Serialize)]
#[non_exhaustive]
pub struct BuilderVolumeRequest {
    
    #[serde(rename = "timePeriod")]
    pub time_period: Option<TimePeriod>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Builder, Default, Serialize)]
#[non_exhaustive]
pub struct TraderLeaderboardRequest {
    
    pub category: Option<LeaderboardCategory>,
    
    #[serde(rename = "timePeriod")]
    pub time_period: Option<TimePeriod>,
    
    #[serde(rename = "orderBy")]
    pub order_by: Option<LeaderboardOrderBy>,
    
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 1, 50, "limit") })]
    pub limit: Option<i32>,
    
    #[builder(with = |v: i32| -> Result<_, BoundedIntError> { validate_bound(v, 0, 1000, "offset") })]
    pub offset: Option<i32>,
    
    #[builder(into)]
    pub user: Option<Address>,
    
    #[builder(into)]
    #[serde(rename = "userName")]
    pub user_name: Option<String>,
}
