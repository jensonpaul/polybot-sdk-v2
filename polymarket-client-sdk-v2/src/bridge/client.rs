use reqwest::{
    Client as ReqwestClient, Method,
    header::{HeaderMap, HeaderValue},
};
use url::Url;

use super::types::{
    DepositRequest, DepositResponse, QuoteRequest, QuoteResponse, StatusRequest, StatusResponse,
    SupportedAssetsResponse, WithdrawRequest, WithdrawResponse,
};
use crate::Result;

#[derive(Clone, Debug)]
pub struct Client {
    host: Url,
    client: ReqwestClient,
}

impl Default for Client {
    fn default() -> Self {
        Client::new("https://bridge.polymarket.com")
            .expect("Client with default endpoint should succeed")
    }
}

impl Client {
    
    pub fn new(host: &str) -> Result<Client> {
        let mut headers = HeaderMap::new();

        headers.insert("User-Agent", HeaderValue::from_static("rs_clob_client"));
        headers.insert("Accept", HeaderValue::from_static("*/*"));
        headers.insert("Connection", HeaderValue::from_static("keep-alive"));
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        let client = ReqwestClient::builder().default_headers(headers).build()?;

        Ok(Self {
            host: Url::parse(host)?,
            client,
        })
    }

    #[must_use]
    pub fn host(&self) -> &Url {
        &self.host
    }

    #[must_use]
    fn client(&self) -> &ReqwestClient {
        &self.client
    }

    pub async fn deposit(&self, request: &DepositRequest) -> Result<DepositResponse> {
        let request = self
            .client()
            .request(Method::POST, format!("{}deposit", self.host()))
            .json(request)
            .build()?;

        crate::request(&self.client, request, None).await
    }

    pub async fn withdraw(&self, request: &WithdrawRequest) -> Result<WithdrawResponse> {
        let request = self
            .client()
            .request(Method::POST, format!("{}withdraw", self.host()))
            .json(request)
            .build()?;

        crate::request(&self.client, request, None).await
    }

    pub async fn supported_assets(&self) -> Result<SupportedAssetsResponse> {
        let request = self
            .client()
            .request(Method::GET, format!("{}supported-assets", self.host()))
            .build()?;

        crate::request(&self.client, request, None).await
    }

    pub async fn status(&self, request: &StatusRequest) -> Result<StatusResponse> {
        let request = self
            .client()
            .request(
                Method::GET,
                format!("{}status/{}", self.host(), request.address),
            )
            .build()?;

        crate::request(&self.client, request, None).await
    }

    pub async fn quote(&self, request: &QuoteRequest) -> Result<QuoteResponse> {
        let request = self
            .client()
            .request(Method::POST, format!("{}quote", self.host()))
            .json(request)
            .build()?;

        crate::request(&self.client, request, None).await
    }
}
