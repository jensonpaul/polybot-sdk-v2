use std::sync::Arc;

use async_stream::try_stream;
use dashmap::mapref::one::{Ref, RefMut};
use dashmap::{DashMap, Entry};
use futures::Stream;
use futures::StreamExt as _;

use super::interest::InterestTracker;
use super::subscription::{ChannelType, SubscriptionManager};
use super::types::response::{
    BestBidAsk, BookUpdate, LastTradePrice, MarketResolved, MidpointUpdate, NewMarket,
    OrderMessage, PriceChange, TickSizeChange, TradeMessage, WsMessage,
};
use crate::Result;
use crate::auth::state::{Authenticated, State, Unauthenticated};
use crate::auth::{Credentials, Kind as AuthKind, Normal};
use crate::error::Error;
use crate::types::{Address, B256, Decimal, U256};
use crate::ws::ConnectionManager;
use crate::ws::config::Config;
use crate::ws::connection::ConnectionState;

#[derive(Clone)]
pub struct Client<S: State = Unauthenticated> {
    inner: Arc<ClientInner<S>>,
}

impl Default for Client<Unauthenticated> {
    fn default() -> Self {
        Self::new(
            "wss://ws-subscriptions-clob.polymarket.com",
            Config::default(),
        )
        .expect("WebSocket client with default endpoint should succeed")
    }
}

struct ClientInner<S: State> {
    
    state: S,
    
    config: Config,
    
    base_endpoint: String,
    
    channels: DashMap<ChannelType, ChannelResources>,
}

impl Client<Unauthenticated> {
    
    pub fn new(endpoint: &str, config: Config) -> Result<Self> {
        let base_endpoint = normalize_base_endpoint(endpoint);

        Ok(Self {
            inner: Arc::new(ClientInner {
                state: Unauthenticated,
                config,
                base_endpoint,
                channels: DashMap::new(),
            }),
        })
    }

    pub fn authenticate(
        self,
        credentials: Credentials,
        address: Address,
    ) -> Result<Client<Authenticated<Normal>>> {
        let inner = Arc::into_inner(self.inner).ok_or(Error::validation(
            "Cannot authenticate while other references to this client exist; \
                 drop all clones before calling authenticate",
        ))?;
        let ClientInner {
            config,
            base_endpoint,
            channels,
            ..
        } = inner;

        Ok(Client {
            inner: Arc::new(ClientInner {
                state: Authenticated {
                    address,
                    credentials,
                    kind: Normal,
                },
                config,
                base_endpoint,
                channels,
            }),
        })
    }
}

impl<S: State> Client<S> {
    
