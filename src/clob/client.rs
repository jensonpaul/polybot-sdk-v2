use std::borrow::Cow;
use std::marker::PhantomData;
use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
#[cfg(feature = "heartbeats")]
use std::time::Duration;

use alloy::dyn_abi::Eip712Domain;
use alloy::primitives::{Signature, U256, keccak256};
use alloy::signers::Signer;
use alloy::sol_types::SolStruct as _;
use alloy::sol_types::SolValue as _;
use async_stream::try_stream;
use bon::Builder;
use chrono::{NaiveDate, Utc};
use dashmap::DashMap;
use futures::Stream;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client as ReqwestClient, Method, Request};
use serde_json::json;
#[cfg(all(feature = "tracing", feature = "heartbeats"))]
use tracing::{debug, error};
use url::Url;
use uuid::Uuid;
#[cfg(feature = "heartbeats")]
use {tokio::sync::oneshot::Receiver, tokio::time, tokio_util::sync::CancellationToken};

use crate::auth::state::{Authenticated, State, Unauthenticated};
use crate::auth::{Credentials, Kind, Normal};
use crate::clob::order_builder::{Limit, Market, OrderBuilder, generate_seed};
use crate::clob::types::request::{
    BalanceAllowanceRequest, CancelMarketOrderRequest, DeleteNotificationsRequest,
    LastTradePriceRequest, MidpointRequest, OrderBookSummaryRequest, OrdersRequest,
    PriceHistoryRequest, PriceRequest, SpreadRequest, TradesRequest, UpdateBalanceAllowanceRequest,
    UserRewardsEarningRequest,
};
use crate::clob::types::response::{
    ApiKeysResponse, BalanceAllowanceResponse, BanStatusResponse, BuilderApiKeyResponse,
    BuilderFeeRateResponse, BuilderTradeResponse, CancelOrdersResponse, ClobMarketInfoResponse,
    CurrentRewardResponse, FeeInfo, FeeRateResponse, GeoblockResponse, HeartbeatResponse,
    LastTradePriceResponse, LastTradesPricesResponse, MarketByTokenResponse, MarketResponse,
    MarketRewardResponse, MidpointResponse, MidpointsResponse, NegRiskResponse,
    NotificationResponse, OpenOrderResponse, OrderBookSummaryResponse, OrderScoringResponse,
    OrdersScoringResponse, Page, PostOrderResponse, PriceHistoryResponse, PriceResponse,
    PricesResponse, ReadonlyApiKeyResponse, RewardsPercentagesResponse, SimplifiedMarketResponse,
    SpreadResponse, SpreadsResponse, TickSizeResponse, TotalUserEarningResponse, TradeResponse,
    UserEarningResponse, UserRewardsEarningResponse,
};
#[cfg(feature = "rfq")]
use crate::clob::types::{
    AcceptRfqQuoteRequest, AcceptRfqQuoteResponse, ApproveRfqOrderRequest, ApproveRfqOrderResponse,
    CancelRfqQuoteRequest, CancelRfqRequestRequest, CreateRfqQuoteRequest, CreateRfqQuoteResponse,
    CreateRfqRequestRequest, CreateRfqRequestResponse, RfqQuote, RfqQuotesRequest, RfqRequest,
    RfqRequestsRequest,
};
use crate::clob::types::{
    Amount, OrderPayload, OrderSignature, OrderType, Side, SignableOrder, SignatureType,
    SignedOrder, TickSize,
};
use crate::error::{Error, Kind as ErrorKind, Synchronization};
use crate::types::{Address, B256, Decimal};
use crate::{
    AMOY, POLYGON, Result, Timestamp, ToQueryParams as _, auth, contract_config,
    derive_proxy_wallet, derive_safe_wallet,
};

const ORDER_NAME: Option<Cow<'static, str>> = Some(Cow::Borrowed("Polymarket CTF Exchange"));
const VERSION_V1: Option<Cow<'static, str>> = Some(Cow::Borrowed("1"));
const VERSION_V2: Option<Cow<'static, str>> = Some(Cow::Borrowed("2"));
const DEPOSIT_WALLET_NAME: &str = "DepositWallet";
const DEPOSIT_WALLET_VERSION: &str = "1";
const ORDER_TYPE_STRING: &str = concat!(
    "Order(uint256 salt,address maker,address signer,uint256 tokenId,",
    "uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,",
    "uint256 timestamp,bytes32 metadata,bytes32 builder)"
);
const SOLADY_TYPE_STRING: &str = concat!(
    "TypedDataSign(Order contents,string name,string version,uint256 chainId,",
    "address verifyingContract,bytes32 salt)",
    "Order(uint256 salt,address maker,address signer,uint256 tokenId,",
    "uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,",
    "uint256 timestamp,bytes32 metadata,bytes32 builder)"
);

const TERMINAL_CURSOR: &str = "LTE="; // base64("-1")

pub(crate) const ORDER_VERSION_MISMATCH_ERROR: &str = "order_version_mismatch";

fn push_hex(out: &mut String, bytes: &[u8]) {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    out.reserve(bytes.len() * 2);
    for byte in bytes {
        out.push(LUT[(byte >> 4) as usize] as char);
        out.push(LUT[(byte & 0x0f) as usize] as char);
    }
}

fn signature_hex_no_prefix(signature: &Signature) -> String {
    let signature = signature.to_string();
    signature
        .strip_prefix("0x")
        .unwrap_or(&signature)
        .to_owned()
}

/// The type used to build a request to authenticate the inner [`Client<Unauthorized>`]. Calling
/// `authenticate` on this will elevate that inner `client` into an [`Client<Authenticated<K>>`].
pub struct AuthenticationBuilder<'signer, S: Signer, K: Kind = Normal> {
    
    client: Client<Unauthenticated>,
    
    signer: &'signer S,
    /// If [`Credentials`] are supplied, then those are used instead of making new calls to obtain one.
    credentials: Option<Credentials>,
    /// An optional `nonce` value, when `credentials` are not present, to pass along to the call to
    /// create or derive [`Credentials`].
    nonce: Option<u32>,
    /// The [`Kind`] that this [`AuthenticationBuilder`] exhibits. Used to generate additional
    /// headers for different types of authentication, e.g. Builder.
    kind: K,
    /// The optional [`Address`] used to represent the funder for this `client`. If a funder is set
    /// then `signature_type` must match `Some(SignatureType::Proxy | SignatureType::GnosisSafe | SignatureType::Poly1271)`.
    /// Conversely, if funder is not set, then `signature_type` must be `Some(SignatureType::Eoa)`.
    funder: Option<Address>,
    /// The optional [`SignatureType`], see `funder` for more information.
    signature_type: Option<SignatureType>,
    /// The optional salt/seed generator for use in creating [`SignableOrder`]s
    salt_generator: Option<fn() -> u64>,
}

