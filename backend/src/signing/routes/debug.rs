//! Debug and health check routes for XFChess backend
//!
//! Provides:
//! - /health - Basic health check
//! - /health/detailed - Full system health with all components
//! - /api/debug/tx/{signature} - Transaction debugging
//!
//! `/metrics` lives in `infrastructure::router` (it needs `AppState` for the
//! presence/transaction/RPC counters, not just the worker-metrics registry).

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
pub async fn detailed_health_check(State(state): State<AppState>) -> impl IntoResponse {
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

/// `GET /api/debug/tx/{signature}` — inspects an on-chain transaction via a
/// real `getTransaction` RPC call. `success`/`error`/`logs`/`account_changes`
/// reflect what actually happened on-chain, not a hardcoded default. Returns
/// 400 on an unparseable signature, 404 if the RPC can't find the transaction
/// (not yet confirmed, wrong cluster, or genuinely unknown).
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
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!("Failed to fetch transaction: {}", e)
            })),
        ),
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

// Both checks below run their RPC call on a blocking thread. They are reached
// from `/health/detailed`, which is unauthenticated: called directly on the
// async runtime, each one parks a Tokio worker for up to the 30s RPC timeout,
// so a handful of concurrent health probes could stall every other request the
// server was serving — including `/health` itself. They also reuse the shared
// `RpcClient` rather than building one (and a fresh TLS handshake) per call.

async fn check_solana_rpc(state: &AppState) -> Result<u64, Box<dyn std::error::Error>> {
    let rpc = std::sync::Arc::clone(&state.solana_rpc);
    let started = std::time::Instant::now();
    let result = tokio::task::spawn_blocking(move || rpc.get_slot()).await?;
    state.metrics.record_solana_rpc_call(
        "getSlot",
        result.is_ok(),
        started.elapsed().as_millis() as f64,
    );
    let slot = result?;
    Ok(slot)
}

async fn check_feepayer_pool(state: &AppState) -> (Option<String>, String) {
    let fee_payer_pubkey = state.feepayer.next().pubkey();
    let rpc = std::sync::Arc::clone(&state.solana_rpc);

    let balance = tokio::task::spawn_blocking(move || rpc.get_balance(&fee_payer_pubkey)).await;
    let balance = match balance {
        Ok(inner) => inner,
        Err(e) => {
            return (
                Some(format!("Balance check task failed: {e}")),
                "error".to_string(),
            )
        }
    };

    match balance {
        Ok(balance) => {
            // key_index 0: the pool round-robins and doesn't expose which slot
            // `.next()` picked, so this gauge tracks "whichever fee payer we
            // most recently checked" rather than a per-key breakdown.
            state.metrics.update_feepayer_balance(0, balance);
            let sol = balance as f64 / 1_000_000_000.0;
            if balance < 10_000_000 {
                // Less than 0.01 SOL
                (
                    Some(format!("Low balance: {} SOL", sol)),
                    "warning".to_string(),
                )
            } else {
                (Some(format!("Balance: {} SOL", sol)), "ok".to_string())
            }
        }
        Err(e) => (
            Some(format!("Error checking balance: {}", e)),
            "error".to_string(),
        ),
    }
}

/// Reports usage of whichever disk holds the current working directory (where
/// the session/vault SQLite files and PID file live) — the mount that
/// actually matters for this process staying up. Cross-platform via
/// `sysinfo`, replacing the old Unix-only `df` shell-out (which silently
/// reported "warning" on every Windows dev box regardless of real usage).
async fn check_disk_space() -> (Option<String>, String) {
    use sysinfo::Disks;

    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            return (
                Some(format!("Could not determine working directory: {e}")),
                "error".to_string(),
            )
        }
    };

    let disks = Disks::new_with_refreshed_list();
    // Pick the disk whose mount point is the longest prefix of `cwd` — the
    // most specific match (e.g. a separate /data mount over the root disk).
    let best = disks
        .list()
        .iter()
        .filter(|d| cwd.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len());

    let Some(disk) = best else {
        return (
            Some("No disk found containing the working directory".to_string()),
            "warning".to_string(),
        );
    };

    let total = disk.total_space();
    if total == 0 {
        return (
            Some("Disk reported zero total space".to_string()),
            "warning".to_string(),
        );
    }
    let used = total.saturating_sub(disk.available_space());
    let usage_percent = (used as f64 / total as f64) * 100.0;
    let msg = format!(
        "Disk usage: {:.1}% ({} used of {} on {})",
        usage_percent,
        format_bytes(used),
        format_bytes(total),
        disk.mount_point().display()
    );

    if usage_percent > 90.0 {
        (Some(msg), "critical".to_string())
    } else if usage_percent > 80.0 {
        (Some(msg), "warning".to_string())
    } else {
        (Some(msg), "ok".to_string())
    }
}

fn format_bytes(bytes: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    format!("{:.2} GB", bytes as f64 / GB)
}

/// Real resident-memory check via `sysinfo`, replacing the old placeholder
/// that always reported "ok" regardless of actual memory pressure.
async fn check_memory() -> (Option<String>, String) {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_memory();
    let total = sys.total_memory();
    if total == 0 {
        return (
            Some("Could not determine total system memory".to_string()),
            "warning".to_string(),
        );
    }
    let used = sys.used_memory();
    let usage_percent = (used as f64 / total as f64) * 100.0;
    let msg = format!(
        "Memory usage: {:.1}% ({} used of {})",
        usage_percent,
        format_bytes(used),
        format_bytes(total)
    );

    if usage_percent > 90.0 {
        (Some(msg), "critical".to_string())
    } else if usage_percent > 80.0 {
        (Some(msg), "warning".to_string())
    } else {
        (Some(msg), "ok".to_string())
    }
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
