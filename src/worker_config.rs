//! # Worker Poll Configuration
//!
//! Polling intervals are shared atomically between the UI control panel and
//! the worker polling loops, so interval changes take effect on the next tick
//! without any additional message passing.

use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum Queue {
    Orders,
    Trades,
    RapidSell,
}

pub struct PollConfig {
    intervals: HashMap<Queue, Arc<AtomicU64>>,
}

pub type SharedPollConfig = Arc<PollConfig>;

impl PollConfig {
    pub fn new() -> Self {
        let mut intervals = HashMap::new();
        intervals.insert(Queue::Orders, Arc::new(AtomicU64::new(4_000)));
        intervals.insert(Queue::Trades, Arc::new(AtomicU64::new(2_500)));
        intervals.insert(Queue::RapidSell, Arc::new(AtomicU64::new(1_000)));
        Self { intervals }
    }

    pub fn set(&self, q: Queue, value: u64) {
        if let Some(cell) = self.intervals.get(&q) {
            cell.store(value, Ordering::Relaxed);
        }
    }

    pub fn get(&self, q: Queue) -> u64 {
        self.intervals
            .get(&q)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Return a cloned `Arc<AtomicU64>` so polling loops can watch for changes
    /// without holding a reference to `PollConfig`.
    pub fn atomic(&self, q: Queue) -> Arc<AtomicU64> {
        self.intervals.get(&q).unwrap().clone()
    }
}