impl<S: Signer, K: Kind> AuthenticationBuilder<'_, S, K> {
    #[must_use]
    pub fn nonce(mut self, nonce: u32) -> Self {
        self.nonce = Some(nonce);
        self
    }

    #[must_use]
    pub fn credentials(mut self, credentials: Credentials) -> Self {
        self.credentials = Some(credentials);
        self
    }

    #[must_use]
    pub fn funder(mut self, funder: Address) -> Self {
        self.funder = Some(funder);
        self
    }

    #[must_use]
    pub fn signature_type(mut self, signature_type: SignatureType) -> Self {
        self.signature_type = Some(signature_type);
        self
    }

    #[must_use]
    pub fn salt_generator(mut self, salt_generator: fn() -> u64) -> Self {
        self.salt_generator = Some(salt_generator);
        self
    }

    pub async fn authenticate(self) -> Result<Client<Authenticated<K>>> {
        let inner = Arc::into_inner(self.client.inner).ok_or(Synchronization)?;

        let chain_id = match self.signer.chain_id() {
            Some(chain) if chain == POLYGON || chain == AMOY => chain,
            Some(chain) => {
                return Err(Error::validation(format!(
                    "Only Polygon and AMOY are supported, got {chain}"
                )));
            }
            None => {
                return Err(Error::validation(
                    "Chain id not set, be sure to provide one on the signer",
                ));
            }
        };

        let funder = match (self.funder, self.signature_type) {
            (None, Some(SignatureType::Proxy)) => {
                let derived =
                    derive_proxy_wallet(self.signer.address(), chain_id).ok_or_else(|| {
                        Error::validation(
                            "Proxy wallet derivation not supported on this chain. \
                             Please provide an explicit funder address.",
                        )
                    })?;
                Some(derived)
            }
            (None, Some(SignatureType::GnosisSafe)) => {
                let derived =
                    derive_safe_wallet(self.signer.address(), chain_id).ok_or_else(|| {
                        Error::validation(
                            "Safe wallet derivation not supported on this chain. \
                             Please provide an explicit funder address.",
                        )
                    })?;
                Some(derived)
            }
            (funder, _) => funder,
        };

        match (funder, self.signature_type) {
            (Some(_), Some(sig @ SignatureType::Eoa)) => {
                return Err(Error::validation(format!(
                    "Cannot have a funder address with a {sig} signature type"
                )));
            }
            (None, Some(SignatureType::Poly1271)) => {
                return Err(Error::validation(
                    "A deposit wallet funder address is required with a Poly1271 signature type",
                ));
            }
            (
                Some(Address::ZERO),
                Some(
                    sig @ (SignatureType::Proxy
                    | SignatureType::GnosisSafe
                    | SignatureType::Poly1271),
                ),
            ) => {
                return Err(Error::validation(format!(
                    "Cannot have a zero funder address with a {sig} signature type"
                )));
            }
            
            _ => {}
        }

        let credentials = match self.credentials {
            Some(_) if self.nonce.is_some() => {
                return Err(Error::validation(
                    "Credentials and nonce are both set. If nonce is set, then you must not supply credentials",
                ));
            }
            Some(credentials) => credentials,
            None => {
                inner
                    .create_or_derive_api_key(self.signer, self.nonce)
                    .await?
            }
        };

        let state = Authenticated {
            address: self.signer.address(),
            credentials,
            kind: self.kind,
        };

        #[cfg_attr(
            not(feature = "heartbeats"),
            expect(
                unused_mut,
                reason = "Modifier only needed when heartbeats feature is enabled"
            )
        )]
        let mut client = Client {
            inner: Arc::new(ClientInner {
                state,
                config: inner.config,
                host: inner.host,
                geoblock_host: inner.geoblock_host,
                client: inner.client,
                tick_sizes: inner.tick_sizes,
                neg_risk: inner.neg_risk,
                fee_rate_bps: inner.fee_rate_bps,
                fee_infos: inner.fee_infos,
                token_condition_map: inner.token_condition_map,
                builder_fee_rates: inner.builder_fee_rates,
                cached_version: inner.cached_version,
                funder,
                signature_type: self.signature_type.unwrap_or(SignatureType::Eoa),
                salt_generator: self.salt_generator.unwrap_or(generate_seed),
            }),
            #[cfg(feature = "heartbeats")]
            heartbeat_token: DroppingCancellationToken(None),
        };

        #[cfg(feature = "heartbeats")]
        Client::<Authenticated<K>>::start_heartbeats(&mut client)?;

        Ok(client)
    }
}

#[derive(Clone, Debug)]
pub struct Client<S: State = Unauthenticated> {
    inner: Arc<ClientInner<S>>,
    #[cfg(feature = "heartbeats")]
    
    heartbeat_token: DroppingCancellationToken,
}

#[cfg(feature = "heartbeats")]

#[derive(Clone, Debug, Default)]
struct DroppingCancellationToken(Option<(CancellationToken, Arc<Receiver<()>>)>);

#[cfg(feature = "heartbeats")]
impl DroppingCancellationToken {
    
    pub(crate) async fn cancel_and_wait(&mut self) -> Result<()> {
        if let Some((token, rx)) = self.0.take() {
            return match Arc::try_unwrap(rx) {
                
                Ok(inner) => {
                    token.cancel();
                    _ = inner.await;
                    Ok(())
                }
                
                Err(original) => {
                    *self = DroppingCancellationToken(Some((token, original)));
                    Err(Synchronization.into())
                }
            };
        }

        Ok(())
    }
}

#[cfg(feature = "heartbeats")]
impl Drop for DroppingCancellationToken {
    fn drop(&mut self) {
        if let Some((token, _)) = self.0.take() {
            token.cancel();
        }
    }
}

impl Default for Client<Unauthenticated> {
    fn default() -> Self {
        Client::new("https://clob-v2.polymarket.com", Config::default())
            .expect("Client with default endpoint should succeed")
    }
}

#[derive(Clone, Debug, Builder)]
pub struct Config {
    
    #[builder(default)]
    use_server_time: bool,
    
    #[builder(into)]
    geoblock_host: Option<String>,
    
    builder_code: Option<B256>,
    #[cfg(feature = "heartbeats")]
    #[builder(default = Duration::from_secs(5))]
    
    heartbeat_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            use_server_time: false,
            geoblock_host: None,
            builder_code: None,
            #[cfg(feature = "heartbeats")]
            heartbeat_interval: Duration::from_secs(5),
        }
    }
}

const DEFAULT_GEOBLOCK_HOST: &str = "https://polymarket.com";

#[derive(Debug)]
struct ClientInner<S: State> {
    config: Config,
    
    state: S,
    
    host: Url,
    
    geoblock_host: Url,
    
    client: ReqwestClient,
    
    tick_sizes: DashMap<U256, TickSize>,
    
    neg_risk: DashMap<U256, bool>,
    
    fee_rate_bps: DashMap<U256, FeeRateResponse>,
    
    fee_infos: DashMap<U256, FeeInfo>,
    
    token_condition_map: DashMap<U256, B256>,
    
    builder_fee_rates: DashMap<B256, BuilderFeeRateResponse>,
    
    cached_version: AtomicU32,
    
    funder: Option<Address>,
    
    signature_type: SignatureType,
    
    salt_generator: fn() -> u64,
}

impl<S: State> ClientInner<S> {
    pub async fn server_time(&self) -> Result<Timestamp> {
        let request = self
            .client
            .request(Method::GET, format!("{}time", self.host))
            .build()?;

        crate::request(&self.client, request, None).await
    }
}

impl ClientInner<Unauthenticated> {
    pub async fn create_api_key<S: Signer>(
        &self,
        signer: &S,
        nonce: Option<u32>,
    ) -> Result<Credentials> {
        let request = self
            .client
            .request(Method::POST, format!("{}auth/api-key", self.host))
            .build()?;
        let headers = self.create_headers(signer, nonce).await?;

        crate::request(&self.client, request, Some(headers)).await
    }

