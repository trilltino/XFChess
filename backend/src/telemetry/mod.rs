//! Telemetry module for XFChess backend
//!
//! Provides structured logging, metrics collection, and request tracing
//! for observability into API performance, Solana transactions, and game sessions.

pub mod logging;
pub mod metrics;
pub mod middleware;
pub mod worker_metrics;

pub use logging::{RequestContext, StructuredLogger};
pub use metrics::Metrics;
pub use middleware::telemetry_middleware;

use std::sync::Arc;
use tokio::sync::RwLock;

/// Global telemetry state
#[derive(Clone)]
pub struct TelemetryState {
    pub metrics: Arc<RwLock<Metrics>>,
}

impl TelemetryState {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(Metrics::new())),
        }
    }
}

impl Default for TelemetryState {
    fn default() -> Self {
        Self::new()
    }
}
