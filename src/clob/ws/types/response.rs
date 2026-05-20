use bon::Builder;
use serde::Deserialize;
use serde_json::Value;
use serde_with::{DefaultOnNull, DisplayFromStr, NoneAsEmptyString, serde_as};
#[cfg(feature = "tracing")]
use tracing::warn;

use crate::auth::ApiKey;
use crate::clob::types::{OrderStatusType, Side, TraderSide};
use crate::clob::ws::interest::MessageInterest;
use crate::error::Kind;
use crate::types::{B256, Decimal, U256};

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event_type")]
pub enum WsMessage {
    
    #[serde(rename = "book")]
    Book(BookUpdate),
    
    #[serde(rename = "price_change")]
    PriceChange(PriceChange),
    
    #[serde(rename = "tick_size_change")]
    TickSizeChange(TickSizeChange),
    
    #[serde(rename = "last_trade_price")]
    LastTradePrice(LastTradePrice),
    
    #[serde(rename = "best_bid_ask")]
    BestBidAsk(BestBidAsk),
    
    #[serde(rename = "new_market")]
    NewMarket(NewMarket),
    
    #[serde(rename = "market_resolved")]
    MarketResolved(MarketResolved),
    
    #[serde(rename = "trade")]
    Trade(TradeMessage),
    
    #[serde(rename = "order")]
    Order(OrderMessage),
}

impl WsMessage {
    
    #[must_use]
    pub const fn is_user(&self) -> bool {
        matches!(self, WsMessage::Trade(_) | WsMessage::Order(_))
    }

