use lazy_static::lazy_static;
use prometheus::{Counter, Gauge, IntCounter, IntGauge, Registry};
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

pub trait Metrics {
    fn increment_active_connections(&self) {
        METRICS_HUB.active_connections.inc();
    }

    fn decrement_active_connections(&self) {
        METRICS_HUB.active_connections.dec();
    }

    fn increment_messages_received(&self) {
        METRICS_HUB.messages_received.inc();
    }
}

pub struct MetricsHub {
    active_connections: IntGauge,
    messages_received: IntCounter,
}

impl MetricsHub {
    pub fn new() -> Self {
        MetricsHub {
            active_connections: IntGauge::new("active_connections", "Active websocket connections")
                .unwrap(),
            messages_received: IntCounter::new("messages_received", "Messages received").unwrap(),
        }
    }

    pub fn get_metrics(&self) -> Result<Vec<prometheus::proto::MetricFamily>, ()> {
        let registry = Registry::new();
        registry
            .register(Box::new(self.active_connections.clone()))
            .unwrap();
        registry
            .register(Box::new(self.messages_received.clone()))
            .unwrap();
        Ok(registry.gather())
    }

    pub fn reset(&self) {
        self.active_connections.set(0);
        self.messages_received.reset();
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

pub fn get_metrics() -> Result<Vec<prometheus::proto::MetricFamily>, ()> {
    METRICS_HUB.get_metrics()
}