    pub async fn derive_api_key<S: Signer>(
        &self,
        signer: &S,
        nonce: Option<u32>,
    ) -> Result<Credentials> {
        let request = self
            .client
            .request(Method::GET, format!("{}auth/derive-api-key", self.host))
            .build()?;
        let headers = self.create_headers(signer, nonce).await?;

        crate::request(&self.client, request, Some(headers)).await
    }

    async fn create_or_derive_api_key<S: Signer>(
        &self,
        signer: &S,
        nonce: Option<u32>,
    ) -> Result<Credentials> {
        match self.create_api_key(signer, nonce).await {
            Ok(creds) => Ok(creds),
            Err(err) if err.kind() == ErrorKind::Status => {
                
                self.derive_api_key(signer, nonce).await
            }
            Err(err) => Err(err),
        }
    }

    async fn create_headers<S: Signer>(&self, signer: &S, nonce: Option<u32>) -> Result<HeaderMap> {
        let chain_id = signer.chain_id().ok_or(Error::validation(
            "Chain id not set, be sure to provide one on the signer",
        ))?;

        let timestamp = if self.config.use_server_time {
            self.server_time().await?
        } else {
            Utc::now().timestamp()
        };

        auth::l1::create_headers(signer, chain_id, timestamp, nonce).await
    }
}

impl<S: State> Client<S> {
    
    #[must_use]
    pub fn host(&self) -> &Url {
        &self.inner.host
    }

    pub fn invalidate_internal_caches(&self) {
        self.inner.tick_sizes.clear();
        self.inner.fee_rate_bps.clear();
        self.inner.neg_risk.clear();
        self.inner.builder_fee_rates.clear();
    }

    pub fn set_tick_size(&self, token_id: U256, tick_size: TickSize) {
        self.inner.tick_sizes.insert(token_id, tick_size);
    }

    pub fn set_neg_risk(&self, token_id: U256, neg_risk: bool) {
        self.inner.neg_risk.insert(token_id, neg_risk);
    }

    pub fn set_fee_rate_bps(&self, token_id: U256, fee_rate_bps: u32) {
        self.inner.fee_rate_bps.insert(
            token_id,
            FeeRateResponse {
                base_fee: fee_rate_bps,
            },
        );
    }

    pub fn set_fee_rate(&self, token_id: U256, fee_rate: FeeRateResponse) {
        self.inner.fee_rate_bps.insert(token_id, fee_rate);
    }