    #[must_use]
    pub const fn is_market(&self) -> bool {
        !self.is_user()
    }
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct BookUpdate {
    
    pub asset_id: U256,
    
    pub market: B256,
    
    #[serde_as(as = "DisplayFromStr")]
    pub timestamp: i64,
    
    #[serde(default)]
    pub bids: Vec<OrderBookLevel>,
    
    #[serde(default)]
    pub asks: Vec<OrderBookLevel>,
    
    pub hash: Option<String>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct OrderBookLevel {
    
    pub price: Decimal,
    
    pub size: Decimal,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct PriceChange {
    
    pub market: B256,
    #[serde_as(as = "DisplayFromStr")]
    pub timestamp: i64,
    #[serde(default)]
    pub price_changes: Vec<PriceChangeBatchEntry>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct PriceChangeBatchEntry {
    
    pub asset_id: U256,
    
    pub price: Decimal,
    
    #[serde(default)]
    pub size: Option<Decimal>,
    
    pub side: Side,
    
    #[serde(default)]
    pub hash: Option<String>,
    
    #[serde(default)]
    pub best_bid: Option<Decimal>,
    
    #[serde(default)]
    pub best_ask: Option<Decimal>,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct TickSizeChange {
    
    pub asset_id: U256,
    
    pub market: B256,
    
    pub old_tick_size: Decimal,
    
    pub new_tick_size: Decimal,
    
    #[serde_as(as = "DisplayFromStr")]
    pub timestamp: i64,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct LastTradePrice {
    
    pub asset_id: U256,
    
    pub market: B256,
    
    pub price: Decimal,
    
    pub side: Option<Side>,
    
    pub size: Option<Decimal>,
    
    pub fee_rate_bps: Option<Decimal>,
    
    #[serde_as(as = "DisplayFromStr")]
    pub timestamp: i64,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct BestBidAsk {
    
    pub market: B256,
    
    pub asset_id: U256,
    
    pub best_bid: Decimal,
    
    pub best_ask: Decimal,
    
    pub spread: Decimal,
    
    #[serde_as(as = "DisplayFromStr")]
    pub timestamp: i64,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct NewMarket {
    
    pub id: String,
    
    pub question: String,
    
    pub market: B256,
    
    pub slug: String,
    
    pub description: String,
    
    #[serde(rename = "assets_ids", alias = "asset_ids")]
    pub asset_ids: Vec<U256>,
    
    pub outcomes: Vec<String>,
    
    #[serde(default)]
    pub event_message: Option<EventMessage>,
    
    #[serde_as(as = "DisplayFromStr")]
    pub timestamp: i64,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct MarketResolved {
    
    pub id: String,
    
    #[serde(default)]
    pub question: Option<String>,
    
    pub market: B256,
    
    #[serde(default)]
    pub slug: Option<String>,
    
    #[serde(default)]
    pub description: Option<String>,
    
    #[serde(rename = "assets_ids", alias = "asset_ids")]
    pub asset_ids: Vec<U256>,
    
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub outcomes: Vec<String>,
    
    pub winning_asset_id: U256,
    
    pub winning_outcome: String,
    
    #[serde(default)]
    pub event_message: Option<EventMessage>,
    
    #[serde_as(as = "DisplayFromStr")]
    pub timestamp: i64,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct EventMessage {
    
    pub id: String,
    
    pub ticker: String,
    
    pub slug: String,
    
    pub title: String,
    
    pub description: String,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct MakerOrder {
    
    pub asset_id: U256,
    
    pub matched_amount: Decimal,
    
    pub order_id: String,
    
    pub outcome: String,
    
    pub owner: ApiKey,
    
    pub price: Decimal,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum TradeMessageType {
    #[serde(alias = "trade", alias = "TRADE")]
    Trade,
    #[serde(untagged)]
    Unknown(String),
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum TradeMessageStatus {
    #[serde(alias = "matched", alias = "MATCHED")]
    Matched,
    #[serde(alias = "mined", alias = "MINED")]
    Mined,
    #[serde(alias = "confirmed", alias = "CONFIRMED")]
    Confirmed,
    #[serde(alias = "retrying", alias = "RETRYING")]
    Retrying,
    #[serde(alias = "failed", alias = "FAILED")]
    Failed,
    #[serde(untagged)]
    Unknown(String),
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct TradeMessage {
    
    pub id: String,
    
    pub market: B256,
    
    pub asset_id: U256,
    
    pub side: Side,
    
    pub size: Decimal,
    
    pub price: Decimal,
    
    pub status: TradeMessageStatus,
    
    #[serde(rename = "type", default)]
    pub msg_type: Option<TradeMessageType>,
    
    #[serde(default)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub last_update: Option<i64>,
    
    #[serde(default, alias = "match_time")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub matchtime: Option<i64>,
    
    #[serde(default)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub timestamp: Option<i64>,
    
    #[serde(default)]
    pub outcome: Option<String>,
    
    #[serde(default)]
    pub owner: Option<ApiKey>,
    
    #[serde(default)]
    pub trade_owner: Option<ApiKey>,
    
    #[serde(default)]
    pub taker_order_id: Option<String>,
    
    #[serde(default)]
    pub maker_orders: Vec<MakerOrder>,
    
    #[serde(default)]
    pub fee_rate_bps: Option<Decimal>,
    
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(default)]
    pub transaction_hash: Option<B256>,
    
    #[serde(default)]
    pub trader_side: Option<TraderSide>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum OrderMessageType {
    #[serde(alias = "placement", alias = "PLACEMENT")]
    Placement,
    #[serde(alias = "update", alias = "UPDATE")]
    Update,
    #[serde(alias = "cancellation", alias = "CANCELLATION")]
    Cancellation,
    #[serde(untagged)]
    Unknown(String),
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct OrderMessage {
    
    pub id: String,
    
    pub market: B256,
    
    pub asset_id: U256,
    
    pub side: Side,
    
    pub price: Decimal,
    
    #[serde(rename = "type", default)]
    pub msg_type: Option<OrderMessageType>,
    
    #[serde(default)]
    pub outcome: Option<String>,
    
    #[serde(default)]
    pub owner: Option<ApiKey>,
    
    #[serde(default)]
    pub order_owner: Option<ApiKey>,
    
    #[serde(default)]
    pub original_size: Option<Decimal>,
    
    #[serde(default)]
    pub size_matched: Option<Decimal>,
    
    #[serde(default)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub timestamp: Option<i64>,
    
    #[serde(default)]
    pub associate_trades: Option<Vec<String>>,
    
    #[serde(default)]
    pub status: Option<OrderStatusType>,
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    
    Open,
    
    Matched,
    
    PartiallyFilled,
    
    Cancelled,
    
    Placement,
    
    Update,
    
    Cancellation,
    
    #[serde(untagged)]
    Unknown(String),
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct MidpointUpdate {
    
    pub asset_id: U256,
    
    pub market: B256,
    
    pub midpoint: Decimal,
    
    #[serde_as(as = "DisplayFromStr")]
    pub timestamp: i64,
}

pub fn parse_if_interested(
    bytes: &[u8],
    interest: &MessageInterest,
) -> crate::Result<Vec<WsMessage>> {
    
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|err| crate::error::Error::with_source(Kind::Internal, Box::new(err)))?;

    match &value {
        Value::Object(map) => {
            
            let event_type = map.get("event_type").and_then(Value::as_str);

            match event_type {
                None => Ok(vec![]),
                Some(event_type) if !interest.is_interested_in_event(event_type) => Ok(vec![]),
                Some(_) => {
                    
                    let msg: WsMessage = serde_json::from_value(value)?;
                    Ok(vec![msg])
                }
            }
        }
        Value::Array(arr) => Ok(arr
            .iter()
            .filter_map(|elem| {
                let obj = elem.as_object()?;
                let event_type = obj.get("event_type").and_then(Value::as_str)?;

                if !interest.is_interested_in_event(event_type) {
                    return None;
                }

                serde_json::from_value(elem.clone())
                    .inspect_err(|err| {
                        #[cfg(feature = "tracing")]
                        warn!(
                            event_type = %event_type,
                            error = %err,
                            "Skipping unknown/invalid WS event in batch"
                        );
                    })
                    .ok()
            })
            .collect()),
        _ => Ok(vec![]),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use rust_decimal_macros::dec;

    use super::*;
    use crate::types::b256;

    const TEST_MARKET: B256 =
        b256!("0000000000000000000000000000000000000000000000000000000000000001");

    fn matches_interest(msg: &WsMessage, interest: MessageInterest) -> bool {
        match msg {
            WsMessage::Book(_) => interest.contains(MessageInterest::BOOK),
            WsMessage::PriceChange(_) => interest.contains(MessageInterest::PRICE_CHANGE),
            WsMessage::TickSizeChange(_) => interest.contains(MessageInterest::TICK_SIZE),
            WsMessage::LastTradePrice(_) => interest.contains(MessageInterest::LAST_TRADE_PRICE),
            WsMessage::BestBidAsk(_) => interest.contains(MessageInterest::BEST_BID_ASK),
            WsMessage::NewMarket(_) => interest.contains(MessageInterest::NEW_MARKET),
            WsMessage::MarketResolved(_) => interest.contains(MessageInterest::MARKET_RESOLVED),
            WsMessage::Trade(_) => interest.contains(MessageInterest::TRADE),
            WsMessage::Order(_) => interest.contains(MessageInterest::ORDER),
        }
    }

    #[test]
    fn parse_book_message() {
        let json = r#"{
            "event_type": "book",
            "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
            "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "timestamp": "1234567890",
            "bids": [{"price": "0.5", "size": "100"}],
            "asks": [{"price": "0.51", "size": "50"}]
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::Book(book) => {
                assert_eq!(book.asset_id, U256::from_str("106585164761922456203746651621390029417453862034640469075081961934906147433548").unwrap());
                assert_eq!(book.market, TEST_MARKET);
                assert_eq!(book.bids.len(), 1);
                assert_eq!(book.asks.len(), 1);
            }
            _ => panic!("Expected Book message"),
        }
    }

    #[test]
    fn parse_price_change_message() {
        let json = r#"{
            "event_type": "price_change",
            "market": "0x0000000000000000000000000000000000000000000000000000000000000002",
            "timestamp": "1234567890",
            "price_changes": [{
                "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
                "price": "0.52",
                "size": "10",
                "side": "BUY"
            }]
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::PriceChange(price) => {
                let changes = &price.price_changes[0];

                assert_eq!(changes.asset_id, U256::from_str("106585164761922456203746651621390029417453862034640469075081961934906147433548").unwrap());
                assert_eq!(changes.side, Side::Buy);
                assert_eq!(changes.size.unwrap(), Decimal::TEN);
            }
            _ => panic!("Expected PriceChange message"),
        }
    }

    #[test]
    fn parse_price_change_interest_message() {
        let json = r#"{
            "event_type": "price_change",
            "market": "0x0000000000000000000000000000000000000000000000000000000000000003",
            "timestamp": "1234567890",
            "price_changes": [
                {
                    "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
                    "price": "0.10",
                    "side": "BUY",
                    "hash": "abc",
                    "best_bid": "0.11",
                    "best_ask": "0.12"
                },
                {
                    "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
                    "price": "0.90",
                    "size": "5",
                    "side": "SELL"
                }
            ]
        }"#;

        let msgs = parse_if_interested(json.as_bytes(), &MessageInterest::ALL).unwrap();
        assert_eq!(msgs.len(), 1);

        match &msgs[0] {
            WsMessage::PriceChange(price) => {
                let expected =
                    b256!("0000000000000000000000000000000000000000000000000000000000000003");
                assert_eq!(price.market, expected);

                let changes = &price.price_changes;
                assert_eq!(changes.len(), 2);

                assert_eq!(changes[0].asset_id, U256::from_str("106585164761922456203746651621390029417453862034640469075081961934906147433548").unwrap());
                assert_eq!(changes[0].best_bid, Some(dec!(0.11)));
                assert_eq!(changes[0].price, dec!(0.10));
                assert!(changes[0].size.is_none());

                assert_eq!(changes[1].asset_id, U256::from_str("106585164761922456203746651621390029417453862034640469075081961934906147433548").unwrap());
                assert_eq!(changes[1].best_bid, None);
                assert_eq!(changes[1].size, Some(dec!(5)));
                assert_eq!(changes[1].price, dec!(0.90));
            }
            _ => panic!("Expected first price change"),
        }
    }

    #[test]
    fn parse_batch_messages() {
        let json = r#"[
            {
                "event_type": "book",
                "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
                "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
                "timestamp": "1234567890",
                "bids": [{"price": "0.5", "size": "100"}],
                "asks": []
            },
            {
                "event_type": "price_change",
                "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
                "timestamp": "1234567891",
                "price_changes": [{
                    "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
                    "price": "0.51",
                    "side": "BUY"
                }]
            },
            {
                "event_type": "last_trade_price",
                "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
                "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
                "price": "0.6",
                "timestamp": "1234567892"
            }
        ]"#;

        let msgs = parse_if_interested(json.as_bytes(), &MessageInterest::ALL).unwrap();
        assert_eq!(msgs.len(), 3);

        assert!(
            matches!(&msgs[0], WsMessage::Book(b) if b.asset_id == U256::from_str("106585164761922456203746651621390029417453862034640469075081961934906147433548").unwrap())
        );
        assert!(matches!(&msgs[1], WsMessage::PriceChange(p) if p.market == TEST_MARKET));
        assert!(
            matches!(&msgs[2], WsMessage::LastTradePrice(l) if l.asset_id == U256::from_str("106585164761922456203746651621390029417453862034640469075081961934906147433548").unwrap())
        );
    }

    #[test]
    fn parse_batch_filters_by_interest() {
        let json = r#"[
            {
                "event_type": "book",
                "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
                "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
                "timestamp": "1234567890",
                "bids": [],
                "asks": []
            },
            {
                "event_type": "trade",
                "id": "trade1",
                "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
                "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
                "side": "BUY",
                "size": "10",
                "price": "0.5",
                "status": "MATCHED"
            }
        ]"#;

        let msgs = parse_if_interested(json.as_bytes(), &MessageInterest::BOOK).unwrap();
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], WsMessage::Book(_)));

        let msgs = parse_if_interested(json.as_bytes(), &MessageInterest::TRADE).unwrap();
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], WsMessage::Trade(_)));

        let msgs = parse_if_interested(json.as_bytes(), &MessageInterest::ALL).unwrap();
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn parse_best_bid_ask_message() {
        let json = r#"{
            "event_type": "best_bid_ask",
            "market": "0x0005c0d312de0be897668695bae9f32b624b4a1ae8b140c49f08447fcc74f442",
            "asset_id": "85354956062430465315924116860125388538595433819574542752031640332592237464430",
            "best_bid": "0.73",
            "best_ask": "0.77",
            "spread": "0.04",
            "timestamp": "1766789469958"
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::BestBidAsk(bba) => {
                assert_eq!(bba.best_bid, dec!(0.73));
                assert_eq!(bba.best_ask, dec!(0.77));
                assert_eq!(bba.spread, dec!(0.04));
            }
            _ => panic!("Expected BestBidAsk message"),
        }
    }

    #[test]
    fn parse_new_market_message() {
        let json = r#"{
            "id": "1031769",
            "question": "Will NVIDIA (NVDA) close above $240 end of January?",
            "market": "0x311d0c4b6671ab54af4970c06fcf58662516f5168997bdda209ec3db5aa6b0c1",
            "slug": "nvda-above-240-on-january-30-2026",
            "description": "This market will resolve to Yes or No.",
            "assets_ids": [
                "76043073756653678226373981964075571318267289248134717369284518995922789326425",
                "31690934263385727664202099278545688007799199447969475608906331829650099442770"
            ],
            "outcomes": ["Yes", "No"],
            "event_message": {
                "id": "125819",
                "ticker": "nvda-above-in-january-2026",
                "slug": "nvda-above-in-january-2026",
                "title": "Will NVIDIA (NVDA) close above ___ end of January?",
                "description": "Market description"
            },
            "timestamp": "1766790415550",
            "event_type": "new_market"
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::NewMarket(nm) => {
                assert_eq!(nm.id, "1031769");
                assert_eq!(
                    nm.question,
                    "Will NVIDIA (NVDA) close above $240 end of January?"
                );
                assert_eq!(nm.asset_ids.len(), 2);
                assert_eq!(nm.outcomes, vec!["Yes", "No"]);
                assert!(nm.event_message.is_some());
                let event = nm.event_message.unwrap();
                assert_eq!(event.id, "125819");
                assert_eq!(event.ticker, "nvda-above-in-january-2026");
            }
            _ => panic!("Expected NewMarket message"),
        }
    }

    #[test]
    fn parse_market_resolved_message() {
        let json = r#"{
            "id": "1031769",
            "question": "Will NVIDIA (NVDA) close above $240 end of January?",
            "market": "0x311d0c4b6671ab54af4970c06fcf58662516f5168997bdda209ec3db5aa6b0c1",
            "slug": "nvda-above-240-on-january-30-2026",
            "description": "This market will resolve to Yes or No.",
            "assets_ids": [
                "76043073756653678226373981964075571318267289248134717369284518995922789326425",
                "31690934263385727664202099278545688007799199447969475608906331829650099442770"
            ],
            "outcomes": ["Yes", "No"],
            "winning_asset_id": "76043073756653678226373981964075571318267289248134717369284518995922789326425",
            "winning_outcome": "Yes",
            "event_message": {
                "id": "125819",
                "ticker": "nvda-above-in-january-2026",
                "slug": "nvda-above-in-january-2026",
                "title": "Will NVIDIA (NVDA) close above ___ end of January?",
                "description": "Market description"
            },
            "timestamp": "1766790415550",
            "event_type": "market_resolved"
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::MarketResolved(mr) => {
                assert_eq!(mr.id, "1031769");
                assert_eq!(mr.winning_outcome, "Yes");
                assert_eq!(
                    mr.winning_asset_id,
                    U256::from_str("76043073756653678226373981964075571318267289248134717369284518995922789326425").unwrap()
                );
                assert_eq!(mr.asset_ids.len(), 2);
            }
            _ => panic!("Expected MarketResolved message"),
        }
    }

    #[test]
    fn parse_last_trade_price_with_new_fields() {
        let json = r#"{
            "asset_id": "114122071509644379678018727908709560226618148003371446110114509806601493071694",
            "event_type": "last_trade_price",
            "fee_rate_bps": "0",
            "market": "0x6a67b9d828d53862160e470329ffea5246f338ecfffdf2cab45211ec578b0347",
            "price": "0.456",
            "side": "BUY",
            "size": "219.217767",
            "timestamp": "1750428146322"
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::LastTradePrice(ltp) => {
                assert_eq!(ltp.price, dec!(0.456));
                assert_eq!(ltp.size, Some(dec!(219.217767)));
                assert_eq!(ltp.fee_rate_bps, Some(Decimal::ZERO));
                assert_eq!(ltp.side, Some(Side::Buy));
            }
            _ => panic!("Expected LastTradePrice message"),
        }
    }

    #[test]
    fn parse_custom_feature_messages_filter_by_interest() {
        let json = r#"[
            {
                "event_type": "best_bid_ask",
                "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
                "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
                "best_bid": "0.5",
                "best_ask": "0.6",
                "spread": "0.1",
                "timestamp": "1234567890"
            },
            {
                "event_type": "book",
                "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
                "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
                "timestamp": "1234567890",
                "bids": [],
                "asks": []
            }
        ]"#;

        let msgs = parse_if_interested(json.as_bytes(), &MessageInterest::BEST_BID_ASK).unwrap();
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], WsMessage::BestBidAsk(_)));

        let msgs = parse_if_interested(json.as_bytes(), &MessageInterest::BOOK).unwrap();
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], WsMessage::Book(_)));

        let msgs = parse_if_interested(json.as_bytes(), &MessageInterest::MARKET).unwrap();
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn parse_new_market_without_event_message() {
        let json = r#"{
            "id": "1031769",
            "question": "Will NVIDIA (NVDA) close above $240 end of January?",
            "market": "0x311d0c4b6671ab54af4970c06fcf58662516f5168997bdda209ec3db5aa6b0c1",
            "slug": "nvda-above-240-on-january-30-2026",
            "description": "This market will resolve to Yes or No.",
            "assets_ids": ["106585164761922456203746651621390029417453862034640469075081961934906147433548", "106585164761922456203746651621390029417453862034640469075081961934906147433548"],
            "outcomes": ["Yes", "No"],
            "timestamp": "1766790415550",
            "event_type": "new_market"
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::NewMarket(nm) => {
                assert_eq!(nm.id, "1031769");
                assert!(nm.event_message.is_none());
            }
            _ => panic!("Expected NewMarket message"),
        }
    }

    #[test]
    fn parse_market_resolved_without_event_message() {
        let json = r#"{
            "id": "1031769",
            "question": "Will NVIDIA (NVDA) close above $240 end of January?",
            "market": "0x311d0c4b6671ab54af4970c06fcf58662516f5168997bdda209ec3db5aa6b0c1",
            "slug": "nvda-above-240-on-january-30-2026",
            "description": "This market will resolve to Yes or No.",
            "assets_ids": ["106585164761922456203746651621390029417453862034640469075081961934906147433548", "106585164761922456203746651621390029417453862034640469075081961934906147433548"],
            "outcomes": ["Yes", "No"],
            "winning_asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
            "winning_outcome": "Yes",
            "timestamp": "1766790415550",
            "event_type": "market_resolved"
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::MarketResolved(mr) => {
                assert_eq!(mr.id, "1031769");
                assert!(mr.event_message.is_none());
                assert_eq!(mr.winning_outcome, "Yes");
            }
            _ => panic!("Expected MarketResolved message"),
        }
    }

    #[test]
    fn parse_last_trade_price_without_optional_fields() {
        let json = r#"{
            "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
            "event_type": "last_trade_price",
            "market": "0x0000000000000000000000000000000000000000000000000000000000000123",
            "price": "0.5",
            "timestamp": "1750428146322"
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::LastTradePrice(ltp) => {
                assert_eq!(ltp.price, dec!(0.5));
                assert!(ltp.size.is_none());
                assert!(ltp.fee_rate_bps.is_none());
                assert!(ltp.side.is_none());
            }
            _ => panic!("Expected LastTradePrice message"),
        }
    }

    #[test]
    fn matches_interest_custom_feature_messages() {
        let bba = WsMessage::BestBidAsk(BestBidAsk {
            market: TEST_MARKET,
            asset_id: U256::from_str(
                "106585164761922456203746651621390029417453862034640469075081961934906147433548",
            )
            .unwrap(),
            best_bid: dec!(0.5),
            best_ask: dec!(0.6),
            spread: dec!(0.1),
            timestamp: 0,
        });
        assert!(matches_interest(&bba, MessageInterest::BEST_BID_ASK));
        assert!(!matches_interest(&bba, MessageInterest::BOOK));
        assert!(matches_interest(&bba, MessageInterest::MARKET));

        let nm = WsMessage::NewMarket(NewMarket {
            id: "1".to_owned(),
            question: "q".to_owned(),
            market: TEST_MARKET,
            slug: "s".to_owned(),
            description: "d".to_owned(),
            asset_ids: vec![],
            outcomes: vec![],
            event_message: None,
            timestamp: 0,
        });
        assert!(matches_interest(&nm, MessageInterest::NEW_MARKET));
        assert!(matches_interest(&nm, MessageInterest::MARKET));

        let mr = WsMessage::MarketResolved(MarketResolved {
            id: "1".to_owned(),
            question: Some("q".to_owned()),
            market: TEST_MARKET,
            slug: Some("s".to_owned()),
            description: Some("d".to_owned()),
            asset_ids: vec![],
            outcomes: vec![],
            winning_asset_id: U256::from_str(
                "106585164761922456203746651621390029417453862034640469075081961934906147433548",
            )
            .unwrap(),
            winning_outcome: "Yes".to_owned(),
            event_message: None,
            timestamp: 0,
        });
        assert!(matches_interest(&mr, MessageInterest::MARKET_RESOLVED));
        assert!(matches_interest(&mr, MessageInterest::MARKET));
    }

    #[test]
    fn parse_if_interested_returns_empty_for_missing_event_type() {
        
        let json = r#"{"some_field": "value"}"#;
        let msgs = parse_if_interested(json.as_bytes(), &MessageInterest::ALL).unwrap();
        assert!(msgs.is_empty());
    }

    #[test]
    fn parse_if_interested_returns_empty_for_primitive_json() {
        
        let msgs = parse_if_interested(b"null", &MessageInterest::ALL).unwrap();
        assert!(msgs.is_empty());

        let msgs = parse_if_interested(b"42", &MessageInterest::ALL).unwrap();
        assert!(msgs.is_empty());

        let msgs = parse_if_interested(b"\"string\"", &MessageInterest::ALL).unwrap();
        assert!(msgs.is_empty());

        let msgs = parse_if_interested(b"true", &MessageInterest::ALL).unwrap();
        assert!(msgs.is_empty());
    }

    #[test]
    fn parse_batch_with_unknown_event_type() {
        let json = r#"[
            {
                "event_type": "book",
                "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
                "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
                "timestamp": "1234567890",
                "bids": [{"price": "0.5", "size": "100"}],
                "asks": []
            },
            {
                "event_type": "SOME_NEW_EVENT",
                "unknown_field": "arbitrary data",
                "another_field": 123
            }
        ]"#;

        let msgs = parse_if_interested(json.as_bytes(), &MessageInterest::ALL).unwrap();
        
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], WsMessage::Book(_)));
    }

    #[test]
    fn parse_trade_message_with_unknown_type() {
        let json = r#"{
            "event_type": "trade",
            "id": "trade123",
            "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
            "side": "BUY",
            "size": "10",
            "price": "0.5",
            "status": "MATCHED",
            "type": "NEW_TYPE"
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::Trade(trade) => {
                assert_eq!(trade.id, "trade123");
                assert_eq!(
                    trade.msg_type,
                    Some(TradeMessageType::Unknown("NEW_TYPE".to_owned()))
                );
            }
            _ => panic!("Expected Trade message"),
        }
    }

    #[test]
    fn parse_trade_message_with_retrying_status() {
        let json = r#"{
            "event_type": "trade",
            "id": "trade123",
            "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
            "side": "BUY",
            "size": "10",
            "price": "0.5",
            "status": "RETRYING",
            "type": "TRADE"
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::Trade(trade) => {
                assert_eq!(trade.id, "trade123");
                assert_eq!(trade.status, TradeMessageStatus::Retrying);
            }
            _ => panic!("Expected Trade message"),
        }
    }

    #[test]
    fn parse_trade_message_with_failed_status() {
        let json = r#"{
            "event_type": "trade",
            "id": "trade123",
            "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
            "side": "BUY",
            "size": "10",
            "price": "0.5",
            "status": "FAILED",
            "type": "TRADE"
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::Trade(trade) => {
                assert_eq!(trade.id, "trade123");
                assert_eq!(trade.status, TradeMessageStatus::Failed);
            }
            _ => panic!("Expected Trade message"),
        }
    }

    #[test]
    fn parse_new_market_with_asset_ids_alias() {
        let json = r#"{
            "id": "test123",
            "question": "Test question?",
            "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "slug": "test-slug",
            "description": "Test description",
            "asset_ids": [
                "106585164761922456203746651621390029417453862034640469075081961934906147433548"
            ],
            "outcomes": ["Yes", "No"],
            "timestamp": "1234567890",
            "event_type": "new_market"
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::NewMarket(nm) => {
                assert_eq!(nm.id, "test123");
                assert_eq!(nm.asset_ids.len(), 1);
                assert_eq!(
                    nm.asset_ids[0],
                    U256::from_str("106585164761922456203746651621390029417453862034640469075081961934906147433548").unwrap()
                );
            }
            _ => panic!("Expected NewMarket message"),
        }
    }

    #[test]
    fn parse_market_resolved_with_asset_ids_alias() {
        let json = r#"{
            "id": "test123",
            "question": "Test question?",
            "market": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "slug": "test-slug",
            "description": "Test description",
            "asset_ids": [
                "106585164761922456203746651621390029417453862034640469075081961934906147433548"
            ],
            "outcomes": ["Yes", "No"],
            "winning_asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
            "winning_outcome": "Yes",
            "timestamp": "1234567890",
            "event_type": "market_resolved"
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsMessage::MarketResolved(mr) => {
                assert_eq!(mr.id, "test123");
                assert_eq!(mr.asset_ids.len(), 1);
            }
            _ => panic!("Expected MarketResolved message"),
        }
    }
}
