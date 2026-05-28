use serde::Serialize;
use serde_with::{DisplayFromStr, serde_as};
use strum_macros::Display;

use crate::types::{B256, U256};
use crate::ws::WithCredentials;

#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Operation {
    Subscribe,
    Unsubscribe,
}

#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Channel {
    User,
    Market,
}

#[non_exhaustive]
#[serde_as]
#[derive(Clone, Debug, Serialize)]
pub struct SubscriptionRequest {
    
    pub r#type: Channel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<Operation>,
    
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub markets: Vec<B256>,
    
    #[serde(rename = "assets_ids")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub asset_ids: Vec<U256>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_dump: Option<bool>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_feature_enabled: Option<bool>,
}

impl WithCredentials for SubscriptionRequest {}

impl SubscriptionRequest {
    
    #[must_use]
    pub fn market(asset_ids: Vec<U256>) -> Self {
        Self {
            r#type: Channel::Market,
            operation: Some(Operation::Subscribe),
            markets: vec![],
            asset_ids,
            initial_dump: Some(true),
            custom_feature_enabled: None,
        }
    }

    #[must_use]
    pub fn market_unsubscribe(asset_ids: Vec<U256>) -> Self {
        Self {
            r#type: Channel::Market,
            operation: Some(Operation::Unsubscribe),
            markets: vec![],
            asset_ids,
            initial_dump: None,
            custom_feature_enabled: None,
        }
    }

    #[must_use]
    pub fn user(markets: Vec<B256>) -> Self {
        Self {
            r#type: Channel::User,
            operation: Some(Operation::Subscribe),
            markets,
            asset_ids: vec![],
            initial_dump: Some(true),
            custom_feature_enabled: None,
        }
    }

    #[must_use]
    pub fn user_unsubscribe(markets: Vec<B256>) -> Self {
        Self {
            r#type: Channel::User,
            operation: Some(Operation::Unsubscribe),
            markets,
            asset_ids: vec![],
            initial_dump: None,
            custom_feature_enabled: None,
        }
    }

    #[must_use]
    pub fn with_custom_features(mut self, enabled: bool) -> Self {
        self.custom_feature_enabled = Some(enabled);
        self
    }
}
