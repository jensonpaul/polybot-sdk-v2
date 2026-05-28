use std::fmt;

use serde::de::StdError;
use serde::{Deserialize, Serialize};
use serde_with::{StringWithSeparator, formats::CommaSeparator, serde_as};

use crate::types::{B256, Decimal};

pub mod request;
pub mod response;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, strum_macros::Display)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
#[non_exhaustive]
pub enum Side {
    
    Buy,
    
    Sell,
    
    #[serde(untagged)]
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, strum_macros::Display)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
#[non_exhaustive]
pub enum ActivityType {
    
    Trade,
    
    Split,
    
    Merge,
    
    Redeem,
    
    Reward,
    
    Conversion,
    
    Yield,
    
    MakerRebate,
    
    #[serde(untagged)]
    Unknown(String),
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, strum_macros::Display,
)]
#[non_exhaustive]
pub enum PositionSortBy {
    
    #[serde(rename = "CURRENT")]
    #[strum(serialize = "CURRENT")]
    Current,
    
    #[serde(rename = "INITIAL")]
    #[strum(serialize = "INITIAL")]
    Initial,
    
    #[default]
    #[serde(rename = "TOKENS")]
    #[strum(serialize = "TOKENS")]
    Tokens,
    
    #[serde(rename = "CASHPNL")]
    #[strum(serialize = "CASHPNL")]
    CashPnl,
    
    #[serde(rename = "PERCENTPNL")]
    #[strum(serialize = "PERCENTPNL")]
    PercentPnl,
    
    #[serde(rename = "TITLE")]
    #[strum(serialize = "TITLE")]
    Title,
    
    #[serde(rename = "RESOLVING")]
    #[strum(serialize = "RESOLVING")]
    Resolving,
    
    #[serde(rename = "PRICE")]
    #[strum(serialize = "PRICE")]
    Price,
    
    #[serde(rename = "AVGPRICE")]
    #[strum(serialize = "AVGPRICE")]
    AvgPrice,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, strum_macros::Display,
)]
#[non_exhaustive]
pub enum ClosedPositionSortBy {
    
    #[default]
    #[serde(rename = "REALIZEDPNL")]
    #[strum(serialize = "REALIZEDPNL")]
    RealizedPnl,
    
    #[serde(rename = "TITLE")]
    #[strum(serialize = "TITLE")]
    Title,
    
    #[serde(rename = "PRICE")]
    #[strum(serialize = "PRICE")]
    Price,
    
    #[serde(rename = "AVGPRICE")]
    #[strum(serialize = "AVGPRICE")]
    AvgPrice,
    
    #[serde(rename = "TIMESTAMP")]
    #[strum(serialize = "TIMESTAMP")]
    Timestamp,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, strum_macros::Display,
)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
#[non_exhaustive]
pub enum ActivitySortBy {
    
    #[default]
    Timestamp,
    
    Tokens,
    
    Cash,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, strum_macros::Display,
)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
#[non_exhaustive]
pub enum SortDirection {
    
    Asc,
    
    #[default]
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum_macros::Display)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
#[non_exhaustive]
pub enum FilterType {
    
    Cash,
    
    Tokens,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, strum_macros::Display,
)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
#[non_exhaustive]
pub enum TimePeriod {
    
    #[default]
    Day,
    
    Week,
    
    Month,
    
    All,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, strum_macros::Display,
)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
#[non_exhaustive]
pub enum LeaderboardCategory {
    
    #[default]
    Overall,
    
    Politics,
    
    Sports,
    
    Crypto,
    
    Culture,
    
    Mentions,
    
    Weather,
    
    Economics,
    
    Tech,
    
    Finance,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, strum_macros::Display,
)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
#[non_exhaustive]
pub enum LeaderboardOrderBy {
    
    #[default]
    Pnl,
    
    Vol,
}

#[serde_as]
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub enum MarketFilter {
    
    #[serde(rename = "market")]
    Markets(#[serde_as(as = "StringWithSeparator::<CommaSeparator, B256>")] Vec<B256>),
    
    #[serde(rename = "eventId")]
    EventIds(#[serde_as(as = "StringWithSeparator::<CommaSeparator, String>")] Vec<String>),
}

impl MarketFilter {
    
    #[must_use]
    pub fn markets<I: IntoIterator<Item = B256>>(ids: I) -> Self {
        Self::Markets(ids.into_iter().collect())
    }

    #[must_use]
    pub fn event_ids<I: IntoIterator<Item = String>>(ids: I) -> Self {
        Self::EventIds(ids.into_iter().collect())
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub struct BoundedIntError {
    
    pub value: i32,
    
    pub min: i32,
    
    pub max: i32,
    
    pub param_name: &'static str,
}

impl BoundedIntError {
    /// Creates a new `BoundedIntError`.
    #[must_use]
    pub const fn new(value: i32, min: i32, max: i32, param_name: &'static str) -> Self {
        Self {
            value,
            min,
            max,
            param_name,
        }
    }
}

impl fmt::Display for BoundedIntError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} must be between {} and {} (got {})",
            self.param_name, self.min, self.max, self.value
        )
    }
}

impl StdError for BoundedIntError {}

/// A filter for minimum trade size.
///
/// Used to filter trades by a minimum value, either in USDC (cash) or tokens.
/// Both `filter_type` and `filter_amount` must be provided together to the API.
///
/// # Example
///
/// ```
/// use polymarket_client_sdk_v2::data::types::TradeFilter;
/// use rust_decimal_macros::dec;
///
/// // Filter trades with at least $100 USDC value
/// let filter = TradeFilter::cash(dec!(100)).unwrap();
///
/// // Filter trades with at least 50 tokens
/// let filter = TradeFilter::tokens(dec!(50)).unwrap();
/// ```
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TradeFilter {
    /// The type of filter (cash or tokens).
    pub filter_type: FilterType,
    /// The minimum amount to filter by (must be >= 0).
    pub filter_amount: Decimal,
}

impl TradeFilter {
    /// Creates a new trade filter with the specified type and amount.
    ///
    /// # Errors
    ///
    /// Returns [`TradeFilterError`] if the amount is negative.
    pub fn new(filter_type: FilterType, filter_amount: Decimal) -> Result<Self, TradeFilterError> {
        if filter_amount.is_sign_negative() {
            return Err(TradeFilterError::NegativeAmount(filter_amount));
        }
        Ok(Self {
            filter_type,
            filter_amount,
        })
    }

    /// Creates a cash (USDC) value filter.
    ///
    /// # Errors
    ///
    /// Returns [`TradeFilterError`] if the amount is negative.
    pub fn cash(amount: Decimal) -> Result<Self, TradeFilterError> {
        Self::new(FilterType::Cash, amount)
    }

    /// Creates a token quantity filter.
    ///
    /// # Errors
    ///
    /// Returns [`TradeFilterError`] if the amount is negative.
    pub fn tokens(amount: Decimal) -> Result<Self, TradeFilterError> {
        Self::new(FilterType::Tokens, amount)
    }
}

/// Error type for invalid trade filter values.
#[derive(Debug)]
#[non_exhaustive]
pub enum TradeFilterError {
    /// The filter amount was negative.
    NegativeAmount(Decimal),
}

impl fmt::Display for TradeFilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeAmount(amount) => {
                write!(f, "filter amount must be >= 0 (got {amount})")
            }
        }
    }
}

impl StdError for TradeFilterError {}