    pub async fn ok(&self) -> Result<String> {
        let request = self
            .client()
            .request(Method::GET, self.host().to_owned())
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn server_time(&self) -> Result<Timestamp> {
        self.inner.server_time().await
    }

    pub async fn version(&self) -> Result<u32> {
        self.resolve_version(false).await
    }

    pub(crate) async fn resolve_version(&self, force: bool) -> Result<u32> {
        #[derive(serde::Deserialize)]
        struct VersionBody {
            version: u32,
        }

        if !force {
            let cached = self.inner.cached_version.load(Ordering::Relaxed);
            if cached != 0 {
                return Ok(cached);
            }
        }

        let request = self
            .client()
            .request(Method::GET, format!("{}version", self.host()))
            .build()?;
        let body: VersionBody = crate::request(&self.inner.client, request, None).await?;
        self.inner
            .cached_version
            .store(body.version, Ordering::Relaxed);
        Ok(body.version)
    }

    pub async fn midpoint(&self, request: &MidpointRequest) -> Result<MidpointResponse> {
        let params = request.query_params(None);
        let request = self
            .client()
            .request(Method::GET, format!("{}midpoint{params}", self.host()))
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn midpoints(&self, requests: &[MidpointRequest]) -> Result<MidpointsResponse> {
        let request = self
            .client()
            .request(Method::POST, format!("{}midpoints", self.host()))
            .json(requests)
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn price(&self, request: &PriceRequest) -> Result<PriceResponse> {
        let params = request.query_params(None);
        let request = self
            .client()
            .request(Method::GET, format!("{}price{params}", self.host()))
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn prices(&self, requests: &[PriceRequest]) -> Result<PricesResponse> {
        let request = self
            .client()
            .request(Method::POST, format!("{}prices", self.host()))
            .json(requests)
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn price_history(
        &self,
        request: &PriceHistoryRequest,
    ) -> Result<PriceHistoryResponse> {
        let params = request.query_params(None);
        let req = self.client().request(
            Method::GET,
            format!("{}prices-history{params}", self.host()),
        );

        crate::request(&self.inner.client, req.build()?, None).await
    }

    pub async fn spread(&self, request: &SpreadRequest) -> Result<SpreadResponse> {
        let params = request.query_params(None);
        let request = self
            .client()
            .request(Method::GET, format!("{}spread{params}", self.host()))
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn spreads(&self, requests: &[SpreadRequest]) -> Result<SpreadsResponse> {
        let request = self
            .client()
            .request(Method::POST, format!("{}spreads", self.host()))
            .json(requests)
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn tick_size(&self, token_id: U256) -> Result<TickSizeResponse> {
        if let Some(tick_size) = self.inner.tick_sizes.get(&token_id) {
            #[cfg(feature = "tracing")]
            tracing::trace!(token_id = %token_id, tick_size = ?tick_size.value(), "cache hit: tick_size");
            return Ok(TickSizeResponse {
                minimum_tick_size: *tick_size,
            });
        }

        #[cfg(feature = "tracing")]
        tracing::trace!(token_id = %token_id, "cache miss: tick_size");

        let request = self
            .client()
            .request(Method::GET, format!("{}tick-size", self.host()))
            .query(&[("token_id", token_id.to_string())])
            .build()?;

        let response =
            crate::request::<TickSizeResponse>(&self.inner.client, request, None).await?;

        self.inner
            .tick_sizes
            .insert(token_id, response.minimum_tick_size);

        #[cfg(feature = "tracing")]
        tracing::trace!(token_id = %token_id, "cached tick_size");

        Ok(response)
    }

    pub async fn neg_risk(&self, token_id: U256) -> Result<NegRiskResponse> {
        if let Some(neg_risk) = self.inner.neg_risk.get(&token_id) {
            #[cfg(feature = "tracing")]
            tracing::trace!(token_id = %token_id, neg_risk = *neg_risk, "cache hit: neg_risk");
            return Ok(NegRiskResponse {
                neg_risk: *neg_risk,
            });
        }

        #[cfg(feature = "tracing")]
        tracing::trace!(token_id = %token_id, "cache miss: neg_risk");

        let request = self
            .client()
            .request(Method::GET, format!("{}neg-risk", self.host()))
            .query(&[("token_id", token_id.to_string())])
            .build()?;

        let response = crate::request::<NegRiskResponse>(&self.inner.client, request, None).await?;

        self.inner.neg_risk.insert(token_id, response.neg_risk);

        #[cfg(feature = "tracing")]
        tracing::trace!(token_id = %token_id, "cached neg_risk");

        Ok(response)
    }

    pub async fn fee_rate_bps(&self, token_id: U256) -> Result<FeeRateResponse> {
        if let Some(cached) = self.inner.fee_rate_bps.get(&token_id) {
            #[cfg(feature = "tracing")]
            tracing::trace!(token_id = %token_id, base_fee = cached.base_fee, "cache hit: fee_rate_bps");
            return Ok(cached.clone());
        }

        #[cfg(feature = "tracing")]
        tracing::trace!(token_id = %token_id, "cache miss: fee_rate_bps");

        let request = self
            .client()
            .request(Method::GET, format!("{}fee-rate", self.host()))
            .query(&[("token_id", token_id.to_string())])
            .build()?;

        let response = crate::request::<FeeRateResponse>(&self.inner.client, request, None).await?;

        self.inner.fee_rate_bps.insert(token_id, response.clone());

        #[cfg(feature = "tracing")]
        tracing::trace!(token_id = %token_id, "cached fee_rate_bps");

        Ok(response)
    }

    pub(crate) async fn resolve_fee_rate_bps(
        &self,
        token_id: U256,
        user_fee_rate_bps: Option<u32>,
    ) -> Result<u32> {
        let market_fee = self.fee_rate_bps(token_id).await?.base_fee;
        if let Some(user) = user_fee_rate_bps
            && market_fee > 0
            && user != market_fee
        {
            return Err(Error::validation(format!(
                "invalid user-provided fee rate {user}; market fee rate must be {market_fee}"
            )));
        }
        Ok(market_fee)
    }

    pub async fn fee_exponent(&self, token_id: U256) -> Result<u32> {
        Ok(self.fee_info(token_id).await?.exponent)
    }

    pub async fn check_geoblock(&self) -> Result<GeoblockResponse> {
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}api/geoblock", self.inner.geoblock_host),
            )
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn order_book(
        &self,
        request: &OrderBookSummaryRequest,
    ) -> Result<OrderBookSummaryResponse> {
        let params = request.query_params(None);
        let request = self
            .client()
            .request(Method::GET, format!("{}book{params}", self.host()))
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn order_books(
        &self,
        requests: &[OrderBookSummaryRequest],
    ) -> Result<Vec<OrderBookSummaryResponse>> {
        let request = self
            .client()
            .request(Method::POST, format!("{}books", self.host()))
            .json(requests)
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub fn order_book_hash(&self, book: &OrderBookSummaryResponse) -> Result<String> {
        book.hash()
    }

    pub async fn last_trade_price(
        &self,
        request: &LastTradePriceRequest,
    ) -> Result<LastTradePriceResponse> {
        let params = request.query_params(None);
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}last-trade-price{params}", self.host()),
            )
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn last_trades_prices(
        &self,
        token_ids: &[LastTradePriceRequest],
    ) -> Result<Vec<LastTradesPricesResponse>> {
        let request = self
            .client()
            .request(Method::GET, format!("{}last-trades-prices", self.host()))
            .json(token_ids)
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn market(&self, condition_id: &str) -> Result<MarketResponse> {
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}markets/{condition_id}", self.host()),
            )
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn markets(&self, next_cursor: Option<String>) -> Result<Page<MarketResponse>> {
        let cursor = next_cursor.map_or(String::new(), |c| format!("?next_cursor={c}"));
        let request = self
            .client()
            .request(Method::GET, format!("{}markets{cursor}", self.host()))
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn sampling_markets(
        &self,
        next_cursor: Option<String>,
    ) -> Result<Page<MarketResponse>> {
        let cursor = next_cursor.map_or(String::new(), |c| format!("?next_cursor={c}"));
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}sampling-markets{cursor}", self.host()),
            )
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn simplified_markets(
        &self,
        next_cursor: Option<String>,
    ) -> Result<Page<SimplifiedMarketResponse>> {
        let cursor = next_cursor.map_or(String::new(), |c| format!("?next_cursor={c}"));
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}simplified-markets{cursor}", self.host()),
            )
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub async fn sampling_simplified_markets(
        &self,
        next_cursor: Option<String>,
    ) -> Result<Page<SimplifiedMarketResponse>> {
        let cursor = next_cursor.map_or(String::new(), |c| format!("?next_cursor={c}"));
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}sampling-simplified-markets{cursor}", self.host()),
            )
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    pub fn stream_data<'client, Call, Fut, Data>(
        &'client self,
        call: Call,
    ) -> impl Stream<Item = Result<Data>> + 'client
    where
        Call: Fn(&'client Client<S>, Option<String>) -> Fut + 'client,
        Fut: Future<Output = Result<Page<Data>>> + 'client,
        Data: 'client,
    {
        try_stream! {
            let mut cursor: Option<String> = None;

            loop {
                let page = call(self, mem::take(&mut cursor)).await?;

                for item in page.data {
                    yield item
                }

                if page.next_cursor == TERMINAL_CURSOR {
                    break;
                }

                cursor = Some(page.next_cursor);
            }
        }
    }

    /// Returns combined CLOB market info for a condition ID.
    ///
    /// Also populates the local caches for tick size, neg risk, and fee rate
    /// for all tokens in the market.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the condition ID is invalid.
    pub async fn clob_market_info(&self, condition_id: &str) -> Result<ClobMarketInfoResponse> {
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}clob-markets/{condition_id}", self.host()),
            )
            .build()?;

        let response: ClobMarketInfoResponse =
            crate::request(&self.inner.client, request, None).await?;

        // Do NOT populate `fee_rate_bps` here — that cache belongs to the V1 `/fee-rate`
        // endpoint, which uses different units.
        let fee_info = response
            .fee_details
            .as_ref()
            .map_or(FeeInfo::default(), |fd| FeeInfo {
                rate: fd.rate,
                exponent: fd.exponent,
            });
        for token in response.tokens.iter().flatten() {
            self.inner
                .tick_sizes
                .insert(token.token_id, response.min_tick_size);
            self.inner
                .neg_risk
                .insert(token.token_id, response.neg_risk);
            self.inner.fee_infos.insert(token.token_id, fee_info);
            self.inner
                .token_condition_map
                .insert(token.token_id, response.condition_id);
        }

        Ok(response)
    }

    /// Primes the tick-size, neg-risk, and fee caches for `token_id` from
    /// `/clob-markets/{id}`, resolving the condition via `/markets-by-token` if needed.
    ///
    /// # Errors
    ///
    /// Returns an error if the market lookup or the clob-market-info fetch fails.
    pub(crate) async fn ensure_market_info_cached(&self, token_id: U256) -> Result<()> {
        if self.inner.fee_infos.contains_key(&token_id) {
            return Ok(());
        }
        let condition_id = if let Some(cid) = self.inner.token_condition_map.get(&token_id) {
            *cid
        } else {
            let market = self.market_by_token(token_id).await?;
            self.inner
                .token_condition_map
                .insert(token_id, market.condition_id);
            market.condition_id
        };
        self.clob_market_info(&condition_id.to_string()).await?;
        Ok(())
    }

    /// Returns V2 fee parameters for `token_id`, priming the cache as needed.
    ///
    /// # Errors
    ///
    /// Returns an error if market metadata cannot be resolved.
    pub(crate) async fn fee_info(&self, token_id: U256) -> Result<FeeInfo> {
        self.ensure_market_info_cached(token_id).await?;
        Ok(self
            .inner
            .fee_infos
            .get(&token_id)
            .map(|e| *e)
            .unwrap_or_default())
    }

    /// Looks up a market by token ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the token ID is invalid.
    pub async fn market_by_token(&self, token_id: U256) -> Result<MarketByTokenResponse> {
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}markets-by-token/{token_id}", self.host()),
            )
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    /// Returns raw on-chain trade events for a market condition ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the condition ID is invalid.
    pub async fn market_trades_events(&self, condition_id: &str) -> Result<serde_json::Value> {
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}markets/live-activity/{condition_id}", self.host()),
            )
            .build()?;

        crate::request(&self.inner.client, request, None).await
    }

