
use reqwest::{
    Client as ReqwestClient, Method,
    header::{HeaderMap, HeaderValue},
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use url::Url;

use super::types::request::{
    ActivityRequest, BuilderLeaderboardRequest, BuilderVolumeRequest, ClosedPositionsRequest,
    HoldersRequest, LiveVolumeRequest, OpenInterestRequest, PositionsRequest, TradedRequest,
    TraderLeaderboardRequest, TradesRequest, ValueRequest,
};
use super::types::response::{
    Activity, BuilderLeaderboardEntry, BuilderVolumeEntry, ClosedPosition, Health, LiveVolume,
    MetaHolder, OpenInterest, Position, Trade, Traded, TraderLeaderboardEntry, Value,
};
use crate::{Result, ToQueryParams as _};

#[derive(Clone, Debug)]
pub struct Client {
    host: Url,
    client: ReqwestClient,
}

impl Default for Client {
    fn default() -> Self {
        Client::new("https://data-api.polymarket.com")
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

    async fn get<Req: Serialize, Res: DeserializeOwned>(
        &self,
        path: &str,
        req: &Req,
    ) -> Result<Res> {
        let query = req.query_params(None);
        let request = self
            .client
            .request(Method::GET, format!("{}{path}{query}", self.host))
            .build()?;
        crate::request(&self.client, request, None).await
    }

    pub async fn health(&self) -> Result<Health> {
        self.get("", &()).await
    }

    pub async fn positions(&self, req: &PositionsRequest) -> Result<Vec<Position>> {
        self.get("positions", req).await
    }

    pub async fn trades(&self, req: &TradesRequest) -> Result<Vec<Trade>> {
        self.get("trades", req).await
    }

    pub async fn activity(&self, req: &ActivityRequest) -> Result<Vec<Activity>> {
        self.get("activity", req).await
    }

    pub async fn holders(&self, req: &HoldersRequest) -> Result<Vec<MetaHolder>> {
        self.get("holders", req).await
    }

    pub async fn value(&self, req: &ValueRequest) -> Result<Vec<Value>> {
        self.get("value", req).await
    }

    pub async fn closed_positions(
        &self,
        req: &ClosedPositionsRequest,
    ) -> Result<Vec<ClosedPosition>> {
        self.get("closed-positions", req).await
    }

    pub async fn leaderboard(
        &self,
        req: &TraderLeaderboardRequest,
    ) -> Result<Vec<TraderLeaderboardEntry>> {
        self.get("v1/leaderboard", req).await
    }

    pub async fn traded(&self, req: &TradedRequest) -> Result<Traded> {
        self.get("traded", req).await
    }

    pub async fn open_interest(&self, req: &OpenInterestRequest) -> Result<Vec<OpenInterest>> {
        self.get("oi", req).await
    }

    pub async fn live_volume(&self, req: &LiveVolumeRequest) -> Result<Vec<LiveVolume>> {
        self.get("live-volume", req).await
    }

    pub async fn builder_leaderboard(
        &self,
        req: &BuilderLeaderboardRequest,
    ) -> Result<Vec<BuilderLeaderboardEntry>> {
        self.get("v1/builders/leaderboard", req).await
    }

    pub async fn builder_volume(
        &self,
        req: &BuilderVolumeRequest,
    ) -> Result<Vec<BuilderVolumeEntry>> {
        self.get("v1/builders/volume", req).await
    }
}
