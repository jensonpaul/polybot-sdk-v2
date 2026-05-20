
use std::future::Future;

use async_stream::try_stream;
use futures::Stream;
use reqwest::{
    Client as ReqwestClient, Method,
    header::{HeaderMap, HeaderValue},
};
use serde::Serialize;
use serde::de::DeserializeOwned;
#[cfg(feature = "tracing")]
use tracing::warn;
use url::Url;

use super::types::request::{
    CommentsByIdRequest, CommentsByUserAddressRequest, CommentsRequest, EventByIdRequest,
    EventBySlugRequest, EventTagsRequest, EventsRequest, MarketByIdRequest, MarketBySlugRequest,
    MarketTagsRequest, MarketsRequest, PublicProfileRequest, RelatedTagsByIdRequest,
    RelatedTagsBySlugRequest, SearchRequest, SeriesByIdRequest, SeriesListRequest, TagByIdRequest,
    TagBySlugRequest, TagsRequest, TeamsRequest,
};
use super::types::response::{
    Comment, Event, HealthResponse, Market, PublicProfile, RelatedTag, SearchResults, Series,
    SportsMarketTypesResponse, SportsMetadata, Tag, Team,
};
use crate::error::Error;
use crate::{Result, ToQueryParams as _};

const MAX_LIMIT: i32 = 500;

#[derive(Clone, Debug)]
pub struct Client {
    host: Url,
    client: ReqwestClient,
}

impl Default for Client {
    fn default() -> Self {
        Client::new("https://gamma-api.polymarket.com")
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

    async fn get<Req: Serialize, Res: DeserializeOwned + Serialize>(
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

    pub async fn status(&self) -> Result<HealthResponse> {
        let request = self
            .client
            .request(Method::GET, format!("{}status", self.host))
            .build()?;

        let response = self.client.execute(request).await?;
        let status_code = response.status();

        if !status_code.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(Error::status(
                status_code,
                Method::GET,
                "status".to_owned(),
                message,
            ));
        }

        Ok(response.text().await?)
    }

    pub async fn teams(&self, request: &TeamsRequest) -> Result<Vec<Team>> {
        self.get("teams", request).await
    }

    pub async fn sports(&self) -> Result<Vec<SportsMetadata>> {
        self.get("sports", &()).await
    }

    pub async fn sports_market_types(&self) -> Result<SportsMarketTypesResponse> {
        self.get("sports/market-types", &()).await
    }

    pub async fn tags(&self, request: &TagsRequest) -> Result<Vec<Tag>> {
        self.get("tags", request).await
    }

    pub async fn tag_by_id(&self, request: &TagByIdRequest) -> Result<Tag> {
        self.get(&format!("tags/{}", request.id), request).await
    }

    pub async fn tag_by_slug(&self, request: &TagBySlugRequest) -> Result<Tag> {
        self.get(&format!("tags/slug/{}", request.slug), request)
            .await
    }

    pub async fn related_tags_by_id(
        &self,
        request: &RelatedTagsByIdRequest,
    ) -> Result<Vec<RelatedTag>> {
        self.get(&format!("tags/{}/related-tags", request.id), request)
            .await
    }

    pub async fn related_tags_by_slug(
        &self,
        request: &RelatedTagsBySlugRequest,
    ) -> Result<Vec<RelatedTag>> {
        self.get(&format!("tags/slug/{}/related-tags", request.slug), request)
            .await
    }

    pub async fn tags_related_to_tag_by_id(
        &self,
        request: &RelatedTagsByIdRequest,
    ) -> Result<Vec<Tag>> {
        self.get(&format!("tags/{}/related-tags/tags", request.id), request)
            .await
    }

    pub async fn tags_related_to_tag_by_slug(
        &self,
        request: &RelatedTagsBySlugRequest,
    ) -> Result<Vec<Tag>> {
        self.get(
            &format!("tags/slug/{}/related-tags/tags", request.slug),
            request,
        )
        .await
    }

    pub async fn events(&self, request: &EventsRequest) -> Result<Vec<Event>> {
        self.get("events", request).await
    }

    pub async fn event_by_id(&self, request: &EventByIdRequest) -> Result<Event> {
        self.get(&format!("events/{}", request.id), request).await
    }

    pub async fn event_by_slug(&self, request: &EventBySlugRequest) -> Result<Event> {
        self.get(&format!("events/slug/{}", request.slug), request)
            .await
    }

    pub async fn event_tags(&self, request: &EventTagsRequest) -> Result<Vec<Tag>> {
        self.get(&format!("events/{}/tags", request.id), request)
            .await
    }

    pub async fn markets(&self, request: &MarketsRequest) -> Result<Vec<Market>> {
        self.get("markets", request).await
    }

    pub async fn market_by_id(&self, request: &MarketByIdRequest) -> Result<Market> {
        self.get(&format!("markets/{}", request.id), request).await
    }

    pub async fn market_by_slug(&self, request: &MarketBySlugRequest) -> Result<Market> {
        self.get(&format!("markets/slug/{}", request.slug), request)
            .await
    }

    pub async fn market_tags(&self, request: &MarketTagsRequest) -> Result<Vec<Tag>> {
        self.get(&format!("markets/{}/tags", request.id), request)
            .await
    }

    pub async fn series(&self, request: &SeriesListRequest) -> Result<Vec<Series>> {
        self.get("series", request).await
    }

    pub async fn series_by_id(&self, request: &SeriesByIdRequest) -> Result<Series> {
        self.get(&format!("series/{}", request.id), request).await
    }

    pub async fn comments(&self, request: &CommentsRequest) -> Result<Vec<Comment>> {
        self.get("comments", request).await
    }

    pub async fn comments_by_id(&self, request: &CommentsByIdRequest) -> Result<Vec<Comment>> {
        self.get(&format!("comments/{}", request.id), request).await
    }

    pub async fn comments_by_user_address(
        &self,
        request: &CommentsByUserAddressRequest,
    ) -> Result<Vec<Comment>> {
        self.get(
            &format!("comments/user_address/{}", request.user_address),
            request,
        )
        .await
    }

    pub async fn public_profile(&self, request: &PublicProfileRequest) -> Result<PublicProfile> {
        self.get("public-profile", request).await
    }

    pub async fn search(&self, request: &SearchRequest) -> Result<SearchResults> {
        self.get("public-search", request).await
    }

    pub fn stream_data<'client, Call, Fut, Data>(
        &'client self,
        call: Call,
        limit: i32,
    ) -> impl Stream<Item = Result<Data>> + 'client
    where
        Call: Fn(&'client Client, i32, i32) -> Fut + 'client,
        Fut: Future<Output = Result<Vec<Data>>> + 'client,
        Data: 'client,
    {
        let limit = if limit > MAX_LIMIT {
            #[cfg(feature = "tracing")]
            warn!(
                "Supplied {limit} limit, Gamma only allows for maximum {MAX_LIMIT} responses per call, defaulting to {MAX_LIMIT}"
            );

            MAX_LIMIT
        } else {
            limit
        };

        try_stream! {
            let mut offset = 0;

            loop {
                let data = call(self, limit, offset).await?;

                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_possible_wrap,
                    reason = "We shouldn't ever truncate/wrap since we'll never return that many records in one call")
                ]
                let count = data.len() as i32;

                for item in data {
                    yield item;
                }

                // Stop if we received fewer items than requested (last page)
                if count < limit {
                    break;
                }

                offset += count;
            }
        }
    }
}
