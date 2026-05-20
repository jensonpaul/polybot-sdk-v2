#![expect(
    clippy::module_name_repetitions,
    reason = "Subscription types deliberately include the module name for clarity"
)]

use std::sync::{Arc, PoisonError, RwLock};
use std::time::Instant;

use async_stream::try_stream;
use dashmap::{DashMap, Entry};
use futures::Stream;
use tokio::sync::broadcast::error::RecvError;

use super::error::RtdsError;
use super::types::request::{Subscription, SubscriptionRequest};
use super::types::response::{RtdsMessage, parse_messages};
use crate::Result;
use crate::auth::Credentials;
use crate::ws::ConnectionManager;
use crate::ws::connection::ConnectionState;

#[non_exhaustive]
#[derive(Clone)]
pub struct SimpleParser;

impl crate::ws::traits::MessageParser<RtdsMessage> for SimpleParser {
    fn parse(&self, bytes: &[u8]) -> Result<Vec<RtdsMessage>> {
        parse_messages(bytes)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TopicType {
    
    pub topic: String,
    
    pub msg_type: String,
}

impl TopicType {
    
    #[must_use]
    pub fn new(topic: String, msg_type: String) -> Self {
        Self { topic, msg_type }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct SubscriptionInfo {
    
    pub topic_type: TopicType,
    
    pub filters: Option<String>,
    
    pub clob_auth: Option<Credentials>,
    
    pub created_at: Instant,
}

pub struct SubscriptionManager {
    connection: ConnectionManager<RtdsMessage, SimpleParser>,
    active_subs: DashMap<String, SubscriptionInfo>,
    
    subscribed_topics: DashMap<TopicType, usize>,
    last_auth: RwLock<Option<Credentials>>,
}

impl SubscriptionManager {
    
    #[must_use]
    pub fn new(connection: ConnectionManager<RtdsMessage, SimpleParser>) -> Self {
        Self {
            connection,
            active_subs: DashMap::new(),
            subscribed_topics: DashMap::new(),
            last_auth: RwLock::new(None),
        }
    }

    pub fn start_reconnection_handler(self: &Arc<Self>) {
        let this = Arc::clone(self);

        tokio::spawn(async move {
            let mut state_rx = this.connection.state_receiver();
            let mut was_connected = state_rx.borrow().is_connected();

            loop {
                
                if state_rx.changed().await.is_err() {
                    
                    break;
                }

                let state = *state_rx.borrow_and_update();

                match state {
                    ConnectionState::Connected { .. } => {
                        if was_connected {
                            
                            #[cfg(feature = "tracing")]
                            tracing::debug!("RTDS reconnected, re-establishing subscriptions");
                            this.resubscribe_all();
                        }
                        was_connected = true;
                    }
                    ConnectionState::Disconnected => {
                        
                        break;
                    }
                    _ => {
                        
                    }
                }
            }
        });
    }

    fn resubscribe_all(&self) {
        
        let auth = self
            .last_auth
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();

        let subscriptions: Vec<Subscription> = self
            .active_subs
            .iter()
            .map(|entry| {
                let info = entry.value();
                let mut sub = Subscription {
                    topic: info.topic_type.topic.clone(),
                    msg_type: info.topic_type.msg_type.clone(),
                    filters: info.filters.clone(),
                    clob_auth: None,
                };
                
                if info.clob_auth.is_some()
                    && let Some(creds) = &auth
                {
                    sub = sub.with_clob_auth(creds.clone());
                }
                sub
            })
            .collect();

        if subscriptions.is_empty() {
            return;
        }

        #[cfg(feature = "tracing")]
        tracing::debug!(count = subscriptions.len(), "Re-subscribing to RTDS topics");

        let request = SubscriptionRequest::subscribe(subscriptions);
        if let Err(e) = self.connection.send(&request) {
            #[cfg(feature = "tracing")]
            tracing::warn!(%e, "Failed to re-subscribe to RTDS topics");
            #[cfg(not(feature = "tracing"))]
            let _: &crate::error::Error = &e;
        }
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "Subscription is consumed to build SubscriptionInfo"
    )]
    pub fn subscribe(
        &self,
        subscription: Subscription,
    ) -> Result<impl Stream<Item = Result<RtdsMessage>>> {
        let topic_type = TopicType::new(subscription.topic.clone(), subscription.msg_type.clone());

        if let Some(auth) = &subscription.clob_auth {
            *self
                .last_auth
                .write()
                .unwrap_or_else(PoisonError::into_inner) = Some(auth.clone());
        }

        match self.subscribed_topics.entry(topic_type.clone()) {
            Entry::Occupied(mut entry) => {
                *entry.get_mut() += 1;
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    topic = %subscription.topic,
                    msg_type = %subscription.msg_type,
                    "RTDS topic already subscribed, multiplexing"
                );
            }
            Entry::Vacant(entry) => {
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    topic = %subscription.topic,
                    msg_type = %subscription.msg_type,
                    "Subscribing to RTDS topic"
                );

                let request = SubscriptionRequest::subscribe(vec![subscription.clone()]);
                self.connection.send(&request)?;
                
                entry.insert(1);
            }
        }

        let sub_id = format!("{}:{}", topic_type.topic, topic_type.msg_type);
        self.active_subs.insert(
            sub_id,
            SubscriptionInfo {
                topic_type: topic_type.clone(),
                filters: subscription.filters.clone(),
                clob_auth: subscription.clob_auth.clone(),
                created_at: Instant::now(),
            },
        );

        let mut rx = self.connection.subscribe();
        let target_topic = topic_type.topic;
        let target_type = topic_type.msg_type;

        Ok(try_stream! {
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        
                        let matches_topic = msg.topic == target_topic;
                        let matches_type = target_type == "*" || msg.msg_type == target_type;

                        if matches_topic && matches_type {
                            yield msg;
                        }
                    }
                    Err(RecvError::Lagged(n)) => {
                        #[cfg(not(feature = "tracing"))]
                        let _ = n;
                        #[cfg(feature = "tracing")]
                        tracing::warn!("RTDS subscription lagged, missed {n} messages — continuing");
                    }
                    Err(RecvError::Closed) => {
                        break;
                    }
                }
            }
        })
    }

    #[must_use]
    pub fn active_subscriptions(&self) -> Vec<SubscriptionInfo> {
        self.active_subs
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    #[must_use]
    pub fn subscription_count(&self) -> usize {
        self.active_subs.len()
    }

    pub fn unsubscribe(&self, topic_types: &[TopicType]) -> Result<()> {
        if topic_types.is_empty() {
            return Err(RtdsError::SubscriptionFailed(
                "topic_types cannot be empty: at least one topic must be provided for unsubscription"
                    .to_owned(),
            )
            .into());
        }

        for topic_type in topic_types {
            if let Entry::Occupied(mut entry) = self.subscribed_topics.entry(topic_type.clone()) {
                let refcount = entry.get_mut();
                *refcount = refcount.saturating_sub(1);
                if *refcount == 0 {
                    #[cfg(feature = "tracing")]
                    tracing::debug!(
                        topic = %topic_type.topic,
                        msg_type = %topic_type.msg_type,
                        "Unsubscribing from RTDS topic"
                    );

                    let request = SubscriptionRequest::unsubscribe(vec![Subscription {
                        topic: topic_type.topic.clone(),
                        msg_type: topic_type.msg_type.clone(),
                        filters: None,
                        clob_auth: None,
                    }]);
                    self.connection.send(&request)?;
                    entry.remove();
                }
            }
        }

        self.active_subs
            .retain(|_, info| self.subscribed_topics.contains_key(&info.topic_type));

        Ok(())
    }
}
