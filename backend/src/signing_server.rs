use backend::infrastructure::{initialize_pools, run_migrations, spawn_background_tasks};
use backend::signing::storage::tournament::TournamentStore;
use backend::signing::storage::SessionStore;
use backend::signing::{AppState, SigningConfig};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::EnvFilter;

const PID_FILE: &str = ".backend.pid";

fn write_pid_file() {
    let pid = std::process::id();
    if let Err(e) = std::fs::write(PID_FILE, pid.to_string()) {
        tracing::warn!("[signing-server] Could not write PID file: {}", e);
    }
}

fn remove_pid_file() {
    let _ = std::fs::remove_file(PID_FILE);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    // Structured JSON logs when LOG_FORMAT=json (production: machine-parseable, one
    // object per line, includes the per-request `request_id` span field). Human-readable
    // pretty logs otherwise (local dev).
    let json_logs = std::env::var("LOG_FORMAT").is_ok_and(|v| v.eq_ignore_ascii_case("json"));
    if json_logs {
        tracing_subscriber::fmt()
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    }

    let config = SigningConfig::from_env();

    // ── Fail fast on bad/placeholder config (hard error under APP_ENV=production) ──
    if let Err(problems) = config.validate() {
        eprintln!("[signing-server] FATAL: invalid production configuration:\n{problems}");
        std::process::exit(1);
    }

    let port = config.port;

    // ── Write PID so the launcher can kill exactly this process on restart ──
    write_pid_file();

    // ── Initialize database pools ────────────────────────────────────────
    // Read DB locations from the environment so production can point them at
    // /opt/xfchess/data (the only writable path under the hardened systemd
    // unit). Falls back to the local-dev defaults when unset.
    let session_db =
        std::env::var("SESSION_DB_URL").unwrap_or_else(|_| "sqlite://sessions.db?mode=rwc".into());
    let vault_db =
        std::env::var("VAULT_DB_URL").unwrap_or_else(|_| "sqlite://vault.db?mode=rwc".into());
    let pools = initialize_pools(&session_db, &vault_db).await?;
    info!("[signing-server] Database pools initialized");

    // ── Run database migrations ───────────────────────────────────────────
    run_migrations(&pools).await?;
    info!("[signing-server] Database migrations completed");

    // ── Initialize application state ─────────────────────────────────────
    let session_store = SessionStore::new(pools.session_pool.clone());
    session_store.init().await?;
    info!("[signing-server] Session store initialized");

    let tournament_store = TournamentStore::new(pools.session_pool.clone()).await;
    info!("[signing-server] Tournament store initialized");

    let mut state = AppState::new(
        config.clone(),
        pools.session_pool.clone(),
        pools.vault_pool.clone(),
        Arc::new(tournament_store.clone()),
    );

    // ── Initialize social tables (friends, contacts) ─────────────────────────
    if let Err(e) = state.friends.init().await {
        tracing::warn!("[signing-server] Failed to init friends tables: {}", e);
    }
    info!("[signing-server] Social tables initialized");

    // ── Build application router ───────────────────────────────────────────
    let app = backend::infrastructure::build_app_router(state.clone());
    info!("[signing-server] Application router built");

    // ── Spawn Braid-Iroh P2P node (ephemeral UDP port, no conflict with HTTP) ─
    info!("[signing-server] Spawning Braid-Iroh Node (ephemeral port)");
    let (_braid_state, _rx) =
        braid_iroh::spawn_node("xfchess-vps", None, None, braid_iroh::DiscoveryConfig::Real)
            .await
            .map_err(|e| anyhow::anyhow!("failed to spawn braid-iroh node: {}", e))?;

    // ── Spawn background tasks ───────────────────────────────────────────────
    let (tournament_trigger, ac_queue) = spawn_background_tasks(state.clone(), config);
    state.tournament_trigger = Some(tournament_trigger);
    state.anticheat_queue = Some(ac_queue);
    info!("[signing-server] Background tasks spawned");

    // ── Start HTTP Server ──────────────────────────────────────────────────
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to bind TCP listener on port {}: {}", port, e))?;
    info!(
        "[signing-server] Listening for HTTP traffic on port {}",
        port
    );

    // Serve with graceful shutdown: on SIGTERM (systemctl stop/restart) or Ctrl-C,
    // stop accepting new connections and let in-flight requests finish before exit.
    axum::serve(listener, app.with_state(state.clone()))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| anyhow::anyhow!("HTTP server error: {}", e))?;

    info!("[signing-server] Graceful shutdown complete");
    remove_pid_file();

    Ok(())
}

/// Resolve when the process receives a shutdown signal: SIGTERM (systemd) or Ctrl-C.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => tracing::warn!("[signing-server] failed to install SIGTERM handler: {}", e),
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("[signing-server] SIGINT received — shutting down"),
        _ = terminate => info!("[signing-server] SIGTERM received — shutting down"),
    }
}
