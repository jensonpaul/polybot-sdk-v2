
pub use alloy::signers::Signer;

pub use alloy::signers::local::LocalSigner;
use async_trait::async_trait;
#[cfg(any(feature = "clob", test))]
use base64::Engine as _;
#[cfg(any(feature = "clob", test))]
use base64::engine::general_purpose::URL_SAFE;
#[cfg(any(feature = "clob", test))]
use hmac::{Hmac, Mac as _};
#[cfg(any(feature = "clob", test))]
use reqwest::Body;
use reqwest::Request;
use reqwest::header::HeaderMap;

pub use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
#[cfg(any(feature = "clob", test))]
use sha2::Sha256;

pub use uuid::Uuid;

use crate::{Result, Timestamp};

pub type ApiKey = Uuid;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Credentials {
    #[serde(alias = "apiKey")]
    pub(crate) key: ApiKey,
    pub(crate) secret: SecretString,
    pub(crate) passphrase: SecretString,
}

impl Credentials {
    #[must_use]
    pub fn new(key: Uuid, secret: String, passphrase: String) -> Self {
        Self {
            key,
            secret: SecretString::from(secret),
            passphrase: SecretString::from(passphrase),
        }
    }

    #[must_use]
    pub fn key(&self) -> ApiKey {
        self.key
    }

    #[must_use]
    pub fn secret(&self) -> &SecretString {
        &self.secret
    }

    #[must_use]
    pub fn passphrase(&self) -> &SecretString {
        &self.passphrase
    }
}

pub mod state {
    use crate::auth::{Credentials, Kind};
    use crate::types::Address;

    #[non_exhaustive]
    #[derive(Clone, Debug)]
    pub struct Unauthenticated;

    #[non_exhaustive]
    #[derive(Clone, Debug)]
    #[cfg_attr(
        not(feature = "clob"),
        expect(dead_code, reason = "Fields used by clob module when feature enabled")
    )]
    pub struct Authenticated<K: Kind> {
        
        pub(crate) address: Address,
        
        pub(crate) credentials: Credentials,
        
        pub(crate) kind: K,
    }

    pub trait State: sealed::Sealed {}

    impl State for Unauthenticated {}
    impl sealed::Sealed for Unauthenticated {}

    impl<K: Kind> State for Authenticated<K> {}
    impl<K: Kind> sealed::Sealed for Authenticated<K> {}

    mod sealed {
        pub trait Sealed {}
    }
}

#[async_trait]
pub trait Kind: sealed::Sealed + Clone + Send + Sync + 'static {
    async fn extra_headers(&self, request: &Request, timestamp: Timestamp) -> Result<HeaderMap>;
}

/// Non-special, generic authentication. Sometimes referred to as L2 authentication.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Normal;

#[async_trait]
impl Kind for Normal {
    async fn extra_headers(&self, _request: &Request, _timestamp: Timestamp) -> Result<HeaderMap> {
        Ok(HeaderMap::new())
    }
}

impl sealed::Sealed for Normal {}

mod sealed {
    pub trait Sealed {}
}

#[cfg(feature = "clob")]
pub(crate) mod l1 {
    use std::borrow::Cow;

    use alloy::core::sol;
    use alloy::dyn_abi::Eip712Domain;
    use alloy::hex::ToHexExt as _;
    use alloy::primitives::{ChainId, U256};
    use alloy::signers::Signer;
    use alloy::sol_types::SolStruct as _;
    use reqwest::header::HeaderMap;

    use crate::{Result, Timestamp};

    pub(crate) const POLY_ADDRESS: &str = "POLY_ADDRESS";
    pub(crate) const POLY_NONCE: &str = "POLY_NONCE";
    pub(crate) const POLY_SIGNATURE: &str = "POLY_SIGNATURE";
    pub(crate) const POLY_TIMESTAMP: &str = "POLY_TIMESTAMP";

    sol! {
        #[non_exhaustive]
        struct ClobAuth {
            address address;
            string  timestamp;
            uint256 nonce;
            string  message;
        }
    }

