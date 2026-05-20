#![allow(
    clippy::module_name_repetitions,
    reason = "Request suffix is intentional for clarity"
)]

use bon::Builder;
use chrono::NaiveDate;
use serde::{Serialize, Serializer};
use serde_with::{
    DisplayFromStr, StringWithSeparator, formats::CommaSeparator, serde_as, skip_serializing_none,
};
#[cfg(feature = "rfq")]
use {
    crate::clob::types::{RfqSortBy, RfqSortDir, RfqState},
    crate::{Timestamp, auth::ApiKey, types::Decimal},
};

use crate::clob::types::{AssetType, Side, SignatureType, TimeRange};
use crate::types::U256;
use crate::types::{Address, B256};

#[serde_as]
#[non_exhaustive]
#[derive(Debug, Serialize, Builder)]
#[builder(on(String, into))]
pub struct MidpointRequest {
    #[serde_as(as = "DisplayFromStr")]
    pub token_id: U256,
}

#[serde_as]
#[non_exhaustive]
#[derive(Debug, Serialize, Builder)]
#[builder(on(String, into))]
pub struct PriceRequest {
    #[serde_as(as = "DisplayFromStr")]
    pub token_id: U256,
    pub side: Side,
}

#[non_exhaustive]
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize, Builder)]
#[builder(on(String, into))]
pub struct SpreadRequest {
    #[serde_as(as = "DisplayFromStr")]
    pub token_id: U256,
    pub side: Option<Side>,
}

#[non_exhaustive]
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize, Builder)]
#[builder(on(String, into))]
pub struct OrderBookSummaryRequest {
    #[serde_as(as = "DisplayFromStr")]
    pub token_id: U256,
    pub side: Option<Side>,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Serialize, Builder)]
#[builder(on(String, into))]
pub struct LastTradePriceRequest {
    #[serde_as(as = "DisplayFromStr")]
    pub token_id: U256,
}

#[serde_as]
#[non_exhaustive]
#[skip_serializing_none]
#[derive(Debug, Serialize, Builder)]
#[builder(on(String, into))]
pub struct PriceHistoryRequest {
    
    #[serde_as(as = "DisplayFromStr")]
    pub market: U256,
    
    #[serde(flatten)]
    #[builder(into)]
    pub time_range: TimeRange,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fidelity: Option<u32>,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Default, Serialize, Builder)]
#[builder(on(String, into))]
pub struct CancelMarketOrderRequest {
    
    pub market: Option<B256>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub asset_id: Option<U256>,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Default, Clone, Builder, Serialize)]
#[builder(on(String, into))]
pub struct TradesRequest {
    pub id: Option<String>,
    #[serde(rename = "taker")]
    pub taker_address: Option<Address>,
    #[serde(rename = "maker")]
    pub maker_address: Option<Address>,
    
    pub market: Option<B256>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub asset_id: Option<U256>,
    pub before: Option<i64>,
    pub after: Option<i64>,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Default, Serialize, Builder)]
#[builder(on(String, into))]
pub struct OrdersRequest {
    #[serde(rename = "id")]
    pub order_id: Option<String>,
    
    pub market: Option<B256>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub asset_id: Option<U256>,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Default, Serialize, Builder)]
pub struct DeleteNotificationsRequest {
    #[serde(rename = "ids", skip_serializing_if = "Vec::is_empty")]
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, String>")]
    #[builder(default)]
    pub notification_ids: Vec<String>,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Default, Clone, Builder, Serialize)]
#[builder(on(String, into))]
pub struct BalanceAllowanceRequest {
    pub asset_type: AssetType,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub token_id: Option<U256>,
    pub signature_type: Option<SignatureType>,
}

pub type UpdateBalanceAllowanceRequest = BalanceAllowanceRequest;

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Builder)]
#[builder(on(String, into))]
pub struct UserRewardsEarningRequest {
    pub date: NaiveDate,
    #[builder(default)]
    pub order_by: String,
    #[builder(default)]
    pub position: String,
    #[builder(default)]
    pub no_competition: bool,
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Asset {
    Usdc,
    Asset(U256),
}

impl Serialize for Asset {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Asset::Usdc => serializer.serialize_str("0"),
            Asset::Asset(a) => serializer.collect_str(a),
        }
    }
}

#[cfg(feature = "rfq")]
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct CreateRfqRequestRequest {
    
    pub asset_in: Asset,
    
    pub asset_out: Asset,
    
    pub amount_in: Decimal,
    
    pub amount_out: Decimal,
    
    pub user_type: SignatureType,
}

#[cfg(feature = "rfq")]
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Builder)]
#[serde(rename_all = "camelCase")]
#[builder(on(String, into))]
pub struct CancelRfqRequestRequest {
    
    pub request_id: String,
}

#[cfg(feature = "rfq")]
#[non_exhaustive]
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Clone, Default, Serialize, Builder)]
#[serde(rename_all = "camelCase")]
#[builder(on(String, into))]
pub struct RfqRequestsRequest {
    
    pub offset: Option<String>,
    
    pub limit: Option<u32>,
    
    pub state: Option<RfqState>,
    
    #[serde(rename = "requestIds", skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub request_ids: Vec<String>,
    
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, B256>")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub markets: Vec<B256>,
    
    pub size_min: Option<Decimal>,
    
    pub size_max: Option<Decimal>,
    
    pub size_usdc_min: Option<Decimal>,
    
    pub size_usdc_max: Option<Decimal>,
    
    pub price_min: Option<Decimal>,
    
    pub price_max: Option<Decimal>,
    
    pub sort_by: Option<RfqSortBy>,
    
    pub sort_dir: Option<RfqSortDir>,
}

