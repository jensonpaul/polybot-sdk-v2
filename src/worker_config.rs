use std::sync::{Arc};
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum Queue {
    Orders,
    Trades,
    RapidSell,
}

pub struct PollConfig {
    pub intervals: HashMap<Queue, Arc<AtomicU64>>,
}

pub type SharedPollConfig = Arc<PollConfig>;

impl PollConfig {
    pub fn new() -> Self {
        let mut intervals = HashMap::new();

        intervals.insert(
            Queue::Orders,
            Arc::new(AtomicU64::new(4000)),
        );

        intervals.insert(
            Queue::Trades,
            Arc::new(AtomicU64::new(2500)),
        );

        intervals.insert(
            Queue::RapidSell,
            Arc::new(AtomicU64::new(1000)),
        );

        Self { intervals }
    }

    pub fn set(&self, q: Queue, value: u64) {
        if let Some(interval) = self.intervals.get(&q) {
            interval.store(value, Ordering::Relaxed);
        }
    }

    pub fn get_atomic(&self, q: Queue) -> Arc<AtomicU64> {
        self.intervals
            .get(&q)
            .unwrap()
            .clone()
    }

    pub fn get(&self, q: Queue) -> u64 {
        self.intervals
            .get(&q)
            .unwrap()
            .load(Ordering::Relaxed)
    }
}