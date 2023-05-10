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

    fn increment_messages_sent(&self) {
        METRICS_HUB.messages_sent.inc();
    }
}

pub struct MetricsHub {
    active_connections: IntGauge,
    messages_received: IntCounter,
    messages_sent: IntCounter,
    // TODO: Add total number of channels
    // TODO: Add a way of tracking the number of messages per channel
    // TODO: Add a way of tracking the number of connections per channel
}

impl MetricsHub {
    pub fn new() -> Self {
        MetricsHub {
            active_connections: IntGauge::new("active_connections", "Active websocket connections")
                .unwrap(),
            messages_received: IntCounter::new("messages_received", "Messages received").unwrap(),
            messages_sent: IntCounter::new("messages_sent", "Messages sent").unwrap(),
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

        registry
            .register(Box::new(self.messages_sent.clone()))
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
