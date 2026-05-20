
use serde::{Deserialize, Serialize};

pub mod request;
pub mod response;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, strum_macros::Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[non_exhaustive]
pub enum RelatedTagsStatus {
    Active,
    Closed,
    All,
    
    #[serde(untagged)]
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, strum_macros::Display)]
#[non_exhaustive]
pub enum ParentEntityType {
    Event,
    Series,
    #[serde(rename = "market")]
    #[strum(serialize = "market")]
    Market,
    
    #[serde(untagged)]
    Unknown(String),
}
