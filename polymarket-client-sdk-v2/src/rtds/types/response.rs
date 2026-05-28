use bon::Builder;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::{Address, Decimal};

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Builder)]
pub struct RtdsMessage {
    
    pub topic: String,
    
    #[serde(rename = "type")]
    pub msg_type: String,
    
    pub timestamp: i64,
    
    pub payload: Value,
}

impl RtdsMessage {
    
    #[must_use]
    pub fn as_crypto_price(&self) -> Option<CryptoPrice> {
        if self.topic == "crypto_prices" {
            serde_json::from_value(self.payload.clone()).ok()
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_chainlink_price(&self) -> Option<ChainlinkPrice> {
        if self.topic == "crypto_prices_chainlink" {
            serde_json::from_value(self.payload.clone()).ok()
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_comment(&self) -> Option<Comment> {
        if self.topic == "comments" {
            serde_json::from_value(self.payload.clone()).ok()
        } else {
            None
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Serialize, Builder)]
pub struct CryptoPrice {
    
    pub symbol: String,
    
    pub timestamp: i64,
    
    pub value: Decimal,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Serialize, Builder)]
pub struct ChainlinkPrice {
    
    pub symbol: String,
    
    pub timestamp: i64,
    
    pub value: Decimal,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Serialize, Builder)]
pub struct Comment {
    
    pub id: String,
    
    pub body: String,
    
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    
    #[serde(rename = "parentCommentID", default)]
    pub parent_comment_id: Option<String>,
    
    #[serde(rename = "parentEntityID")]
    pub parent_entity_id: i64,
    
    #[serde(rename = "parentEntityType")]
    pub parent_entity_type: String,
    
    pub profile: CommentProfile,
    
    #[serde(rename = "reactionCount", default)]
    pub reaction_count: i64,
    
    #[serde(rename = "replyAddress", default)]
    pub reply_address: Option<Address>,
    
    #[serde(rename = "reportCount", default)]
    pub report_count: i64,
    
    #[serde(rename = "userAddress")]
    pub user_address: Address,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Serialize, Builder)]
pub struct CommentProfile {
    
    #[serde(rename = "baseAddress")]
    pub base_address: Address,
    
    #[serde(rename = "displayUsernamePublic", default)]
    pub display_username_public: bool,
    
    pub name: String,
    
    #[serde(rename = "proxyWallet", default)]
    pub proxy_wallet: Option<Address>,
    
    #[serde(default)]
    pub pseudonym: Option<String>,
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentType {
    
    CommentCreated,
    
    CommentRemoved,
    
    ReactionCreated,
    
    ReactionRemoved,
    
    #[serde(untagged)]
    Unknown(String),
}

pub fn parse_messages(bytes: &[u8]) -> crate::Result<Vec<RtdsMessage>> {
    
    let trimmed = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .map_or(&[][..], |start| &bytes[start..]);

    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    if trimmed.first() == Some(&b'[') {
        Ok(serde_json::from_slice(trimmed)?)
    } else {
        let msg: RtdsMessage = serde_json::from_slice(trimmed)?;
        Ok(vec![msg])
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn parse_crypto_price_message() {
        let json = r#"{
            "topic": "crypto_prices",
            "type": "update",
            "timestamp": 1753314064237,
            "payload": {
                "symbol": "solusdt",
                "timestamp": 1753314064213,
                "value": 189.55
            }
        }"#;

        let msgs = parse_messages(json.as_bytes()).unwrap();
        assert_eq!(msgs.len(), 1);

        let msg = &msgs[0];
        assert_eq!(msg.topic, "crypto_prices");
        assert_eq!(msg.msg_type, "update");

        let price = msg.as_crypto_price().unwrap();
        assert_eq!(price.symbol, "solusdt");
        assert_eq!(price.value, dec!(189.55));
    }

    #[test]
    fn parse_chainlink_price_message() {
        let json = r#"{
            "topic": "crypto_prices_chainlink",
            "type": "update",
            "timestamp": 1753314064237,
            "payload": {
                "symbol": "eth/usd",
                "timestamp": 1753314064213,
                "value": 3456.78
            }
        }"#;

        let msgs = parse_messages(json.as_bytes()).unwrap();
        assert_eq!(msgs.len(), 1);

        let msg = &msgs[0];
        assert_eq!(msg.topic, "crypto_prices_chainlink");

        let price = msg.as_chainlink_price().unwrap();
        assert_eq!(price.symbol, "eth/usd");
        assert_eq!(price.value, dec!(3456.78));
    }

    #[test]
    fn parse_comment_message() {
        let json = r#"{
            "topic": "comments",
            "type": "comment_created",
            "timestamp": 1753454975808,
            "payload": {
                "body": "Test comment",
                "createdAt": "2025-07-25T14:49:35.801298Z",
                "id": "1763355",
                "parentCommentID": "1763325",
                "parentEntityID": 18396,
                "parentEntityType": "Event",
                "profile": {
                    "baseAddress": "0xce533188d53a16ed580fd5121dedf166d3482677",
                    "displayUsernamePublic": true,
                    "name": "salted.caramel",
                    "proxyWallet": "0x4ca749dcfa93c87e5ee23e2d21ff4422c7a4c1ee",
                    "pseudonym": "Adored-Disparity"
                },
                "reactionCount": 0,
                "replyAddress": "0x0bda5d16f76cd1d3485bcc7a44bc6fa7db004cdd",
                "reportCount": 0,
                "userAddress": "0xce533188d53a16ed580fd5121dedf166d3482677"
            }
        }"#;

        let msgs = parse_messages(json.as_bytes()).unwrap();
        assert_eq!(msgs.len(), 1);

        let msg = &msgs[0];
        assert_eq!(msg.topic, "comments");
        assert_eq!(msg.msg_type, "comment_created");

        let comment = msg.as_comment().unwrap();
        assert_eq!(comment.id, "1763355");
        assert_eq!(comment.body, "Test comment");
        assert_eq!(comment.profile.name, "salted.caramel");
    }

    #[test]
    fn parse_message_array() {
        let json = r#"[{
            "topic": "crypto_prices",
            "type": "update",
            "timestamp": 1753314064237,
            "payload": {
                "symbol": "btcusdt",
                "timestamp": 1753314064213,
                "value": 67234.50
            }
        }]"#;

        let msgs = parse_messages(json.as_bytes()).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].topic, "crypto_prices");
    }

    #[test]
    fn parse_empty_input() {
        let msgs = parse_messages(b"").unwrap();
        assert!(msgs.is_empty());
    }

    #[test]
    fn parse_whitespace_only_input() {
        let msgs = parse_messages(b"   \n\t  ").unwrap();
        assert!(msgs.is_empty());
    }
}
