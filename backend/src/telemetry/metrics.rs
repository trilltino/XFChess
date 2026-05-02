//! Prometheus-compatible metrics for XFChess backend
//!
//! Tracks API performance, Solana transactions, game sessions, and infrastructure health.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Core metrics collection
#[derive(Debug)]
pub struct Metrics {
    // API metrics
    pub http_requests_total: HashMap<(String, u16), AtomicU64>,
    pub http_request_duration: HashMap<String, Vec<f64>>,
    
    // Solana RPC metrics
    pub solana_rpc_calls_total: HashMap<(String, String), AtomicU64>,
    pub solana_rpc_latency: HashMap<String, Vec<f64>>,
    pub solana_rpc_errors_total: HashMap<(String, String), AtomicU64>,
    
    // Transaction metrics
    pub transactions_submitted_total: HashMap<String, AtomicU64>,
    pub transactions_confirmed_total: HashMap<String, AtomicU64>,
    pub transactions_failed_total: HashMap<(String, String), AtomicU64>,
    pub transaction_confirmation_time: HashMap<String, Vec<f64>>,
    
    // Game session metrics
    pub active_sessions: AtomicU64,
    pub games_created_total: AtomicU64,
    pub games_finalized_total: HashMap<String, AtomicU64>,
    
    // Fee payer metrics
    pub feepayer_balance_lamports: HashMap<usize, AtomicU64>,
    pub feepayer_transactions_total: HashMap<usize, AtomicU64>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            http_requests_total: HashMap::new(),
            http_request_duration: HashMap::new(),
            solana_rpc_calls_total: HashMap::new(),
            solana_rpc_latency: HashMap::new(),
            solana_rpc_errors_total: HashMap::new(),
            transactions_submitted_total: HashMap::new(),
            transactions_confirmed_total: HashMap::new(),
            transactions_failed_total: HashMap::new(),
            transaction_confirmation_time: HashMap::new(),
            active_sessions: AtomicU64::new(0),
            games_created_total: AtomicU64::new(0),
            games_finalized_total: HashMap::new(),
            feepayer_balance_lamports: HashMap::new(),
            feepayer_transactions_total: HashMap::new(),
        }
    }
    
    /// Record an HTTP request
    pub fn record_http_request(&mut self, endpoint: &str, status: u16, duration_ms: f64) {
        let key = (endpoint.to_string(), status);
        self.http_requests_total
            .entry(key)
            .or_insert_with(AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
        
        self.http_request_duration
            .entry(endpoint.to_string())
            .or_insert_with(Vec::new)
            .push(duration_ms);
    }
    
    /// Record Solana RPC call
    pub fn record_solana_rpc_call(&mut self, method: &str, success: bool, latency_ms: f64) {
        let status = if success { "success" } else { "error" };
        let key = (method.to_string(), status.to_string());
        self.solana_rpc_calls_total
            .entry(key)
            .or_insert_with(AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
        
        self.solana_rpc_latency
            .entry(method.to_string())
            .or_insert_with(Vec::new)
            .push(latency_ms);
    }
    
    /// Record transaction submission
    pub fn record_transaction_submitted(&mut self, chain_type: &str) {
        self.transactions_submitted_total
            .entry(chain_type.to_string())
            .or_insert_with(AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record transaction confirmation
    pub fn record_transaction_confirmed(&mut self, chain_type: &str, confirmation_time_ms: f64) {
        self.transactions_confirmed_total
            .entry(chain_type.to_string())
            .or_insert_with(AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
        
        self.transaction_confirmation_time
            .entry(chain_type.to_string())
            .or_insert_with(Vec::new)
            .push(confirmation_time_ms);
    }
    
    /// Record transaction failure
    pub fn record_transaction_failed(&mut self, chain_type: &str, error_type: &str) {
        let key = (chain_type.to_string(), error_type.to_string());
        self.transactions_failed_total
            .entry(key)
            .or_insert_with(AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }
    
    /// Update fee payer balance
    pub fn update_feepayer_balance(&mut self, key_index: usize, lamports: u64) {
        self.feepayer_balance_lamports
            .entry(key_index)
            .or_insert_with(|| AtomicU64::new(0))
            .store(lamports, Ordering::Relaxed);
    }
    
    /// Get active sessions count
    pub fn get_active_sessions(&self) -> u64 {
        self.active_sessions.load(Ordering::Relaxed)
    }
    
    /// Increment active sessions
    pub fn increment_active_sessions(&self) {
        self.active_sessions.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Decrement active sessions
    pub fn decrement_active_sessions(&self) {
        self.active_sessions.fetch_sub(1, Ordering::Relaxed);
    }
    
    /// Export metrics in Prometheus format
    pub fn export_prometheus_format(&self) -> String {
        let mut output = String::new();
        
        // HTTP requests
        output.push_str("# HELP http_requests_total Total HTTP requests\n");
        output.push_str("# TYPE http_requests_total counter\n");
        for ((endpoint, status), count) in &self.http_requests_total {
            let value = count.load(Ordering::Relaxed);
            output.push_str(&format!(
                "http_requests_total{{endpoint=\"{}\",status=\"{}\"}} {}\n",
                endpoint, status, value
            ));
        }
        
        // Transaction submissions
        output.push_str("\n# HELP transactions_submitted_total Total transactions submitted\n");
        output.push_str("# TYPE transactions_submitted_total counter\n");
        for (chain_type, count) in &self.transactions_submitted_total {
            let value = count.load(Ordering::Relaxed);
            output.push_str(&format!(
                "transactions_submitted_total{{chain=\"{}\"}} {}\n",
                chain_type, value
            ));
        }
        
        // Transaction confirmations
        output.push_str("\n# HELP transactions_confirmed_total Total transactions confirmed\n");
        output.push_str("# TYPE transactions_confirmed_total counter\n");
        for (chain_type, count) in &self.transactions_confirmed_total {
            let value = count.load(Ordering::Relaxed);
            output.push_str(&format!(
                "transactions_confirmed_total{{chain=\"{}\"}} {}\n",
                chain_type, value
            ));
        }
        
        // Active sessions
        output.push_str("\n# HELP active_sessions Current active game sessions\n");
        output.push_str("# TYPE active_sessions gauge\n");
        output.push_str(&format!(
            "active_sessions {}\n",
            self.get_active_sessions()
        ));
        
        // Games created
        output.push_str("\n# HELP games_created_total Total games created\n");
        output.push_str("# TYPE games_created_total counter\n");
        output.push_str(&format!(
            "games_created_total {}\n",
            self.games_created_total.load(Ordering::Relaxed)
        ));
        
        output
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
