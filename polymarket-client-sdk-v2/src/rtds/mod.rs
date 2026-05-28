#![expect(
    clippy::module_name_repetitions,
    reason = "Re-exported names intentionally match their modules for API clarity"
)]

pub mod client;
pub mod error;
pub mod subscription;
pub mod types;

pub use client::Client;
pub use error::RtdsError;
pub use subscription::SubscriptionInfo;
pub use types::request::{Subscription, SubscriptionAction, SubscriptionRequest};
pub use types::response::{
    ChainlinkPrice, Comment, CommentProfile, CommentType, CryptoPrice, RtdsMessage,
};
