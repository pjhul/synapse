use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;

pub trait Metrics {
    fn increment(&self, key: &str, value: u64) {
        let mut metrics = METRICS_HUB.metrics.lock().unwrap();
        let counter = metrics.entry(key.to_string()).or_insert(0);
        *counter += value;
    }

    fn decrement(&self, key: &str, value: u64) {
        let mut metrics = METRICS_HUB.metrics.lock().unwrap();
        let counter = metrics.entry(key.to_string()).or_insert(0);
        *counter -= value;
    }
}

pub struct MetricsHub {
    // FIXME: This is very inefficient for the type of mostly increment/decrement only access
    // pattern we have here. Perhaps some sort of event based system that aggregates over time
    // would be better.
    metrics: Mutex<HashMap<String, u64>>,
}

impl MetricsHub {
    pub fn new() -> Self {
        MetricsHub {
            metrics: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_metrics(&self) -> HashMap<String, u64> {
        let metrics = self.metrics.lock().unwrap();
        metrics.clone()
    }

    pub fn get(&self, key: &str) -> u64 {
        let metrics = self.metrics.lock().unwrap();
        let counter = metrics.get(key);
        match counter {
            Some(value) => *value,
            None => 0,
        }
    }

    pub fn reset(&self) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.clear();
    }
}

impl Default for MetricsHub {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static! {
    static ref METRICS_HUB: MetricsHub = MetricsHub::new();
}

pub fn get_metrics() -> HashMap<String, u64> {
    METRICS_HUB.get_metrics()
}