    /// Calculates the effective fill price for a market order by walking the orderbook.
    ///
    /// The unit of `amount` (USDC vs shares) determines which side of the book is walked
    /// — see [`super::utilities::calculate_market_price`] for the full matrix.
    ///
    /// # Errors
    ///
    /// - Orderbook fetch fails.
    /// - `side == Side::Sell` paired with an `Amount::usdc(_)`.
    /// - `order_type == OrderType::FOK` with insufficient liquidity.
    pub async fn calculate_market_price(
        &self,
        token_id: U256,
        side: Side,
        amount: Amount,
        order_type: OrderType,
    ) -> Result<Decimal> {
        let book = self
            .order_book(&OrderBookSummaryRequest {
                token_id,
                side: None,
            })
            .await?;

        super::utilities::calculate_market_price(&book, side, amount, &order_type)
    }

    fn client(&self) -> &ReqwestClient {
        &self.inner.client
    }
}

impl Client<Unauthenticated> {
    /// Creates a new unauthenticated CLOB client.
    ///
    /// This client can access public API endpoints like market data, prices,
    /// and orderbooks. To place orders or access user-specific endpoints,
    /// use [`Self::authentication_builder`] to upgrade to an authenticated client.
    ///
    /// # Arguments
    ///
    /// * `host` - The CLOB API URL (e.g., <https://clob-v2.polymarket.com>)
    /// * `config` - Client configuration options
    ///
    /// # Errors
    ///
    /// Returns an error if the host URL is invalid or the HTTP client cannot be initialized.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polymarket_client_sdk_v2::clob::{Client, Config};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::new("https://clob-v2.polymarket.com", Config::default())?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(host: &str, config: Config) -> Result<Client<Unauthenticated>> {
        let mut headers = HeaderMap::new();

        headers.insert("User-Agent", HeaderValue::from_static("rs_clob_client"));
        headers.insert("Accept", HeaderValue::from_static("*/*"));
        headers.insert("Connection", HeaderValue::from_static("keep-alive"));
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));

        let client = ReqwestClient::builder().default_headers(headers).build()?;

        let geoblock_host = Url::parse(
            config
                .geoblock_host
                .as_deref()
                .unwrap_or(DEFAULT_GEOBLOCK_HOST),
        )?;

        Ok(Self {
            inner: Arc::new(ClientInner {
                config,
                host: Url::parse(host)?,
                geoblock_host,
                client,
                tick_sizes: DashMap::new(),
                neg_risk: DashMap::new(),
                fee_rate_bps: DashMap::new(),
                fee_infos: DashMap::new(),
                token_condition_map: DashMap::new(),
                builder_fee_rates: DashMap::new(),
                cached_version: AtomicU32::new(0),
                state: Unauthenticated,
                funder: None,
                signature_type: SignatureType::Eoa,
                salt_generator: generate_seed,
            }),
            #[cfg(feature = "heartbeats")]
            heartbeat_token: DroppingCancellationToken(None),
        })
    }

    /// Creates an authentication builder to upgrade this client to authenticated mode.
    ///
    /// Returns an [`AuthenticationBuilder`] that can be configured with credentials
    /// or used to create/derive API keys. Call [`AuthenticationBuilder::authenticate`]
    /// to complete the upgrade to an authenticated client.
    ///
    /// # Arguments
    ///
    /// * `signer` - A wallet signer used to generate authentication signatures
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polymarket_client_sdk_v2::clob::{Client, Config};
    /// use alloy::signers::local::LocalSigner;
    /// use std::str::FromStr;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::new("https://clob-v2.polymarket.com", Config::default())?;
    /// let signer = LocalSigner::from_str("0x...")?;
    ///
    /// let authenticated_client = client
    ///     .authentication_builder(&signer)
    ///     .authenticate()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn authentication_builder<S: Signer>(
        self,
        signer: &S,
    ) -> AuthenticationBuilder<'_, S, Normal> {
        AuthenticationBuilder {
            signer,
            credentials: None,
            nonce: None,
            kind: Normal,
            funder: self.inner.funder,
            signature_type: Some(self.inner.signature_type),
            client: self,
            salt_generator: None,
        }
    }

    pub async fn create_api_key<S: Signer>(
        &self,
        signer: &S,
        nonce: Option<u32>,
    ) -> Result<Credentials> {
        self.inner.create_api_key(signer, nonce).await
    }

    pub async fn derive_api_key<S: Signer>(
        &self,
        signer: &S,
        nonce: Option<u32>,
    ) -> Result<Credentials> {
        self.inner.derive_api_key(signer, nonce).await
    }

    pub async fn create_or_derive_api_key<S: Signer>(
        &self,
        signer: &S,
        nonce: Option<u32>,
    ) -> Result<Credentials> {
        self.inner.create_or_derive_api_key(signer, nonce).await
    }
}

impl<K: Kind> Client<Authenticated<K>> {
    