    /// Returns the [`HeaderMap`] needed to obtain [`Credentials`] .
    pub(crate) async fn create_headers<S: Signer>(
        signer: &S,
        chain_id: ChainId,
        timestamp: Timestamp,
        nonce: Option<u32>,
    ) -> Result<HeaderMap> {
        let naive_nonce = nonce.unwrap_or(0);

        let auth = ClobAuth {
            address: signer.address(),
            timestamp: timestamp.to_string(),
            nonce: U256::from(naive_nonce),
            message: "This message attests that I control the given wallet".to_owned(),
        };

        let domain = Eip712Domain {
            name: Some(Cow::Borrowed("ClobAuthDomain")),
            version: Some(Cow::Borrowed("1")),
            chain_id: Some(U256::from(chain_id)),
            ..Eip712Domain::default()
        };

        let hash = auth.eip712_signing_hash(&domain);
        let signature = signer.sign_hash(&hash).await?;

        let mut map = HeaderMap::new();
        map.insert(
            POLY_ADDRESS,
            signer.address().encode_hex_with_prefix().parse()?,
        );
        map.insert(POLY_NONCE, naive_nonce.to_string().parse()?);
        map.insert(POLY_SIGNATURE, signature.to_string().parse()?);
        map.insert(POLY_TIMESTAMP, timestamp.to_string().parse()?);

        Ok(map)
    }
}

#[cfg(feature = "clob")]
pub(crate) mod l2 {
    use reqwest::Request;
    use reqwest::header::HeaderMap;
    use secrecy::ExposeSecret as _;

    use crate::auth::state::Authenticated;
    use crate::auth::{Kind, hmac, to_message};
    use crate::{Result, Timestamp};

    pub(crate) const POLY_ADDRESS: &str = "POLY_ADDRESS";
    pub(crate) const POLY_API_KEY: &str = "POLY_API_KEY";
    pub(crate) const POLY_PASSPHRASE: &str = "POLY_PASSPHRASE";
    pub(crate) const POLY_SIGNATURE: &str = "POLY_SIGNATURE";
    pub(crate) const POLY_TIMESTAMP: &str = "POLY_TIMESTAMP";

    /// Returns the [`Headers`] needed to interact with any authenticated endpoints.
    pub(crate) async fn create_headers<K: Kind>(
        state: &Authenticated<K>,
        request: &Request,
        timestamp: Timestamp,
    ) -> Result<HeaderMap> {
        let credentials = &state.credentials;
        let signature = hmac(&credentials.secret, &to_message(request, timestamp))?;

        let mut map = HeaderMap::new();

        map.insert(POLY_ADDRESS, state.address.to_checksum(None).parse()?);
        map.insert(POLY_API_KEY, state.credentials.key.to_string().parse()?);
        map.insert(
            POLY_PASSPHRASE,
            state.credentials.passphrase.expose_secret().parse()?,
        );
        map.insert(POLY_SIGNATURE, signature.parse()?);
        map.insert(POLY_TIMESTAMP, timestamp.to_string().parse()?);

        let extra_headers = state.kind.extra_headers(request, timestamp).await?;

        map.extend(extra_headers);

        Ok(map)
    }
}

// These helpers back `l2::create_headers` and are covered by tests in this module.
// Gated so the `lib` target without `clob` doesn't flag them as dead code.
#[cfg(any(feature = "clob", test))]
#[must_use]
fn to_message(request: &Request, timestamp: Timestamp) -> String {
    let method = request.method();
    let body = request.body().and_then(body_to_string).unwrap_or_default();
    let path = request.url().path();

    format!("{timestamp}{method}{path}{body}")
}

#[cfg(any(feature = "clob", test))]
#[must_use]
fn body_to_string(body: &Body) -> Option<String> {
    body.as_bytes()
        .map(String::from_utf8_lossy)
        .map(|b| b.replace('\'', "\""))
}

