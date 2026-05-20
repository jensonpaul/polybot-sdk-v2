#![expect(
    clippy::module_name_repetitions,
    reason = "Error types include the module name to indicate their scope"
)]

use std::error::Error as StdError;
use std::fmt;

#[non_exhaustive]
#[derive(Debug)]
pub enum RtdsError {
    
    Connection(tokio_tungstenite::tungstenite::Error),
    
    MessageParse(serde_json::Error),
    
    SubscriptionFailed(String),
    
    AuthenticationFailed,
    
    ConnectionClosed,
    
    Timeout,
    
    InvalidMessage(String),
}

impl fmt::Display for RtdsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection(err) => write!(f, "RTDS WebSocket connection error: {err}"),
            Self::MessageParse(err) => write!(f, "Failed to parse RTDS message: {err}"),
            Self::SubscriptionFailed(reason) => write!(f, "RTDS subscription failed: {reason}"),
            Self::AuthenticationFailed => write!(f, "RTDS WebSocket authentication failed"),
            Self::ConnectionClosed => write!(f, "RTDS WebSocket connection closed"),
            Self::Timeout => write!(f, "RTDS WebSocket operation timed out"),
            Self::InvalidMessage(msg) => write!(f, "Invalid RTDS message: {msg}"),
        }
    }
}

impl StdError for RtdsError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Connection(err) => Some(err),
            Self::MessageParse(err) => Some(err),
            _ => None,
        }
    }
}

impl From<RtdsError> for crate::error::Error {
    fn from(err: RtdsError) -> Self {
        crate::error::Error::with_source(crate::error::Kind::WebSocket, err)
    }
}