    pub fn subscribe_orderbook(
        &self,
        asset_ids: Vec<U256>,
    ) -> Result<impl Stream<Item = Result<BookUpdate>> + use<S>> {
        let resources = self.inner.get_or_create_channel(ChannelType::Market)?;
        let stream = resources.subscriptions.subscribe_market(asset_ids)?;

        Ok(stream.filter_map(|msg_result| async move {
            match msg_result {
                Ok(WsMessage::Book(book)) => Some(Ok(book)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        }))
    }

    pub fn subscribe_last_trade_price(
        &self,
        asset_ids: Vec<U256>,
    ) -> Result<impl Stream<Item = Result<LastTradePrice>> + use<S>> {
        let resources = self.inner.get_or_create_channel(ChannelType::Market)?;
        let stream = resources.subscriptions.subscribe_market(asset_ids)?;

        Ok(stream.filter_map(|msg_result| async move {
            match msg_result {
                Ok(WsMessage::LastTradePrice(last_trade_price)) => Some(Ok(last_trade_price)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        }))
    }

    pub fn subscribe_prices(
        &self,
        asset_ids: Vec<U256>,
    ) -> Result<impl Stream<Item = Result<PriceChange>> + use<S>> {
        let resources = self.inner.get_or_create_channel(ChannelType::Market)?;
        let stream = resources.subscriptions.subscribe_market(asset_ids)?;

        Ok(stream.filter_map(|msg_result| async move {
            match msg_result {
                Ok(WsMessage::PriceChange(price)) => Some(Ok(price)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        }))
    }

    pub fn subscribe_tick_size_change(
        &self,
        asset_ids: Vec<U256>,
    ) -> Result<impl Stream<Item = Result<TickSizeChange>> + use<S>> {
        let resources = self.inner.get_or_create_channel(ChannelType::Market)?;
        let stream = resources.subscriptions.subscribe_market(asset_ids)?;

        Ok(stream.filter_map(|msg_result| async move {
            match msg_result {
                Ok(WsMessage::TickSizeChange(tsc)) => Some(Ok(tsc)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        }))
    }

    pub fn subscribe_midpoints(
        &self,
        asset_ids: Vec<U256>,
    ) -> Result<impl Stream<Item = Result<MidpointUpdate>> + use<S>> {
        let stream = self.subscribe_orderbook(asset_ids)?;

        Ok(try_stream! {
            for await book_result in stream {
                let book = book_result?;

                if let (Some(bid), Some(ask)) = (book.bids.first(), book.asks.first()) {
                    let midpoint = (bid.price + ask.price) / Decimal::TWO;
                    yield MidpointUpdate {
                        asset_id: book.asset_id,
                        market: book.market,
                        midpoint,
                        timestamp: book.timestamp,
                    };
                }
            }
        })
    }

    pub fn subscribe_best_bid_ask(
        &self,
        asset_ids: Vec<U256>,
    ) -> Result<impl Stream<Item = Result<BestBidAsk>> + use<S>> {
        let stream = self
            .inner
            .get_or_create_channel(ChannelType::Market)?
            .subscriptions
            .subscribe_market_with_options(asset_ids, true)?;

        Ok(stream.filter_map(|msg_result| async move {
            match msg_result {
                Ok(WsMessage::BestBidAsk(bba)) => Some(Ok(bba)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        }))
    }

    pub fn subscribe_new_markets(
        &self,
        asset_ids: Vec<U256>,
    ) -> Result<impl Stream<Item = Result<NewMarket>> + use<S>> {
        let stream = self
            .inner
            .get_or_create_channel(ChannelType::Market)?
            .subscriptions
            .subscribe_market_with_options(asset_ids, true)?;

        Ok(stream.filter_map(|msg_result| async move {
            match msg_result {
                Ok(WsMessage::NewMarket(nm)) => Some(Ok(nm)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        }))
    }

    pub fn subscribe_market_resolutions(
        &self,
        asset_ids: Vec<U256>,
    ) -> Result<impl Stream<Item = Result<MarketResolved>> + use<S>> {
        let stream = self
            .inner
            .get_or_create_channel(ChannelType::Market)?
            .subscriptions
            .subscribe_market_with_options(asset_ids, true)?;

        Ok(stream.filter_map(|msg_result| async move {
            match msg_result {
                Ok(WsMessage::MarketResolved(mr)) => Some(Ok(mr)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        }))
    }

    #[must_use]
    pub fn connection_state(&self, channel_type: ChannelType) -> ConnectionState {
        self.inner.channel(channel_type).as_deref().map_or(
            ConnectionState::Disconnected,
            ChannelResources::connection_state,
        )
    }

    #[must_use]
    pub fn is_connected(&self, channel_type: ChannelType) -> bool {
        self.inner.channel(channel_type).is_some()
    }

    #[must_use]
    pub fn subscription_count(&self) -> usize {
        self.inner
            .channels
            .iter()
            .map(|entry| entry.value().subscriptions.subscription_count())
            .sum()
    }

    pub fn unsubscribe_orderbook(&self, asset_ids: &[U256]) -> Result<()> {
        self.inner
            .unsubscribe_and_cleanup(ChannelType::Market, |subs| {
                subs.unsubscribe_market(asset_ids)
            })
    }

    pub fn unsubscribe_prices(&self, asset_ids: &[U256]) -> Result<()> {
        self.unsubscribe_orderbook(asset_ids)
    }

    pub fn unsubscribe_tick_size_change(&self, asset_ids: &[U256]) -> Result<()> {
        self.unsubscribe_orderbook(asset_ids)
    }

    pub fn unsubscribe_midpoints(&self, asset_ids: &[U256]) -> Result<()> {
        self.unsubscribe_orderbook(asset_ids)
    }
}

impl<K: AuthKind> Client<Authenticated<K>> {
    
    pub fn subscribe_user_events(
        &self,
        markets: Vec<B256>,
    ) -> Result<impl Stream<Item = Result<WsMessage>> + use<K>> {
        let resources = self.inner.get_or_create_channel(ChannelType::User)?;

        resources
            .subscriptions
            .subscribe_user(markets, &self.inner.state.credentials)
    }

    pub fn subscribe_orders(
        &self,
        markets: Vec<B256>,
    ) -> Result<impl Stream<Item = Result<OrderMessage>> + use<K>> {
        let stream = self.subscribe_user_events(markets)?;

        Ok(stream.filter_map(|msg_result| async move {
            match msg_result {
                Ok(WsMessage::Order(order)) => Some(Ok(order)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        }))
    }

    pub fn subscribe_trades(
        &self,
        markets: Vec<B256>,
    ) -> Result<impl Stream<Item = Result<TradeMessage>> + use<K>> {
        let stream = self.subscribe_user_events(markets)?;

        Ok(stream.filter_map(|msg_result| async move {
            match msg_result {
                Ok(WsMessage::Trade(trade)) => Some(Ok(trade)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        }))
    }

    pub fn unsubscribe_user_events(&self, markets: &[B256]) -> Result<()> {
        self.inner
            .unsubscribe_and_cleanup(ChannelType::User, |subs| subs.unsubscribe_user(markets))
    }

    pub fn unsubscribe_orders(&self, markets: &[B256]) -> Result<()> {
        self.unsubscribe_user_events(markets)
    }

    pub fn unsubscribe_trades(&self, markets: &[B256]) -> Result<()> {
        self.unsubscribe_user_events(markets)
    }

    pub fn deauthenticate(self) -> Result<Client<Unauthenticated>> {
        let inner = Arc::into_inner(self.inner).ok_or(Error::validation(
            "Cannot deauthenticate while other references to this client exist; \
                 drop all clones before calling deauthenticate",
        ))?;
        let ClientInner {
            config,
            base_endpoint,
            channels,
            ..
        } = inner;
        channels.remove(&ChannelType::User);

        Ok(Client {
            inner: Arc::new(ClientInner {
                state: Unauthenticated,
                config,
                base_endpoint,
                channels,
            }),
        })
    }
}

impl<S: State> ClientInner<S> {
    fn get_or_create_channel(
        &self,
        channel_type: ChannelType,
    ) -> Result<Ref<'_, ChannelType, ChannelResources>> {
        self.channels
            .entry(channel_type)
            .or_try_insert_with(|| {
                let endpoint = channel_endpoint(&self.base_endpoint, channel_type);
                ChannelResources::new(endpoint, self.config.clone())
            })
            .map(RefMut::downgrade)
    }

    fn channel(&self, channel_type: ChannelType) -> Option<Ref<'_, ChannelType, ChannelResources>> {
        self.channels.get(&channel_type)
    }

    fn unsubscribe_and_cleanup<F>(&self, channel_type: ChannelType, unsubscribe_fn: F) -> Result<()>
    where
        F: FnOnce(&SubscriptionManager) -> Result<()>,
    {
        match self.channels.entry(channel_type) {
            Entry::Vacant(_) => Ok(()),
            Entry::Occupied(channel_ref) => {
                
                let subs = Arc::clone(&channel_ref.get().subscriptions);
                drop(channel_ref); 

                unsubscribe_fn(&subs)?;

                if let Entry::Occupied(entry) = self.channels.entry(channel_type)
                    && !entry.get().subscriptions.has_subscriptions(channel_type)
                {
                    entry.remove();
                }
                Ok(())
            }
        }
    }
}

struct ChannelResources {
    connection: ConnectionManager<WsMessage, Arc<InterestTracker>>,
    subscriptions: Arc<SubscriptionManager>,
}

impl ChannelResources {
    fn new(endpoint: String, config: Config) -> Result<Self> {
        let interest = Arc::new(InterestTracker::new());
        let connection = ConnectionManager::new(endpoint, config, Arc::clone(&interest))?;
        let subscriptions = Arc::new(SubscriptionManager::new(connection.clone(), interest));

        subscriptions.start_reconnection_handler();

        Ok(Self {
            connection,
            subscriptions,
        })
    }

    fn connection_state(&self) -> ConnectionState {
        self.connection.state()
    }
}

fn normalize_base_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim_end_matches('/');
    if let Some(stripped) = trimmed.strip_suffix("/ws/market") {
        stripped.to_owned()
    } else if let Some(stripped) = trimmed.strip_suffix("/ws/user") {
        stripped.to_owned()
    } else if let Some(stripped) = trimmed.strip_suffix("/ws") {
        stripped.to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn channel_endpoint(base: &str, channel: ChannelType) -> String {
    let trimmed = base.trim_end_matches('/');
    let segment = match channel {
        ChannelType::Market => "market",
        ChannelType::User => "user",
    };
    format!("{trimmed}/ws/{segment}")
}