    #[cfg_attr(
        not(feature = "heartbeats"),
        expect(
            clippy::unused_async,
            unused_mut,
            reason = "Nothing to await or modify when heartbeats are disabled"
        )
    )]
    pub async fn deauthenticate(mut self) -> Result<Client<Unauthenticated>> {
        #[cfg(feature = "heartbeats")]
        self.heartbeat_token.cancel_and_wait().await?;

        let inner = Arc::into_inner(self.inner).ok_or(Synchronization)?;

        Ok(Client::<Unauthenticated> {
            inner: Arc::new(ClientInner {
                state: Unauthenticated,
                host: inner.host,
                geoblock_host: inner.geoblock_host,
                config: inner.config,
                client: inner.client,
                tick_sizes: inner.tick_sizes,
                neg_risk: inner.neg_risk,
                fee_rate_bps: inner.fee_rate_bps,
                fee_infos: inner.fee_infos,
                token_condition_map: inner.token_condition_map,
                builder_fee_rates: inner.builder_fee_rates,
                cached_version: inner.cached_version,
                
                funder: None,
                signature_type: SignatureType::Eoa,
                salt_generator: generate_seed,
            }),
            #[cfg(feature = "heartbeats")]
            heartbeat_token: DroppingCancellationToken(None),
        })
    }

    #[must_use]
    pub fn state(&self) -> &Authenticated<K> {
        &self.inner.state
    }

    #[must_use]
    pub fn address(&self) -> Address {
        self.state().address
    }

    #[must_use]
    pub fn credentials(&self) -> &Credentials {
        &self.state().credentials
    }

    pub async fn api_keys(&self) -> Result<ApiKeysResponse> {
        let request = self
            .client()
            .request(Method::GET, format!("{}auth/api-keys", self.host()))
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn delete_api_key(&self) -> Result<serde_json::Value> {
        let request = self
            .client()
            .request(Method::DELETE, format!("{}auth/api-key", self.host()))
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn closed_only_mode(&self) -> Result<BanStatusResponse> {
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}auth/ban-status/closed-only", self.host()),
            )
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    #[must_use]
    pub fn limit_order(&self) -> OrderBuilder<Limit, K> {
        self.order_builder()
    }

    #[must_use]
    pub fn market_order(&self) -> OrderBuilder<Market, K> {
        self.order_builder()
    }

    #[expect(
        clippy::missing_panics_doc,
        reason = "No need to publicly document as we are guarded by the typestate pattern. \
        We cannot call `sign` without first calling `authenticate`"
    )]
    pub async fn sign<S: Signer>(
        &self,
        signer: &S,
        SignableOrder {
            payload,
            order_type,
            post_only,
            defer_exec,
        }: SignableOrder,
    ) -> Result<SignedOrder> {
        let chain_id = signer
            .chain_id()
            .expect("Validated not none in `authenticate`");

        let token_id = match &payload {
            OrderPayload::V1(p) => p.order.tokenId,
            OrderPayload::V2(p) => p.order.tokenId,
        };
        let neg_risk = self.neg_risk(token_id).await?.neg_risk;
        let config = contract_config(chain_id, neg_risk)
            .ok_or(Error::missing_contract_config(chain_id, neg_risk))?;

        let signature = match &payload {
            OrderPayload::V2(p) => {
                let exchange = config.exchange_v2.ok_or_else(|| {
                    Error::validation(format!(
                        "No V2 exchange contract configured for chain_id={chain_id}, neg_risk={neg_risk}"
                    ))
                })?;
                let domain = Eip712Domain {
                    name: ORDER_NAME,
                    version: VERSION_V2,
                    chain_id: Some(U256::from(chain_id)),
                    verifying_contract: Some(exchange),
                    ..Eip712Domain::default()
                };
                if p.order.signatureType == SignatureType::Poly1271 as u8 {
                    self.sign_poly1271_order(signer, &p.order, &domain, chain_id)
                        .await?
                } else {
                    signer
                        .sign_hash(&p.order.eip712_signing_hash(&domain))
                        .await?
                        .into()
                }
            }
            OrderPayload::V1(p) => {
                let domain = Eip712Domain {
                    name: ORDER_NAME,
                    version: VERSION_V1,
                    chain_id: Some(U256::from(chain_id)),
                    verifying_contract: Some(config.exchange),
                    ..Eip712Domain::default()
                };
                signer
                    .sign_hash(&p.order.eip712_signing_hash(&domain))
                    .await?
                    .into()
            }
        };

        Ok(SignedOrder {
            payload,
            signature,
            order_type,
            owner: self.state().credentials.key,
            post_only,
            defer_exec,
        })
    }

    async fn sign_poly1271_order<S: Signer>(
        &self,
        signer: &S,
        order: &crate::clob::types::OrderV2,
        app_domain: &Eip712Domain,
        chain_id: u64,
    ) -> Result<OrderSignature> {
        let contents_hash = order.eip712_hash_struct();
        let app_domain_separator = app_domain.hash_struct();

        let typed_data_sign_struct_hash = keccak256(
            (
                keccak256(SOLADY_TYPE_STRING.as_bytes()),
                contents_hash,
                keccak256(DEPOSIT_WALLET_NAME.as_bytes()),
                keccak256(DEPOSIT_WALLET_VERSION.as_bytes()),
                U256::from(chain_id),
                order.signer,
                B256::ZERO,
            )
                .abi_encode(),
        );

        let mut digest_input = [0_u8; 66];
        digest_input[0] = 0x19;
        digest_input[1] = 0x01;
        digest_input[2..34].copy_from_slice(app_domain_separator.as_slice());
        digest_input[34..66].copy_from_slice(typed_data_sign_struct_hash.as_slice());
        let digest = keccak256(digest_input);

        let inner_signature = signer.sign_hash(&digest).await?;
        let mut wrapped =
            String::with_capacity(2 + 130 + 64 + 64 + (ORDER_TYPE_STRING.len() * 2) + 4);
        wrapped.push_str("0x");
        wrapped.push_str(&signature_hex_no_prefix(&inner_signature));
        push_hex(&mut wrapped, app_domain_separator.as_slice());
        push_hex(&mut wrapped, contents_hash.as_slice());
        push_hex(&mut wrapped, ORDER_TYPE_STRING.as_bytes());
        let contents_type_len =
            u16::try_from(ORDER_TYPE_STRING.len()).expect("order type string length fits in u16");
        push_hex(&mut wrapped, &contents_type_len.to_be_bytes());

        Ok(OrderSignature::Wrapped(wrapped))
    }

    pub async fn post_order(&self, order: SignedOrder) -> Result<PostOrderResponse> {
        let request = self
            .client()
            .request(Method::POST, format!("{}order", self.host()))
            .json(&order)
            .build()?;
        let headers = self.create_headers(&request).await?;

        let result = crate::request(&self.inner.client, request, Some(headers)).await;
        self.invalidate_version_if_mismatch(&result).await;
        result
    }

    pub async fn post_orders(&self, orders: Vec<SignedOrder>) -> Result<Vec<PostOrderResponse>> {
        let request = self
            .client()
            .request(Method::POST, format!("{}orders", self.host()))
            .json(&orders)
            .build()?;
        let headers = self.create_headers(&request).await?;

        let result = crate::request(&self.inner.client, request, Some(headers)).await;
        self.invalidate_version_if_mismatch(&result).await;
        result
    }

    async fn invalidate_version_if_mismatch<T>(&self, result: &Result<T>) {
        let Err(err) = result else { return };
        let Some(status) = err.downcast_ref::<crate::error::Status>() else {
            return;
        };
        if status.message.contains(ORDER_VERSION_MISMATCH_ERROR) {
            let _: Result<u32> = self.resolve_version(true).await;
        }
    }

    pub async fn order(&self, order_id: &str) -> Result<OpenOrderResponse> {
        let request = self
            .client()
            .request(Method::GET, format!("{}data/order/{order_id}", self.host()))
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn orders(
        &self,
        request: &OrdersRequest,
        next_cursor: Option<String>,
    ) -> Result<Page<OpenOrderResponse>> {
        let params = request.query_params(next_cursor.as_deref());
        let request = self
            .client()
            .request(Method::GET, format!("{}data/orders{params}", self.host()))
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn cancel_order(&self, order_id: &str) -> Result<CancelOrdersResponse> {
        let request = self
            .client()
            .request(Method::DELETE, format!("{}order", self.host()))
            .json(&json!({ "orderID": order_id }))
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn cancel_orders(&self, order_ids: &[&str]) -> Result<CancelOrdersResponse> {
        let request = self
            .client()
            .request(Method::DELETE, format!("{}orders", self.host()))
            .json(&json!(order_ids))
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn cancel_all_orders(&self) -> Result<CancelOrdersResponse> {
        let request = self
            .client()
            .request(Method::DELETE, format!("{}cancel-all", self.host()))
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn cancel_market_orders(
        &self,
        request: &CancelMarketOrderRequest,
    ) -> Result<CancelOrdersResponse> {
        let request = self
            .client()
            .request(
                Method::DELETE,
                format!("{}cancel-market-orders", self.host()),
            )
            .json(&request)
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn trades(
        &self,
        request: &TradesRequest,
        next_cursor: Option<String>,
    ) -> Result<Page<TradeResponse>> {
        let params = request.query_params(next_cursor.as_deref());
        let request = self
            .client()
            .request(Method::GET, format!("{}data/trades{params}", self.host()))
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn notifications(&self) -> Result<Vec<NotificationResponse>> {
        let request = self
            .client()
            .request(Method::GET, format!("{}notifications", self.host()))
            .query(&[("signature_type", self.inner.signature_type as u8)])
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn delete_notifications(&self, request: &DeleteNotificationsRequest) -> Result<()> {
        let params = request.query_params(None);
        let mut request = self
            .client()
            .request(
                Method::DELETE,
                format!("{}notifications{params}", self.host()),
            )
            .build()?;
        let headers = self.create_headers(&request).await?;
        *request.headers_mut() = headers;

        self.client().execute(request).await?;

        Ok(())
    }

    pub async fn balance_allowance(
        &self,
        mut request: BalanceAllowanceRequest,
    ) -> Result<BalanceAllowanceResponse> {
        if request.signature_type.is_none() {
            request.signature_type = Some(self.inner.signature_type);
        }

        let params = request.query_params(None);
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}balance-allowance{params}", self.host()),
            )
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn update_balance_allowance(
        &self,
        mut request: UpdateBalanceAllowanceRequest,
    ) -> Result<()> {
        if request.signature_type.is_none() {
            request.signature_type = Some(self.inner.signature_type);
        }

        let params = request.query_params(None);
        let mut request = self
            .client()
            .request(
                Method::GET,
                format!("{}balance-allowance/update{params}", self.host()),
            )
            .build()?;
        let headers = self.create_headers(&request).await?;

        *request.headers_mut() = headers;

        self.client().execute(request).await?;

        Ok(())
    }

    pub async fn is_order_scoring(&self, order_id: &str) -> Result<OrderScoringResponse> {
        let request = self
            .client()
            .request(Method::GET, format!("{}order-scoring", self.host()))
            .query(&[("order_id", order_id)])
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn are_orders_scoring(&self, order_ids: &[&str]) -> Result<OrdersScoringResponse> {
        let request = self
            .client()
            .request(Method::POST, format!("{}orders-scoring", self.host()))
            .json(&order_ids)
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn earnings_for_user_for_day(
        &self,
        date: NaiveDate,
        next_cursor: Option<String>,
    ) -> Result<Page<UserEarningResponse>> {
        let cursor = next_cursor.map_or(String::new(), |c| format!("?next_cursor={c}"));
        let request = self
            .client()
            .request(Method::GET, format!("{}rewards/user{cursor}", self.host()))
            .query(&[
                ("date", date.to_string()),
                (
                    "signature_type",
                    (self.inner.signature_type as u8).to_string(),
                ),
            ])
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn total_earnings_for_user_for_day(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<TotalUserEarningResponse>> {
        let request = self
            .client()
            .request(Method::GET, format!("{}rewards/user/total", self.host()))
            .query(&[
                ("date", date.to_string()),
                (
                    "signature_type",
                    (self.inner.signature_type as u8).to_string(),
                ),
            ])
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn user_earnings_and_markets_config(
        &self,
        request: &UserRewardsEarningRequest,
        next_cursor: Option<String>,
    ) -> Result<Page<UserRewardsEarningResponse>> {
        let params = request.query_params(next_cursor.as_deref());
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}rewards/user/markets{params}", self.host()),
            )
            .query(&[(
                "signature_type",
                (self.inner.signature_type as u8).to_string(),
            )])
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn reward_percentages(&self) -> Result<RewardsPercentagesResponse> {
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}rewards/user/percentages", self.host()),
            )
            .query(&[(
                "signature_type",
                (self.inner.signature_type as u8).to_string(),
            )])
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn current_rewards(
        &self,
        next_cursor: Option<String>,
    ) -> Result<Page<CurrentRewardResponse>> {
        let cursor = next_cursor.map_or(String::new(), |c| format!("?next_cursor={c}"));
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}rewards/markets/current{cursor}", self.host()),
            )
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn raw_rewards_for_market(
        &self,
        condition_id: &str,
        next_cursor: Option<String>,
    ) -> Result<Page<MarketRewardResponse>> {
        let cursor = next_cursor.map_or(String::new(), |c| format!("?next_cursor={c}"));
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}rewards/markets/{condition_id}{cursor}", self.host()),
            )
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn create_readonly_api_key(&self) -> Result<ReadonlyApiKeyResponse> {
        let request = self
            .client()
            .request(
                Method::POST,
                format!("{}auth/readonly-api-key", self.host()),
            )
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn readonly_api_keys(&self) -> Result<Vec<String>> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ReadonlyApiKeysBody {
            readonly_api_keys: Vec<String>,
        }

        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}auth/readonly-api-keys", self.host()),
            )
            .build()?;
        let headers = self.create_headers(&request).await?;

        let body: ReadonlyApiKeysBody =
            crate::request(&self.inner.client, request, Some(headers)).await?;
        Ok(body.readonly_api_keys)
    }

    pub async fn delete_readonly_api_key(&self, key: &str) -> Result<()> {
        let mut request = self
            .client()
            .request(
                Method::DELETE,
                format!("{}auth/readonly-api-key", self.host()),
            )
            .json(&serde_json::json!({ "key": key }))
            .build()?;
        let method = request.method().clone();
        let path = request.url().path().to_owned();
        let headers = self.create_headers(&request).await?;

        request.headers_mut().extend(headers);
        let response = self.inner.client.execute(request).await?;
        let status = response.status();

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(Error::status(status, method, path, message));
        }

        Ok(())
    }

    pub async fn pre_migration_orders(
        &self,
        next_cursor: Option<String>,
    ) -> Result<Page<OpenOrderResponse>> {
        let cursor = next_cursor
            .map(|c| format!("?next_cursor={c}"))
            .unwrap_or_default();

        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}data/pre-migration-orders{cursor}", self.host()),
            )
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn builder_fee_rate(&self, builder_code: B256) -> Result<BuilderFeeRateResponse> {
        if let Some(cached) = self.inner.builder_fee_rates.get(&builder_code) {
            return Ok(cached.clone());
        }

        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}fees/builder-fees/{builder_code}", self.host()),
            )
            .build()?;
        let headers = self.create_headers(&request).await?;

        let response: BuilderFeeRateResponse =
            crate::request(&self.inner.client, request, Some(headers)).await?;

        self.inner
            .builder_fee_rates
            .insert(builder_code, response.clone());

        Ok(response)
    }

    pub async fn create_builder_api_key(&self) -> Result<Credentials> {
        let request = self
            .client()
            .request(Method::POST, format!("{}auth/builder-api-key", self.host()))
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn builder_api_keys(&self) -> Result<Vec<BuilderApiKeyResponse>> {
        let request = self
            .client()
            .request(Method::GET, format!("{}auth/builder-api-key", self.host()))
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn revoke_builder_api_key(&self) -> Result<()> {
        let mut request = self
            .client()
            .request(
                Method::DELETE,
                format!("{}auth/builder-api-key", self.host()),
            )
            .build()?;
        let headers = self.create_headers(&request).await?;

        *request.headers_mut() = headers;

        self.client().execute(request).await?;

        Ok(())
    }

    pub async fn builder_trades(
        &self,
        builder_code: B256,
        request: &TradesRequest,
        next_cursor: Option<String>,
    ) -> Result<Page<BuilderTradeResponse>> {
        if builder_code == B256::ZERO {
            return Err(Error::validation(
                "builder_code is required and cannot be zero",
            ));
        }
        let params = request.query_params(next_cursor.as_deref());
        let sep = if params.is_empty() { '?' } else { '&' };
        let url = format!(
            "{}builder/trades{params}{sep}builder_code={builder_code}",
            self.host()
        );

        let request = self.client().request(Method::GET, url).build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    pub async fn post_heartbeat(&self, heartbeat_id: Option<Uuid>) -> Result<HeartbeatResponse> {
        let request = self
            .client()
            .request(Method::POST, format!("{}v1/heartbeats", self.host()))
            .json(&json!({ "heartbeat_id": heartbeat_id }))
            .build()?;
        let headers = self.create_headers(&request).await?;

        crate::request(&self.inner.client, request, Some(headers)).await
    }

    #[cfg(feature = "heartbeats")]
    
    #[must_use]
    pub fn heartbeats_active(&self) -> bool {
        self.heartbeat_token.0.is_some()
    }

    #[cfg(feature = "heartbeats")]
    
    pub fn start_heartbeats(client: &mut Client<Authenticated<K>>) -> Result<()> {
        if client.heartbeats_active() {
            return Err(Error::validation("Unable to create another heartbeat task"));
        }

        let token = CancellationToken::new();
        let duration = client.inner.config.heartbeat_interval;
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        let token_clone = token.clone();
        let client_clone = client.clone();

        tokio::task::spawn(async move {
            let mut heartbeat_id: Option<Uuid> = None;

            let mut ticker = time::interval(duration);
            ticker.tick().await;

            loop {
                tokio::select! {
                    () = token_clone.cancelled() => {
                        #[cfg(feature = "tracing")]
                        debug!("Heartbeat cancellation requested, terminating...");
                        break
                    },
                    _ = ticker.tick() => {
                        match client_clone.post_heartbeat(heartbeat_id).await {
                            Ok(response) => {
                                #[cfg(feature = "tracing")]
                                debug!("Heartbeat successfully sent: {response:?}");
                                heartbeat_id = Some(response.heartbeat_id);
                            },
                            Err(e) => {
                                #[cfg(feature = "tracing")]
                                error!("Unable to post heartbeat: {e:?}");
                                #[cfg(not(feature = "tracing"))]
                                let _ = &e;
                            }
                        }
                    }
                }
            }

            tx.send(())
        });

        client.heartbeat_token = DroppingCancellationToken(Some((token, Arc::new(rx))));

        Ok(())
    }

    #[cfg(feature = "heartbeats")]
    
    pub async fn stop_heartbeats(&mut self) -> Result<()> {
        self.heartbeat_token.cancel_and_wait().await
    }

    async fn create_headers(&self, request: &Request) -> Result<HeaderMap> {
        let timestamp = if self.inner.config.use_server_time {
            self.server_time().await?
        } else {
            Utc::now().timestamp()
        };

        auth::l2::create_headers(self.state(), request, timestamp).await
    }

    #[cfg(feature = "rfq")]
    pub async fn create_request(
        &self,
        request: &CreateRfqRequestRequest,
    ) -> Result<CreateRfqRequestResponse> {
        let http_request = self
            .client()
            .request(Method::POST, format!("{}rfq/request", self.host()))
            .json(request)
            .build()?;
        let headers = self.create_headers(&http_request).await?;

        crate::request(&self.inner.client, http_request, Some(headers)).await
    }

    #[cfg(feature = "rfq")]
    pub async fn cancel_request(&self, request: &CancelRfqRequestRequest) -> Result<()> {
        let http_request = self
            .client()
            .request(Method::DELETE, format!("{}rfq/request", self.host()))
            .json(request)
            .build()?;
        let headers = self.create_headers(&http_request).await?;

        self.rfq_request_text(http_request, headers).await
    }

    #[cfg(feature = "rfq")]
    pub async fn requests(
        &self,
        request: &RfqRequestsRequest,
        next_cursor: Option<&str>,
    ) -> Result<Page<RfqRequest>> {
        let params = request.query_params(next_cursor);
        let http_request = self
            .client()
            .request(
                Method::GET,
                format!("{}rfq/data/requests{params}", self.host()),
            )
            .build()?;
        let headers = self.create_headers(&http_request).await?;

        crate::request(&self.inner.client, http_request, Some(headers)).await
    }

    #[cfg(feature = "rfq")]
    pub async fn create_quote(
        &self,
        request: &CreateRfqQuoteRequest,
    ) -> Result<CreateRfqQuoteResponse> {
        let http_request = self
            .client()
            .request(Method::POST, format!("{}rfq/quote", self.host()))
            .json(request)
            .build()?;
        let headers = self.create_headers(&http_request).await?;

        crate::request(&self.inner.client, http_request, Some(headers)).await
    }

    #[cfg(feature = "rfq")]
    pub async fn cancel_quote(&self, request: &CancelRfqQuoteRequest) -> Result<()> {
        let http_request = self
            .client()
            .request(Method::DELETE, format!("{}rfq/quote", self.host()))
            .json(request)
            .build()?;
        let headers = self.create_headers(&http_request).await?;

        self.rfq_request_text(http_request, headers).await
    }

    #[cfg(feature = "rfq")]
    pub async fn quotes(
        &self,
        request: &RfqQuotesRequest,
        next_cursor: Option<&str>,
    ) -> Result<Page<RfqQuote>> {
        let params = request.query_params(next_cursor);
        let http_request = self
            .client()
            .request(
                Method::GET,
                format!("{}rfq/data/quotes{params}", self.host()),
            )
            .build()?;
        let headers = self.create_headers(&http_request).await?;

        crate::request(&self.inner.client, http_request, Some(headers)).await
    }

    #[cfg(feature = "rfq")]
    pub async fn accept_quote(
        &self,
        request: &AcceptRfqQuoteRequest,
    ) -> Result<AcceptRfqQuoteResponse> {
        let http_request = self
            .client()
            .request(Method::POST, format!("{}rfq/request/accept", self.host()))
            .json(request)
            .build()?;
        let headers = self.create_headers(&http_request).await?;

        self.rfq_request_text(http_request, headers).await?;
        Ok(AcceptRfqQuoteResponse)
    }

    #[cfg(feature = "rfq")]
    pub async fn approve_order(
        &self,
        request: &ApproveRfqOrderRequest,
    ) -> Result<ApproveRfqOrderResponse> {
        let http_request = self
            .client()
            .request(Method::POST, format!("{}rfq/quote/approve", self.host()))
            .json(request)
            .build()?;
        let headers = self.create_headers(&http_request).await?;

        crate::request(&self.inner.client, http_request, Some(headers)).await
    }

    #[cfg(feature = "rfq")]
    async fn rfq_request_text(&self, mut request: Request, headers: HeaderMap) -> Result<()> {
        let method = request.method().clone();
        let path = request.url().path().to_owned();

        *request.headers_mut() = headers;

        let response = self.inner.client.execute(request).await?;
        let status = response.status();

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(crate::error::Error::status(status, method, path, message));
        }

        Ok(())
    }

    fn order_builder<OrderKind>(&self) -> OrderBuilder<OrderKind, K> {
        OrderBuilder {
            signer: self.address(),
            signature_type: self.inner.signature_type,
            funder: self.inner.funder,
            salt_generator: self.inner.salt_generator,
            token_id: None,
            price: None,
            size: None,
            amount: None,
            side: None,
            expiration: None,
            order_type: None,
            post_only: Some(false),
            metadata: None,
            builder_code: self.inner.config.builder_code,
            defer_exec: None,
            user_usdc_balance: None,
            taker: None,
            nonce: None,
            fee_rate_bps: None,
            client: Client {
                inner: Arc::clone(&self.inner),
                #[cfg(feature = "heartbeats")]
                heartbeat_token: self.heartbeat_token.clone(),
            },
            _kind: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_default_should_succeed() {
        _ = Client::default();
    }
}