#[cfg(any(feature = "clob", test))]
fn hmac(secret: &SecretString, message: &str) -> Result<String> {
    let decoded_secret = URL_SAFE.decode(secret.expose_secret())?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&decoded_secret)?;
    mac.update(message.as_bytes());

    let result = mac.finalize().into_bytes();
    Ok(URL_SAFE.encode(result))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    #[cfg(feature = "clob")]
    use alloy::signers::local::LocalSigner;
    use reqwest::{Client, Method, RequestBuilder};
    use serde_json::json;
    use url::Url;
    use uuid::Uuid;

    use super::*;
    #[cfg(feature = "clob")]
    use crate::auth::state::Authenticated;
    #[cfg(feature = "clob")]
    use crate::types::address;
    #[cfg(feature = "clob")]
    use crate::{AMOY, Result};

    #[cfg(feature = "clob")]
    const PRIVATE_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    #[cfg(feature = "clob")]
    #[tokio::test]
    async fn l1_headers_should_succeed() -> anyhow::Result<()> {
        let signer = LocalSigner::from_str(PRIVATE_KEY)?.with_chain_id(Some(AMOY));

        let headers = l1::create_headers(&signer, AMOY, 10_000_000, Some(23)).await?;

        assert_eq!(
            signer.address(),
            address!("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266")
        );
        assert_eq!(
            headers[l1::POLY_ADDRESS],
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
        assert_eq!(headers[l1::POLY_NONCE], "23");
        assert_eq!(
            headers[l1::POLY_SIGNATURE],
            "0xf62319a987514da40e57e2f4d7529f7bac38f0355bd88bb5adbb3768d80de6c1682518e0af677d5260366425f4361e7b70c25ae232aff0ab2331e2b164a1aedc1b"
        );
        assert_eq!(headers[l1::POLY_TIMESTAMP], "10000000");

        Ok(())
    }

    #[cfg(feature = "clob")]
    #[tokio::test]
    async fn l2_headers_should_succeed() -> anyhow::Result<()> {
        let signer = LocalSigner::from_str(PRIVATE_KEY)?;

        let authenticated = Authenticated {
            address: signer.address(),
            credentials: Credentials {
                key: Uuid::nil(),
                passphrase: SecretString::from(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                ),
                secret: SecretString::from(
                    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned(),
                ),
            },
            kind: Normal,
        };

        let request = Request::new(Method::GET, Url::parse("http://localhost/")?);
        let headers = l2::create_headers(&authenticated, &request, 1).await?;

        assert_eq!(
            headers[l2::POLY_ADDRESS],
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        );
        assert_eq!(
            headers[l2::POLY_PASSPHRASE],
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(headers[l2::POLY_API_KEY], Uuid::nil().to_string());
        assert_eq!(
            headers[l2::POLY_SIGNATURE],
            "eHaylCwqRSOa2LFD77Nt_SaTpbsxzN8eTEI3LryhEj4="
        );
        assert_eq!(headers[l2::POLY_TIMESTAMP], "1");

        Ok(())
    }

    #[test]
    fn request_args_should_succeed() -> Result<()> {
        let request = Request::new(Method::POST, Url::parse("http://localhost/path")?);
        let request = RequestBuilder::from_parts(Client::new(), request)
            .json(&json!({"foo": "bar"}))
            .build()?;

        let timestamp = 1;

        assert_eq!(
            to_message(&request, timestamp),
            r#"1POST/path{"foo":"bar"}"#
        );

        Ok(())
    }

    #[test]
    fn hmac_succeeds() -> Result<()> {
        let json = json!({
            "hash": "0x123"
        });

        let method = Method::from_str("test-sign")
            .expect("To avoid needing an error variant just for one test");
        let request = Request::new(method, Url::parse("http://localhost/orders")?);
        let request = RequestBuilder::from_parts(Client::new(), request)
            .json(&json)
            .build()?;

        let message = to_message(&request, 1_000_000);
        let signature = hmac(
            &SecretString::from("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned()),
            &message,
        )?;

        assert_eq!(message, r#"1000000test-sign/orders{"hash":"0x123"}"#);
        assert_eq!(signature, "4gJVbox-R6XlDK4nlaicig0_ANVL1qdcahiL8CXfXLM=");

        Ok(())
    }

    #[test]
    fn credentials_key_returns_api_key() {
        let key = Uuid::new_v4();
        let credentials = Credentials::new(key, "secret".to_owned(), "passphrase".to_owned());
        assert_eq!(credentials.key(), key);
    }

    #[test]
    fn debug_does_not_expose_secrets() {
        let secret_value = "my_super_secret_value_12345";
        let passphrase_value = "my_super_secret_passphrase_67890";
        let credentials = Credentials::new(
            Uuid::nil(),
            secret_value.to_owned(),
            passphrase_value.to_owned(),
        );

        let debug_output = format!("{credentials:?}");

        assert!(
            !debug_output.contains(secret_value),
            "Debug output should NOT contain the secret value. Got: {debug_output}"
        );
        assert!(
            !debug_output.contains(passphrase_value),
            "Debug output should NOT contain the passphrase value. Got: {debug_output}"
        );
    }
}
