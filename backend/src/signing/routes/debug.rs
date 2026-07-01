//! Debug and health check routes for XFChess backend
//!
//! Provides:
//! - /health - Basic health check
//! - /health/detailed - Full system health with all components
//! - /metrics - Prometheus-compatible metrics export
//! - /api/debug/tx/{signature} - Transaction debugging

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use solana_sdk::signature::{Signature, Signer};
use std::str::FromStr;

use crate::signing::{
    solana::{debug_transaction, format_debug_info, TransactionDebugInfo},
    AppState,
};

/// Basic health response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    /// Git commit the binary was built from (deploy → commit traceability).
    pub git_sha: String,
    pub timestamp: String,
}

/// Detailed health check response
#[derive(Serialize)]
pub struct DetailedHealthResponse {
    pub status: String,
    pub version: String,
    pub timestamp: String,
    pub checks: Vec<HealthCheck>,
}

/// Individual health check
#[derive(Serialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: String,
    pub message: Option<String>,
    pub response_time_ms: u64,
}

/// Transaction debug response
#[derive(Serialize)]
pub struct DebugTxResponse {
    pub signature: String,
    pub debug_info: TransactionDebugInfo,
    pub formatted: String,
}

/// Basic liveness check — is the process up? (cheap, no dependency I/O)
pub async fn health_check() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        git_sha: option_env!("GIT_SHA").unwrap_or("unknown").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Readiness check — can we actually serve traffic? Verifies DB connectivity and
/// returns 503 if not, so deploy smoke-tests / load balancers don't route to a
/// process that's up but can't reach its database.
pub async fn readiness_check(State(state): State<AppState>) -> impl IntoResponse {
    match check_database(&state).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "ready",
                "git_sha": option_env!("GIT_SHA").unwrap_or("unknown"),
            })),
        ),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "status": "not_ready", "error": e.to_string() })),
        ),
    }
}

/// Detailed health check
pub async fn detailed_health_check(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let _start = std::time::Instant::now();
    let mut checks = vec![];
    
    // Check database connectivity
    let db_start = std::time::Instant::now();
    let db_check = match check_database(&state).await {
        Ok(_) => HealthCheck {
            name: "database".to_string(),
            status: "ok".to_string(),
            message: None,
            response_time_ms: db_start.elapsed().as_millis() as u64,
        },
        Err(e) => HealthCheck {
            name: "database".to_string(),
            status: "error".to_string(),
            message: Some(e.to_string()),
            response_time_ms: db_start.elapsed().as_millis() as u64,
        },
    };
    checks.push(db_check);
    
    // Check Solana RPC
    let rpc_start = std::time::Instant::now();
    let rpc_check = match check_solana_rpc(&state).await {
        Ok(slot) => HealthCheck {
            name: "solana_rpc".to_string(),
            status: "ok".to_string(),
            message: Some(format!("Current slot: {}", slot)),
            response_time_ms: rpc_start.elapsed().as_millis() as u64,
        },
        Err(e) => HealthCheck {
            name: "solana_rpc".to_string(),
            status: "error".to_string(),
            message: Some(e.to_string()),
            response_time_ms: rpc_start.elapsed().as_millis() as u64,
        },
    };
    checks.push(rpc_check);
    
    // Check fee payer pool
    let feepayer_start = std::time::Instant::now();
    let feepayer_check = check_feepayer_pool(&state).await;
    checks.push(HealthCheck {
        name: "feepayer_pool".to_string(),
        status: feepayer_check.1.clone(),
        message: feepayer_check.0,
        response_time_ms: feepayer_start.elapsed().as_millis() as u64,
    });
    
    // Check disk space
    let disk_start = std::time::Instant::now();
    let disk_check = check_disk_space().await;
    checks.push(HealthCheck {
        name: "disk_space".to_string(),
        status: disk_check.1,
        message: disk_check.0,
        response_time_ms: disk_start.elapsed().as_millis() as u64,
    });
    
    // Check memory
    let memory_start = std::time::Instant::now();
    let memory_check = check_memory().await;
    checks.push(HealthCheck {
        name: "memory".to_string(),
        status: memory_check.1,
        message: memory_check.0,
        response_time_ms: memory_start.elapsed().as_millis() as u64,
    });
    
    // Determine overall status
    let overall_status = if checks.iter().all(|c| c.status == "ok") {
        "healthy"
    } else if checks.iter().any(|c| c.status == "critical") {
        "critical"
    } else {
        "degraded"
    };
    
    let response = DetailedHealthResponse {
        status: overall_status.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        checks,
    };
    
    let status_code = match overall_status {
        "healthy" => StatusCode::OK,
        "degraded" => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };
    
    (status_code, Json(response))
}