#[cfg(feature = "rfq")]
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Builder)]
#[serde(rename_all = "camelCase")]
#[builder(on(String, into))]
pub struct CreateRfqQuoteRequest {
    
    pub request_id: String,
    
    pub asset_in: Asset,
    
    pub asset_out: Asset,
    
    pub amount_in: Decimal,
    
    pub amount_out: Decimal,
    
    pub user_type: SignatureType,
}

#[cfg(feature = "rfq")]
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Builder)]
#[serde(rename_all = "camelCase")]
#[builder(on(String, into))]
pub struct CancelRfqQuoteRequest {
    
    pub quote_id: String,
}

#[cfg(feature = "rfq")]
#[non_exhaustive]
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Clone, Default, Serialize, Builder)]
#[serde(rename_all = "camelCase")]
#[builder(on(String, into))]
pub struct RfqQuotesRequest {
    
    pub offset: Option<String>,
    
    pub limit: Option<u32>,
    
    pub state: Option<RfqState>,
    
    #[serde(rename = "quoteIds", skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub quote_ids: Vec<String>,
    
    #[serde(rename = "requestIds", skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub request_ids: Vec<String>,
    
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, B256>")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub markets: Vec<B256>,
    
    pub size_min: Option<Decimal>,
    
    pub size_max: Option<Decimal>,
    
    pub size_usdc_min: Option<Decimal>,
    
    pub size_usdc_max: Option<Decimal>,
    
    pub price_min: Option<Decimal>,
    
    pub price_max: Option<Decimal>,
    
    pub sort_by: Option<RfqSortBy>,
    
    pub sort_dir: Option<RfqSortDir>,
}

#[cfg(feature = "rfq")]
#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Serialize, Builder)]
#[serde(rename_all = "camelCase")]
#[builder(on(String, into))]
pub struct AcceptRfqQuoteRequest {
    
    pub request_id: String,
    
    pub quote_id: String,
    
    pub maker_amount: Decimal,
    
    pub taker_amount: Decimal,
    
    #[serde_as(as = "DisplayFromStr")]
    pub token_id: U256,
    
    pub maker: Address,
    
    pub signer: Address,
    
    pub taker: Address,
    
    pub nonce: u64,
    
    pub expiration: i64,
    
    pub side: Side,
    
    pub fee_rate_bps: u64,
    
    pub signature: String,
    
    pub salt: String,
    
    pub owner: ApiKey,
}

#[cfg(feature = "rfq")]
#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Serialize, Builder)]
#[serde(rename_all = "camelCase")]
#[builder(on(String, into))]
pub struct ApproveRfqOrderRequest {
    
    pub request_id: String,
    
    pub quote_id: String,
    
    pub maker_amount: Decimal,
    
    pub taker_amount: Decimal,
    
    #[serde_as(as = "DisplayFromStr")]
    pub token_id: U256,
    
    pub maker: Address,
    
    pub signer: Address,
    
    pub taker: Address,
    
    pub nonce: u64,
    
    pub expiration: Timestamp,
    
    pub side: Side,
    
    pub fee_rate_bps: u64,
    
    pub signature: String,
    
    pub salt: String,
    
    pub owner: ApiKey,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToQueryParams as _;
    use crate::types::b256;

    #[test]
    fn trades_request_as_params_should_succeed() {
        let market = b256!("0000000000000000000000000000000000000000000000000000000000010000");
        let request = TradesRequest::builder()
            .market(market)
            .asset_id(U256::from(100))
            .id("aa-bb")
            .maker_address(Address::ZERO)
            .build();

        assert_eq!(
            request.query_params(None),
            "?id=aa-bb&maker=0x0000000000000000000000000000000000000000&market=0x0000000000000000000000000000000000000000000000000000000000010000&asset_id=100"
        );
        assert_eq!(
            request.query_params(Some("1")),
            "?id=aa-bb&maker=0x0000000000000000000000000000000000000000&market=0x0000000000000000000000000000000000000000000000000000000000010000&asset_id=100&next_cursor=1"
        );
    }

    #[test]
    fn orders_request_as_params_should_succeed() {
        let market = b256!("0000000000000000000000000000000000000000000000000000000000010000");
        let request = OrdersRequest::builder()
            .market(market)
            .asset_id(U256::from(100))
            .order_id("aa-bb")
            .build();

        assert_eq!(
            request.query_params(None),
            "?id=aa-bb&market=0x0000000000000000000000000000000000000000000000000000000000010000&asset_id=100"
        );
        assert_eq!(
            request.query_params(Some("1")),
            "?id=aa-bb&market=0x0000000000000000000000000000000000000000000000000000000000010000&asset_id=100&next_cursor=1"
        );
    }

    #[test]
    fn delete_notifications_request_as_params_should_succeed() {
        let empty_request = DeleteNotificationsRequest::builder().build();
        let request = DeleteNotificationsRequest::builder()
            .notification_ids(vec!["1".to_owned(), "2".to_owned()])
            .build();

        assert_eq!(empty_request.query_params(None), "");
        assert_eq!(request.query_params(None), "?ids=1%2C2");
    }

    #[test]
    fn balance_allowance_request_as_params_should_succeed() {
        let request = BalanceAllowanceRequest::builder()
            .asset_type(AssetType::Collateral)
            .token_id(U256::from(1))
            .signature_type(SignatureType::Eoa)
            .build();

        assert_eq!(
            request.query_params(None),
            "?asset_type=COLLATERAL&token_id=1&signature_type=0"
        );
    }

    #[test]
    fn user_rewards_earning_request_as_params_should_succeed() {
        let request = UserRewardsEarningRequest::builder()
            .date(NaiveDate::MIN)
            .build();

        assert_eq!(
            request.query_params(Some("1")),
            "?date=-262143-01-01&order_by=&position=&no_competition=false&next_cursor=1"
        );
    }
}
