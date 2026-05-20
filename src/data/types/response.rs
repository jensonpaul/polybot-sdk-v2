
use bon::Builder;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Deserializer};
use serde_with::{DefaultOnNull, DisplayFromStr, NoneAsEmptyString, serde_as};

use super::{ActivityType, Side};
use crate::types::{Address, B256, Decimal, U256};

fn deserialize_optional_side<'de, D>(deserializer: D) -> Result<Option<Side>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => match s.to_uppercase().as_str() {
            "BUY" => Ok(Some(Side::Buy)),
            "SELL" => Ok(Some(Side::Sell)),
            _ => Ok(None),
        },
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum Market {
    
    #[serde(alias = "global", alias = "GLOBAL")]
    Global,
    
    #[serde(untagged)]
    Market(B256),
}

#[derive(Debug, Clone, Deserialize, Builder)]
#[non_exhaustive]
pub struct Health {
    
    pub data: String,
}

#[derive(Debug, Clone, Deserialize, Builder)]
#[non_exhaustive]
pub struct ApiError {
    
    pub error: String,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Position {
    
    pub proxy_wallet: Address,
    
    pub asset: U256,
    
    pub condition_id: B256,
    
    pub size: Decimal,
    
    pub avg_price: Decimal,
    
    pub initial_value: Decimal,
    
    pub current_value: Decimal,
    
    pub cash_pnl: Decimal,
    
    pub percent_pnl: Decimal,
    
    pub total_bought: Decimal,
    
    pub realized_pnl: Decimal,
    
    pub percent_realized_pnl: Decimal,
    
    pub cur_price: Decimal,
    
    pub redeemable: bool,
    
    pub mergeable: bool,
    
    pub title: String,
    
    pub slug: String,
    
    pub icon: String,
    
    pub event_slug: String,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub event_id: Option<String>,
    
    pub outcome: String,
    
    pub outcome_index: i32,
    
    pub opposite_outcome: String,
    
    pub opposite_asset: U256,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub end_date: Option<NaiveDate>,
    
    pub negative_risk: bool,
}

#[derive(Debug, Clone, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ClosedPosition {
    
    pub proxy_wallet: Address,
    
    pub asset: U256,
    
    pub condition_id: B256,
    
    pub avg_price: Decimal,
    
    pub total_bought: Decimal,
    
    pub realized_pnl: Decimal,
    
    pub cur_price: Decimal,
    
    pub timestamp: i64,
    
    pub title: String,
    
    pub slug: String,
    
    pub icon: String,
    
    pub event_slug: String,
    
    pub outcome: String,
    
    pub outcome_index: i32,
    
    pub opposite_outcome: String,
    
    pub opposite_asset: U256,
    
    pub end_date: DateTime<Utc>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Trade {
    
    pub proxy_wallet: Address,
    
    pub side: Side,
    
    pub asset: U256,
    
    pub condition_id: B256,
    
    pub size: Decimal,
    
    pub price: Decimal,
    
    pub timestamp: i64,
    
    pub title: String,
    
    pub slug: String,
    
    pub icon: String,
    
    pub event_slug: String,
    
    pub outcome: String,
    
    pub outcome_index: i32,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub name: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub pseudonym: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub bio: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub profile_image: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub profile_image_optimized: Option<String>,
    
    pub transaction_hash: B256,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Activity {
    
    pub proxy_wallet: Address,
    
    pub timestamp: i64,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub condition_id: Option<B256>,
    
    #[serde(rename = "type")]
    pub activity_type: ActivityType,
    
    pub size: Decimal,
    
    pub usdc_size: Decimal,
    
    pub transaction_hash: B256,
    
    pub price: Option<Decimal>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub asset: Option<U256>,
    
    #[serde(default, deserialize_with = "deserialize_optional_side")]
    pub side: Option<Side>,
    
    pub outcome_index: Option<i32>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub title: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub slug: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub icon: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub event_slug: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub outcome: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub name: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub pseudonym: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub bio: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub profile_image: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub profile_image_optimized: Option<String>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Holder {
    
    pub proxy_wallet: Address,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub bio: Option<String>,
    
    pub asset: U256,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub pseudonym: Option<String>,
    
    pub amount: Decimal,
    
    pub display_username_public: Option<bool>,
    
    pub outcome_index: i32,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub name: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub profile_image: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub profile_image_optimized: Option<String>,
    
    pub verified: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Builder)]
#[non_exhaustive]
pub struct MetaHolder {
    
    pub token: U256,
    
    pub holders: Vec<Holder>,
}

#[derive(Debug, Clone, Deserialize, Builder)]
#[non_exhaustive]
pub struct Traded {
    
    pub user: Address,
    
    pub traded: i32,
}

#[derive(Debug, Clone, Deserialize, Builder)]
#[non_exhaustive]
pub struct Value {
    
    pub user: Address,
    
    pub value: Decimal,
}

#[derive(Debug, Clone, Deserialize, Builder)]
#[non_exhaustive]
pub struct OpenInterest {
    
    pub market: Market,
    
    pub value: Decimal,
}

#[derive(Debug, Clone, Deserialize, Builder)]
#[non_exhaustive]
pub struct MarketVolume {
    
    pub market: Market,
    
    pub value: Decimal,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
#[non_exhaustive]
pub struct LiveVolume {
    
    pub total: Decimal,
    
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub markets: Vec<MarketVolume>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct BuilderLeaderboardEntry {
    
    #[serde_as(as = "DisplayFromStr")]
    pub rank: i32,
    
    pub builder: String,
    
    pub volume: Decimal,
    
    pub active_users: i32,
    
    pub verified: bool,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub builder_logo: Option<String>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct BuilderVolumeEntry {
    
    pub dt: DateTime<Utc>,
    
    pub builder: String,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub builder_logo: Option<String>,
    
    pub verified: bool,
    
    pub volume: Decimal,
    
    pub active_users: i32,
    
    #[serde_as(as = "DisplayFromStr")]
    pub rank: i32,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TraderLeaderboardEntry {
    
    #[serde_as(as = "DisplayFromStr")]
    pub rank: i32,
    
    pub proxy_wallet: Address,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub user_name: Option<String>,
    
    pub vol: Decimal,
    
    pub pnl: Decimal,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub profile_image: Option<String>,
    
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub x_username: Option<String>,
    
    pub verified_badge: Option<bool>,
}