/// Metrics endpoint for Prometheus
pub async fn metrics_endpoint(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    let mut metrics = format!(
        "# HELP xfchess_health Health status (1 = healthy, 0 = unhealthy)\n\
         # TYPE xfchess_health gauge\n\
         xfchess_health 1\n\
         \n\
         # HELP xfchess_version Version info\n\
         # TYPE xfchess_version gauge\n\
         xfchess_version{{version=\"{}\"}} 1\n",
        env!("CARGO_PKG_VERSION")
    );
    metrics.push('\n');
    metrics.push_str(&crate::telemetry::worker_metrics::render_prometheus());

    ([("content-type", "text/plain")], metrics)
}

/// Debug transaction endpoint
pub async fn debug_transaction_endpoint(
    Path(signature): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Parse signature
    let sig = match Signature::from_str(&signature) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Invalid signature: {}", e)
                })),
            );
        }
    };
    
    // Create RPC client
    let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);
    
    // Fetch debug info
    match debug_transaction(&rpc, &sig).await {
        Ok(debug_info) => {
            let formatted = format_debug_info(&debug_info);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "signature": signature,
                    "debug_info": debug_info,
                    "formatted": formatted
                })),
            )
        }
        Err(e) => {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": format!("Failed to fetch transaction: {}", e)
                })),
            )
        }
    }
}

/// Build debug routes
pub fn debug_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_check))
        .route("/readyz", get(readiness_check))
        .route("/health/detailed", get(detailed_health_check))
        .route("/api/debug/tx/{signature}", get(debug_transaction_endpoint))
}

// Health check helpers

async fn check_database(state: &AppState) -> Result<(), Box<dyn std::error::Error>> {
    // Try a simple query on the vault pool
    let _row: (i64,) = sqlx::query_as("SELECT 1")
        .fetch_one(&*state.vault_pool)
        .await?;
    Ok(())
}

async fn check_solana_rpc(state: &AppState) -> Result<u64, Box<dyn std::error::Error>> {
    let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);
    let slot = rpc.get_slot()?;
    Ok(slot)
}

async fn check_feepayer_pool(state: &AppState) -> (Option<String>, String) {
    // Get a fee payer and check its balance
    let fee_payer = state.feepayer.next();
    let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);
    
    match rpc.get_balance(&fee_payer.pubkey()) {
        Ok(balance) => {
            let sol = balance as f64 / 1_000_000_000.0;
            if balance < 10_000_000 { // Less than 0.01 SOL
                (Some(format!("Low balance: {} SOL", sol)), "warning".to_string())
            } else {
                (Some(format!("Balance: {} SOL", sol)), "ok".to_string())
            }
        }
        Err(e) => (Some(format!("Error checking balance: {}", e)), "error".to_string()),
    }
}

async fn check_disk_space() -> (Option<String>, String) {
    // Check disk space using system command
    #[cfg(unix)]
    {
        use std::process::Command;
        match Command::new("df").args(["-h", "/"]).output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<&str> = stdout.lines().collect();
                if lines.len() >= 2 {
                    let parts: Vec<&str> = lines[1].split_whitespace().collect();
                    if parts.len() >= 5 {
                        let usage = parts[4];
                        let usage_percent = usage.trim_end_matches('%').parse::<u32>().unwrap_or(0);
                        
                        if usage_percent > 90 {
                            (Some(format!("Disk usage: {}", usage)), "critical".to_string())
                        } else if usage_percent > 80 {
                            (Some(format!("Disk usage: {}", usage)), "warning".to_string())
                        } else {
                            (Some(format!("Disk usage: {}", usage)), "ok".to_string())
                        }
                    } else {
                        (Some("Could not parse disk info".to_string()), "warning".to_string())
                    }
                } else {
                    (Some("Could not get disk info".to_string()), "warning".to_string())
                }
            }
            Err(e) => (Some(format!("Error checking disk: {}", e)), "error".to_string()),
        }
    }
    
    #[cfg(not(unix))]
    {
        (Some("Disk check not available on Windows".to_string()), "warning".to_string())
    }
}

async fn check_memory() -> (Option<String>, String) {
    // This is a placeholder - in production you'd use sysinfo crate
    (Some("Memory check available".to_string()), "ok".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_health_check() {
        let _response = health_check().await;
        // Just verify it doesn't panic
    }
}
