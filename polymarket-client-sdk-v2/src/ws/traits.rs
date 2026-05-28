
use secrecy::ExposeSecret as _;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::auth::Credentials;

pub trait MessageParser<M: DeserializeOwned>: Send + Sync + 'static {
    /// Parse incoming bytes into messages.
    ///
    /// May return empty vec if messages are filtered out based on interest or other criteria.
    /// Handles both single objects and arrays of messages.
    fn parse(&self, bytes: &[u8]) -> crate::Result<Vec<M>>;
}

pub trait WithCredentials: Serialize + Sized {
    fn as_authenticated(&self, credentials: &Credentials) -> Result<String, serde_json::Error> {
        let mut payload_json = serde_json::to_value(self)?;
        let auth = json!({
            "apiKey": credentials.key.to_string(),
            "secret": credentials.secret.expose_secret(),
            "passphrase": credentials.passphrase.expose_secret(),
        });

        if let Value::Object(ref mut obj) = payload_json {
            obj.insert("auth".to_owned(), auth);
        }

        serde_json::to_string(&payload_json)
    }
}
