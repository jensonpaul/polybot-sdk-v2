use bon::Builder;
use secrecy::ExposeSecret as _;
use serde::Serialize;
use serde_json::Value;

use super::response::CommentType;
use crate::auth::Credentials;

#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Builder)]
pub struct SubscriptionRequest {
    
    pub action: SubscriptionAction,
    
    pub subscriptions: Vec<Subscription>,
}

impl SubscriptionRequest {
    
    #[must_use]
    pub fn subscribe(subscriptions: Vec<Subscription>) -> Self {
        Self {
            action: SubscriptionAction::Subscribe,
            subscriptions,
        }
    }

    #[must_use]
    pub fn unsubscribe(subscriptions: Vec<Subscription>) -> Self {
        Self {
            action: SubscriptionAction::Unsubscribe,
            subscriptions,
        }
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionAction {
    
    Subscribe,
    
    Unsubscribe,
}

#[non_exhaustive]
#[derive(Clone, Debug, Builder)]
pub struct Subscription {
    
    pub topic: String,
    
    pub msg_type: String,
    
    pub filters: Option<String>,
    
    pub clob_auth: Option<Credentials>,
}

impl Subscription {
    
    #[must_use]
    pub fn crypto_prices(symbols: Option<Vec<String>>) -> Self {
        
        let filters =
            symbols.map(|s| serde_json::to_string(&s).unwrap_or_else(|_| "[]".to_owned()));
        Self {
            topic: "crypto_prices".to_owned(),
            msg_type: "update".to_owned(),
            filters,
            clob_auth: None,
        }
    }

    #[must_use]
    pub fn chainlink_prices(symbol: Option<String>) -> Self {
        let filters = symbol.map(|s| format!(r#"{{"symbol":"{s}"}}"#));
        Self {
            topic: "crypto_prices_chainlink".to_owned(),
            msg_type: "*".to_owned(),
            filters,
            clob_auth: None,
        }
    }

    #[must_use]
    pub fn comments(msg_type: Option<CommentType>) -> Self {
        let type_str = msg_type.map_or("*".to_owned(), |t| {
            serde_json::to_string(&t)
                .ok()
                .and_then(|s| s.trim_matches('"').to_owned().into())
                .unwrap_or_else(|| "*".to_owned())
        });
        Self {
            topic: "comments".to_owned(),
            msg_type: type_str,
            filters: None,
            clob_auth: None,
        }
    }

    #[must_use]
    pub fn with_clob_auth(mut self, credentials: Credentials) -> Self {
        self.clob_auth = Some(credentials);
        self
    }

    #[must_use]
    pub fn with_filters(mut self, filters: String) -> Self {
        self.filters = Some(filters);
        self
    }
}

impl Serialize for Subscription {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap as _;

        let mut map = serializer.serialize_map(None)?;

        map.serialize_entry("topic", &self.topic)?;
        map.serialize_entry("type", &self.msg_type)?;

        if let Some(filters) = &self.filters {
            
            if self.topic == "crypto_prices_chainlink" {
                
                map.serialize_entry("filters", filters)?;
            } else if let Ok(json_value) = serde_json::from_str::<Value>(filters) {
                
                map.serialize_entry("filters", &json_value)?;
            } else {
                
                map.serialize_entry("filters", filters)?;
            }
        }

        if let Some(creds) = &self.clob_auth {
            let auth = serde_json::json!({
                "key": creds.key.to_string(),
                "secret": creds.secret.expose_secret(),
                "passphrase": creds.passphrase.expose_secret(),
            });
            map.serialize_entry("clob_auth", &auth)?;
        }

        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_subscription_request() {
        let sub =
            Subscription::crypto_prices(Some(vec!["btcusdt".to_owned(), "ethusdt".to_owned()]));
        let request = SubscriptionRequest::subscribe(vec![sub]);

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"action\":\"subscribe\""));
        assert!(json.contains("\"topic\":\"crypto_prices\""));
        
        assert!(json.contains("\"filters\":[\"btcusdt\",\"ethusdt\"]"));
    }

    #[test]
    fn serialize_chainlink_subscription() {
        let sub = Subscription::chainlink_prices(Some("eth/usd".to_owned()));
        let request = SubscriptionRequest::subscribe(vec![sub]);

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"topic\":\"crypto_prices_chainlink\""));
        assert!(json.contains("\"type\":\"*\""));
        
        assert!(
            json.contains(r#""filters":"{\"symbol\":\"eth/usd\"}""#),
            "Chainlink filters should be serialized as escaped JSON string, got: {json}"
        );
    }

    #[test]
    fn serialize_comments_subscription() {
        let sub = Subscription::comments(Some(CommentType::CommentCreated));
        let request = SubscriptionRequest::subscribe(vec![sub]);

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"topic\":\"comments\""));
        assert!(json.contains("\"type\":\"comment_created\""));
    }

    #[test]
    fn serialize_chainlink_without_filters() {
        
        let sub = Subscription::chainlink_prices(None);
        let request = SubscriptionRequest::subscribe(vec![sub]);

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"topic\":\"crypto_prices_chainlink\""));
        assert!(!json.contains("\"filters\""));
    }

    #[test]
    fn serialize_crypto_prices_without_filters() {
        
        let sub = Subscription::crypto_prices(None);
        let request = SubscriptionRequest::subscribe(vec![sub]);

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"topic\":\"crypto_prices\""));
        assert!(!json.contains("\"filters\""));
    }

    #[test]
    fn serialize_mixed_subscriptions() {
        
        let chainlink = Subscription::chainlink_prices(Some("btc/usd".to_owned()));
        let binance =
            Subscription::crypto_prices(Some(vec!["btcusdt".to_owned(), "ethusdt".to_owned()]));
        let request = SubscriptionRequest::subscribe(vec![chainlink, binance]);

        let json = serde_json::to_string(&request).unwrap();

        assert!(
            json.contains(r#""filters":"{\"symbol\":\"btc/usd\"}""#),
            "Chainlink filters should be escaped string, got: {json}"
        );
        // Binance should have raw JSON array filters
        assert!(
            json.contains("\"filters\":[\"btcusdt\",\"ethusdt\"]"),
            "Binance filters should be raw JSON array, got: {json}"
        );
    }

    #[test]
    fn serialize_unsubscribe_request() {
        let sub = Subscription::crypto_prices(Some(vec!["btcusdt".to_owned()]));
        let request = SubscriptionRequest::unsubscribe(vec![sub]);

        let json = serde_json::to_string(&request).unwrap();
        assert!(
            json.contains("\"action\":\"unsubscribe\""),
            "Action should be 'unsubscribe', got: {json}"
        );
        assert!(json.contains("\"topic\":\"crypto_prices\""));
        assert!(json.contains("\"type\":\"update\""));
    }

    #[test]
    fn serialize_unsubscribe_without_filters() {
        let sub = Subscription::crypto_prices(None);
        let request = SubscriptionRequest::unsubscribe(vec![sub]);

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"action\":\"unsubscribe\""));
        assert!(json.contains("\"topic\":\"crypto_prices\""));
        assert!(
            !json.contains("\"filters\""),
            "Should have no filters field"
        );
    }

    #[test]
    fn serialize_unsubscribe_chainlink() {
        let sub = Subscription::chainlink_prices(Some("btc/usd".to_owned()));
        let request = SubscriptionRequest::unsubscribe(vec![sub]);

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"action\":\"unsubscribe\""));
        assert!(json.contains("\"topic\":\"crypto_prices_chainlink\""));
        assert!(json.contains("\"type\":\"*\""));
    }

    #[test]
    fn serialize_unsubscribe_comments() {
        let sub = Subscription::comments(Some(CommentType::CommentCreated));
        let request = SubscriptionRequest::unsubscribe(vec![sub]);

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"action\":\"unsubscribe\""));
        assert!(json.contains("\"topic\":\"comments\""));
        assert!(json.contains("\"type\":\"comment_created\""));
    }

    #[test]
    fn serialize_unsubscribe_multiple_topics() {
        let crypto = Subscription::crypto_prices(None);
        let chainlink = Subscription::chainlink_prices(None);
        let comments = Subscription::comments(None);
        let request = SubscriptionRequest::unsubscribe(vec![crypto, chainlink, comments]);

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"action\":\"unsubscribe\""));
        assert!(json.contains("\"topic\":\"crypto_prices\""));
        assert!(json.contains("\"topic\":\"crypto_prices_chainlink\""));
        assert!(json.contains("\"topic\":\"comments\""));
    }
}
