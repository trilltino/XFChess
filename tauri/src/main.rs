#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::Command;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tauri::Manager;
use backend::signing::{AppState as SigningAppState, SigningConfig, build_router as build_signing_router};
use backend::signing::storage::tournament::TournamentStore;
use sqlx::SqlitePool;
use dotenvy;
use tauri_plugin_deep_link::DeepLinkExt;

#[cfg(windows)]
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

/// Wallet pubkey in base58 — written by the React app after Phantom connects.
#[derive(Default, Clone)]
struct WalletPubkey(Arc<Mutex<Option<String>>>);

/// In-flight signing request: raw tx bytes + a channel to return signed bytes.
type PendingTxInner = Option<(Vec<u8>, oneshot::Sender<Result<Vec<u8>, String>>)>;
type PendingTx = Arc<Mutex<PendingTxInner>>;

/// HTTP port for the wallet signing page.
/// Reads XFCHESS_WALLET_PORT env var; defaults to 7454.
fn http_port() -> u16 {
    std::env::var("XFCHESS_WALLET_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7454)
}

/// Get the base URL for the backend (defaults to localhost).
fn get_backend_url() -> String {
    std::env::var("SIGNING_SERVICE_URL")
        .or_else(|_| std::env::var("BACKEND_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:8090".to_string())
}

/// Open the login page in the browser.
fn open_production_site() {
    let backend_url = get_backend_url();
    let is_local = backend_url.contains("localhost") || backend_url.contains("127.0.0.1");
    
    let url = if is_local {
        format!("http://localhost:{}/auth/login", http_port())
    } else {
        format!("{}/auth/login", backend_url.replace(":8090", ""))
    };
    
    match open::that(&url) {
        Ok(_) => println!("[TAURI] Opened login page in browser: {}", url),
        Err(e) => eprintln!("[TAURI] Failed to open site: {}", e),
    }
}

/// Open the wallet signing page when a transaction needs signing.
/// This is called during gameplay when the Bevy game needs a signature.
fn show_signing_window(_app: &tauri::AppHandle) {
    let port = http_port();
    let url = format!("http://localhost:{}/onboard", port);
    let _ = open::that(&url);
}

/// Register the xfchess:// protocol in Windows registry on first run.
/// Uses HKCU (user-level) so no admin/UAC prompt is required.
#[cfg(windows)]
fn register_protocol() {
    use winreg::enums::{KEY_WRITE, REG_SZ};
    
    let exe_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "xfchess-tauri.exe".to_string());
    
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    
    // Create HKCU\Software\Classes\xfchess
    if let Ok((key, _)) = hkcu.create_subkey("Software\\Classes\\xfchess") {
        let _ = key.set_value("", &"URL:XFChess Protocol");
        let _ = key.set_value("URL Protocol", &"");
        
        // Create shell\open\command subkey
        if let Ok((cmd_key, _)) = key.create_subkey("shell\\open\\command") {
            let command = format!("\"{}\" \"%1\"", exe_path);
            let _ = cmd_key.set_value("", &command);
        }
    }
    
    println!("[TAURI] Registered xfchess:// protocol for: {}", exe_path);
}

#[cfg(not(windows))]
fn register_protocol() {
    // No-op on non-Windows platforms
}

// ---------------------------------------------------------------------------
// HTTP signing bridge (axum)
// ---------------------------------------------------------------------------

async fn http_server(app: tauri::AppHandle, pending: PendingTx, wallet_pubkey: WalletPubkey) {
    use axum::{
        extract::State,
        http::{Method, StatusCode},
        response::IntoResponse,
        routing::{get, post},
        Json, Router,
    };
    use tower_http::cors::{Any, CorsLayer};

    #[derive(Clone)]
    struct LocalAppState {
        app: tauri::AppHandle,
        pending: PendingTx,
        wallet_pubkey: WalletPubkey,
    }

    // GET /pending — React polls this; returns {"tx": "<base64>"} or {"tx": null}
    async fn get_pending(
        State(state): State<LocalAppState>,
    ) -> impl IntoResponse {
        let lock = state.pending.lock().unwrap();
        let tx_b64 = lock.as_ref().map(|(bytes, _)| B64.encode(bytes));
        // If we have a TX, ensure signing window is shown
        if tx_b64.is_some() {
            show_signing_window(&state.app);
        }
        Json(serde_json::json!({ "tx": tx_b64 }))
    }

    // POST /resolved — React posts {"signed": "<base64>"} after Phantom signs
    async fn post_resolved(
        State(state): State<LocalAppState>,
        Json(body): Json<serde_json::Value>,
    ) -> impl IntoResponse {
        let signed_b64 = body["signed"].as_str().unwrap_or("").to_string();
        let mut lock = state.pending.lock().unwrap();
        if let Some((_, sender)) = lock.take() {
            if signed_b64.is_empty() {
                let _ = sender.send(Err("User cancelled".to_string()));
            } else {
                match B64.decode(&signed_b64) {
                    Ok(bytes) => { let _ = sender.send(Ok(bytes)); }
                    Err(e) => { let _ = sender.send(Err(format!("base64 decode: {}", e))); }
                }
            }
        }
        StatusCode::OK
    }

    // POST /wallet — React posts {"pubkey": "<base58>"} on wallet connect
    async fn post_wallet(
        State(state): State<LocalAppState>,
        Json(body): Json<serde_json::Value>,
    ) -> impl IntoResponse {
        if let Some(pk) = body["pubkey"].as_str() {
            let mut lock = state.wallet_pubkey.0.lock().unwrap();
            *lock = Some(pk.to_string());
            println!("[HTTP] Wallet connected: {}", pk);
        }
        StatusCode::OK
    }

    // GET /status — health-check / wallet info for the React page
    async fn get_status(State(state): State<LocalAppState>) -> impl IntoResponse {
        let pubkey = state.wallet_pubkey.0.lock().unwrap().clone();
        Json(serde_json::json!({ "connected": pubkey.is_some(), "pubkey": pubkey }))
    }

    // GET /api/consent — load consent record from disk
    async fn api_get_consent() -> impl IntoResponse {
        let path = consent_path();
        match std::fs::read_to_string(&path).ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        {
            Some(v) => Json(v).into_response(),
            None    => Json(serde_json::Value::Null).into_response(),
        }
    }

    // POST /api/consent — save consent record to disk
    async fn api_post_consent(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
        let version = body["version"].as_u64().unwrap_or(1) as u8;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        let record = serde_json::json!({ "version": version, "accepted_at": ts });
        let path = consent_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, record.to_string());
        StatusCode::OK
    }

    // POST /api/auth/login — proxy to Hetzner :8090
    async fn api_login(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
        let client = reqwest::Client::new();
        let url = format!("{}/api/auth/login", get_backend_url());
        match client.post(&url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(v) => (StatusCode::OK, Json(v)).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
                }
            }
            Ok(resp) => {
                let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_REQUEST);
                let text = resp.text().await.unwrap_or_default();
                (status, text).into_response()
            }
            Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
        }
    }

    // POST /api/auth/register — proxy to :8090
    async fn api_register(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
        let client = reqwest::Client::new();
        let base = get_backend_url();
        // Register
        match client.post(format!("{}/api/auth/register", base)).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                // Auto-login after register
                let login_body = serde_json::json!({
                    "email": body["email"], "password": body["password"]
                });
                match client.post(format!("{}/api/auth/login", base)).json(&login_body).send().await {
                    Ok(lr) if lr.status().is_success() => {
                        match lr.json::<serde_json::Value>().await {
                            Ok(v) => (StatusCode::OK, Json(v)).into_response(),
                            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
                        }
                    }
                    Ok(lr) => {
                        let text = lr.text().await.unwrap_or_default();
                        (StatusCode::BAD_REQUEST, text).into_response()
                    }
                    Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
                }
            }
            Ok(resp) => {
                let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_REQUEST);
                let text = resp.text().await.unwrap_or_default();
                (status, text).into_response()
            }
            Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
        }
    }

    // POST /api/game/launch — spawn Bevy game, store pubkey, hide window
    async fn api_launch_game(
        State(state): State<LocalAppState>,
        Json(body): Json<serde_json::Value>,
    ) -> impl IntoResponse {
        let pubkey_opt = body["pubkey"].as_str().map(|s| s.to_string());
        let is_hot = body["hot"].as_bool().unwrap_or(false);
        let username_opt = body["username"].as_str().map(|s| s.to_string());
        let ai_diff = body["ai_difficulty"].as_u64().map(|v| v as u8);
        let ai_side = body["ai_side"].as_str().map(|s| s.to_string());

        if let Some(ref pk) = pubkey_opt {
            let mut lock = state.wallet_pubkey.0.lock().unwrap();
            *lock = Some(pk.clone());
        }
        let token_opt = body["token"].as_str().map(|s| s.to_string());

        match spawn_bevy_game(true, pubkey_opt, is_hot, username_opt, ai_diff, ai_side, token_opt) {
            Ok(mut child) => {
                println!("[HTTP] Bevy game launched (PID {})", child.id());
                std::thread::spawn(move || {
                    let _ = child.wait();
                    println!("[TAURI] Game exited — shutting down");
                    std::process::exit(0);
                });
                if let Some(w) = state.app.get_webview_window("main") {
                    let _ = w.hide();
                }
                StatusCode::OK.into_response()
            }
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        }
    }

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let state = LocalAppState {
        app,
        pending: pending.clone(),
        wallet_pubkey: wallet_pubkey.clone(),
    };

    let app = Router::new()
        // Wallet onboarding SPA — served at /onboard (ServeDir with SPA index fallback)
        .nest_service(
            "/onboard",
            tower_http::services::ServeDir::new(dist_path())
                .fallback(tower_http::services::ServeFile::new(dist_path().join("index.html"))),
        )
        // API routes
        .route("/pending", get(get_pending))
        .route("/resolved", post(post_resolved))
        .route("/wallet", post(post_wallet))
        .route("/status", get(get_status))
        .route("/api/consent", get(api_get_consent).post(api_post_consent))
        .route("/api/auth/login", post(api_login))
        .route("/api/auth/register", post(api_register))
        .route("/api/game/launch", post(api_launch_game))
        // Web-solana marketing site — serves everything else at /
        .fallback_service(
            tower_http::services::ServeDir::new(site_path())
                .fallback(tower_http::services::ServeFile::new(site_path().join("index.html"))),
        )
        .layer(cors)
        .with_state(state);

    let port = http_port();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    println!("[HTTP] Wallet signing server on http://localhost:{}", port);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ---------------------------------------------------------------------------
// TCP signing bridge (Bevy ↔ Tauri)
// ---------------------------------------------------------------------------

async fn tcp_signing_server(app: tauri::AppHandle, pending: PendingTx, wallet_pubkey: WalletPubkey) {
    let listener = {
        let base = http_port();
        let tcp_start = base.saturating_sub(11);
        let tcp_end = base.saturating_sub(2);
        let mut found = None;
        for port in tcp_start..=tcp_end {
            if let Ok(l) = TcpListener::bind(("127.0.0.1", port)).await {
                println!("[SIGN-SRV] TCP bridge on 127.0.0.1:{}", port);
                found = Some(l);
                break;
            }
        }
        match found {
            Some(l) => l,
            None => {
                eprintln!("[SIGN-SRV] No port available in {}-{}", tcp_start, tcp_end);
                return;
            }
        }
    };

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(p) => p,
            Err(e) => { eprintln!("[SIGN-SRV] accept: {}", e); continue; }
        };

        let pending = pending.clone();
        let wallet_pubkey = wallet_pubkey.clone();
        let app = app.clone();

        tauri::async_runtime::spawn(async move {
            let (mut reader, mut writer) = stream.into_split();

            let mut len_buf = [0u8; 4];
            if reader.read_exact(&mut len_buf).await.is_err() {
                let _ = writer.write_all(&0xFFFF_FFFFu32.to_le_bytes()).await;
                return;
            }

            // OPEN command — open the wallet signing page
            if &len_buf == b"OPEN" {
                show_signing_window(&app);
                return;
            }

            // PKEY query — return the wallet pubkey as a UTF-8 base58 string.
            if &len_buf == b"PKEY" {
                let pubkey_str = wallet_pubkey.0.lock().unwrap().clone();
                match pubkey_str {
                    Some(s) => {
                        let bytes = s.into_bytes();
                        let _ = writer.write_all(&(bytes.len() as u32).to_le_bytes()).await;
                        let _ = writer.write_all(&bytes).await;
                    }
                    None => {
                        let _ = writer.write_all(&0u32.to_le_bytes()).await;
                    }
                }
                return;
            }

            // Transaction signing request.
            let tx_len = u32::from_le_bytes(len_buf) as usize;
            if tx_len == 0 || tx_len > 4 * 1024 * 1024 {
                let _ = writer.write_all(&0xFFFF_FFFFu32.to_le_bytes()).await;
                return;
            }
            let mut tx_bytes = vec![0u8; tx_len];
            if reader.read_exact(&mut tx_bytes).await.is_err() {
                let _ = writer.write_all(&0xFFFF_FFFFu32.to_le_bytes()).await;
                return;
            }

            // Store the pending tx and show the wallet popup.
            let (tx, rx) = oneshot::channel::<Result<Vec<u8>, String>>();
            {
                let mut lock = pending.lock().unwrap();
                *lock = Some((tx_bytes, tx));
            }

            println!("[SIGN-SRV] Pending signing request — showing signing window");
            show_signing_window(&app);

            // Wait for the React page to POST the signed bytes (60 s timeout).
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(60),
                rx,
            )
            .await;

            match result {
                Ok(Ok(Ok(signed))) => {
                    let _ = writer.write_all(&(signed.len() as u32).to_le_bytes()).await;
                    let _ = writer.write_all(&signed).await;
                }
                Ok(Ok(Err(e))) => {
                    eprintln!("[SIGN-SRV] Signing rejected: {}", e);
                    let _ = writer.write_all(&0xFFFF_FFFFu32.to_le_bytes()).await;
                }
                _ => {
                    eprintln!("[SIGN-SRV] Signing timed out or channel dropped");
                    let _ = writer.write_all(&0xFFFF_FFFFu32.to_le_bytes()).await;
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------


/// Path to the built wallet-ui SPA.
fn dist_path() -> PathBuf {
    let self_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    if let Some(ref dir) = self_dir {
        let candidate = dir.join("wallet-ui").join("dist");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("tauri/wallet-ui/dist")
}

/// Path to the built web-solana site.
fn site_path() -> PathBuf {
    let self_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    if let Some(ref dir) = self_dir {
        let candidate = dir.join("web-solana").join("dist");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("web-solana/dist")
}

fn spawn_bevy_game(
    wallet_mode: bool, 
    hot_wallet_pubkey: Option<String>, 
    is_hot: bool, 
    username: Option<String>,
    ai_difficulty: Option<u8>,
    ai_side: Option<String>,
    token: Option<String>,
) -> Result<std::process::Child, String> {
    let exe_name = if cfg!(windows) { "xfchess.exe" } else { "xfchess" };

    let self_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let resource_dir = self_dir.as_ref().map(|dir| {
        if dir.ends_with("bin") {
            dir.parent().map(|parent| parent.to_path_buf()).unwrap_or_else(|| dir.clone())
        } else {
            dir.clone()
        }
    });

    let candidate_paths = [
        self_dir.as_ref().map(|d| d.join(exe_name)),
        resource_dir.as_ref().map(|d| d.join(exe_name)),
        Some(PathBuf::from(format!("target/debug/{}", exe_name))),
        Some(PathBuf::from(format!("target/release/{}", exe_name))),
    ];

    let game_path = candidate_paths
        .iter()
        .flatten()
        .find(|p| p.exists())
        .ok_or_else(|| format!("Could not find {} binary", exe_name))?
        .clone();

    let mut cmd = Command::new(&game_path);
    if wallet_mode {
        cmd.env("XFCHESS_WALLET_MODE", "tauri");
    }
    if let Some(ref pk) = hot_wallet_pubkey {
        cmd.env("XFCHESS_WALLET_PUBKEY", pk);
        if is_hot {
            cmd.env("XFCHESS_HOT_WALLET", pk);
        }
    }
    if let Some(ref u) = username {
        cmd.env("XFCHESS_USERNAME", u);
    }
    if let Some(d) = ai_difficulty {
        cmd.env("XFCHESS_AI_DIFFICULTY", d.to_string());
    }
    if let Some(ref s) = ai_side {
        cmd.env("XFCHESS_AI_SIDE", s);
    }
    if let Some(ref t) = token {
        cmd.env("XFCHESS_AUTH_TOKEN", t);
    }
    if let Ok(signing_url) = std::env::var("SIGNING_SERVICE_URL") {
        cmd.env("SIGNING_SERVICE_URL", signing_url);
    }
    if let Ok(backend_url) = std::env::var("BACKEND_URL") {
        cmd.env("BACKEND_URL", backend_url);
    }
    if let Ok(wallet_port) = std::env::var("XFCHESS_WALLET_PORT") {
        cmd.env("XFCHESS_WALLET_PORT", wallet_port);
    }
    // Set working directory to project root so the game can find assets
    if let Some(parent) = game_path.parent() {
        let is_target = parent.ends_with("target") 
                     || parent.ends_with("target\\debug") 
                     || parent.ends_with("target\\release")
                     || parent.ends_with("target/debug")
                     || parent.ends_with("target/release");
        
        if is_target {
            // If in target/debug or target/release, climb up to reach the root
            let mut root = parent.to_path_buf();
            while root.ends_with("debug") || root.ends_with("release") || root.ends_with("target") {
                if let Some(p) = root.parent() {
                    root = p.to_path_buf();
                } else {
                    break;
                }
            }
            println!("[TAURI] Setting game CWD to project root: {:?}", root);
            // Bevy 0.18 uses CARGO_MANIFEST_DIR / BEVY_ASSET_ROOT to locate assets
            // when not launched via `cargo run`. Set both to the project root.
            cmd.env("CARGO_MANIFEST_DIR", &root);
            cmd.env("BEVY_ASSET_ROOT", &root);
            cmd.current_dir(&root);
        } else {
            println!("[TAURI] Setting game CWD to binary parent: {:?}", parent);
            cmd.env("CARGO_MANIFEST_DIR", parent);
            cmd.env("BEVY_ASSET_ROOT", parent);
            cmd.current_dir(parent);
        }
    }
    // Inherit stdout/stderr so game logs appear in Tauri console
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());
    println!("[TAURI] Launching game from: {:?}", game_path);
    cmd.spawn().map_err(|e| format!("Failed to launch game: {}", e))
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_wallet_status() -> serde_json::Value {
    serde_json::json!({ "connected": false, "pubkey": null })
}

#[tauri::command]
async fn wallet_connect() -> Result<String, String> {
    Ok("open_browser".to_string())
}

#[tauri::command]
async fn resolve_signed_tx(_signed_b64: String) -> Result<(), String> {
    Ok(())
}

/// Proxy login/register to the embedded auth server.
#[tauri::command]
async fn login_user(email: String, password: String) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/auth/login", get_backend_url());
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.status().is_success() {
        resp.json::<serde_json::Value>().await.map_err(|e| e.to_string())
    } else {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        Err(format!("HTTP {}: {}", status, text))
    }
}

/// Proxy register to the embedded auth server.
#[tauri::command]
async fn register_user(
    username: String,
    email: String,
    password: String,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let base = get_backend_url();
    // Register
    let reg = client
        .post(format!("{}/api/auth/register", base))
        .json(&serde_json::json!({ "username": username, "email": email, "password": password }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !reg.status().is_success() {
        let text = reg.text().await.unwrap_or_default();
        return Err(text);
    }
    // Auto-login after registration
    let login = client
        .post(format!("{}/api/auth/login", base))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    login.json::<serde_json::Value>().await.map_err(|e| e.to_string())
}

/// Save GDPR consent timestamp.
#[tauri::command]
fn save_consent(version: u8) -> Result<(), String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let record = serde_json::json!({ "version": version, "accepted_at": ts });
    let path = consent_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, record.to_string()).map_err(|e| e.to_string())
}

/// Load existing consent record (returns null if none).
#[tauri::command]
fn load_consent() -> serde_json::Value {
    let path = consent_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .unwrap_or(serde_json::Value::Null)
}

/// Launch the Bevy game and open browser for wallet connection (Tauri window stays hidden)
#[tauri::command]
async fn launch_game(
    app: tauri::AppHandle,
    wallet_pubkey_state: tauri::State<'_, WalletPubkey>,
    pubkey: Option<String>,
) -> Result<(), String> {
    // Store wallet pubkey if provided
    if let Some(ref pk) = pubkey {
        let mut lock = wallet_pubkey_state.0.lock().unwrap();
        *lock = Some(pk.clone());
    }

    // Spawn Bevy game
    match spawn_bevy_game(true, pubkey, false, None, None, None, None) {
        Ok(mut child) => {
            let pid = child.id();
            println!("[TAURI] Bevy game launched (PID {})", pid);
            std::thread::spawn(move || {
                let _ = child.wait();
                println!("[TAURI] Game exited — shutting down");
                std::process::exit(0);
            });
        }
        Err(e) => return Err(format!("Failed to launch game: {}", e)),
    }

    // Hide the Tauri window after game launches
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }

    Ok(())
}

/// Explicitly hide the window (called from React after signing)
#[tauri::command]
fn hide_main_window_cmd(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn consent_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xfchess")
        .join("consent.json")
}

// ---------------------------------------------------------------------------
// Embedded Signing Server (VPS)
// ---------------------------------------------------------------------------

async fn start_embedded_signing_server() {
    dotenvy::dotenv().ok();
    
    let config = SigningConfig::from_env();
    let port = config.port;
    
    let pool = match SqlitePool::connect("sqlite://sessions.db?mode=rwc").await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[SIGN-SRV] Failed to connect to SQLite: {}", e);
            return;
        }
    };
    
    let vault_pool = match SqlitePool::connect("sqlite://vault.db?mode=rwc").await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[SIGN-SRV] Failed to connect to vault DB: {}", e);
            return;
        }
    };
    
    let tournament_store = Arc::new(TournamentStore::new(pool.clone()).await);
    let state = SigningAppState::new(config, pool.clone(), vault_pool, tournament_store);
    if let Err(e) = state.store.init().await {
        eprintln!("[SIGN-SRV] Failed to init session store: {}", e);
        return;
    }
    
    let app = build_signing_router(state);
    let addr = format!("0.0.0.0:{}", port);
    
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[SIGN-SRV] Failed to bind to {}: {}", addr, e);
            return;
        }
    };
    
    println!("[SIGN-SRV] VPS signing server listening on http://{}", addr);
    
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("[SIGN-SRV] Server error: {}", e);
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    // Register xfchess:// protocol on Windows (no admin prompt needed for HKCU)
    register_protocol();
    
    let pending: PendingTx = Arc::new(Mutex::new(None));
    let wallet_pubkey = WalletPubkey::default();

    let pending_for_setup = pending.clone();
    let pubkey_for_setup = wallet_pubkey.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .manage(wallet_pubkey.clone())
        .invoke_handler(tauri::generate_handler![
            get_wallet_status,
            wallet_connect,
            resolve_signed_tx,
            login_user,
            register_user,
            save_consent,
            load_consent,
            launch_game,
            hide_main_window_cmd,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // Handle deep-link protocol events (xfchess://launch?pubkey=X&username=Y)
            let handle_clone = handle.clone();
            let wallet_clone = pubkey_for_setup.clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    println!("[DEEP-LINK] Received URL: {}", url);
                    if url.as_str().starts_with("xfchess://launch") {
                        println!("[DEEP-LINK] Launching game from protocol...");
                        
                        // Parse query parameters from the URL
                        let url_str = url.as_str();
                        let query = url_str.split('?').nth(1).unwrap_or("");
                        let params: std::collections::HashMap<&str, &str> = query
                            .split('&')
                            .filter_map(|pair| {
                                let mut parts = pair.split('=');
                                let key = parts.next()?;
                                let value = parts.next().unwrap_or("");
                                Some((key, value))
                            })
                            .collect();
                        
                        let pubkey = params.get("pubkey").map(|s| s.to_string())
                            .or_else(|| wallet_clone.0.lock().unwrap().clone());
                        let username = params.get("username").map(|s| s.to_string());
                        let ai_difficulty = params.get("ai_difficulty")
                            .and_then(|s| s.parse::<u8>().ok());
                        let ai_side = params.get("ai_side").map(|s| s.to_string());
                        let token = params.get("token").map(|s| s.to_string());
                        
                        // Update wallet pubkey state if provided in URL
                        if let Some(ref pk) = pubkey {
                            let mut lock = wallet_clone.0.lock().unwrap();
                            *lock = Some(pk.clone());
                        }
                        
                        match spawn_bevy_game(true, pubkey, false, username, ai_difficulty, ai_side, token) {
                            Ok(mut child) => {
                                let pid = child.id();
                                println!("[TAURI] Bevy game launched (PID {}) from deep-link", pid);
                                std::thread::spawn(move || {
                                    let _ = child.wait();
                                    println!("[TAURI] Game exited");
                                });
                            }
                            Err(e) => {
                                eprintln!("[DEEP-LINK] Failed to launch game: {}", e);
                            }
                        }
                    }
                }
            });

            // Show the Tauri window initially, then hide after Chrome opens successfully
            // This prevents invisible crashes if Chrome fails to launch
            let handle_for_chrome = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(2000));
                if let Some(w) = handle_for_chrome.get_webview_window("main") {
                    let _ = w.hide();
                }
            });

            // Start HTTP wallet server.
            let p = pending_for_setup.clone();
            let w = pubkey_for_setup.clone();
            let h1 = handle.clone();
            tauri::async_runtime::spawn(http_server(h1, p, w));

            // Start TCP signing bridge.
            let p2 = pending_for_setup.clone();
            let w2 = pubkey_for_setup.clone();
            let h2 = handle.clone();
            tauri::async_runtime::spawn(tcp_signing_server(h2, p2, w2));

            // NOTE: Local embedded server disabled - using Hetzner backend at 178.104.55.19:8090
            // tauri::async_runtime::spawn(start_embedded_signing_server());

            // Open Chrome to the production site after server starts
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(800));
                println!("[TAURI] Opening site: {}", get_backend_url());
                open_production_site();
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .map_err(|e| {
            eprintln!("[FATAL] Tauri application error: {}", e);
            eprintln!("[FATAL] Press any key to exit...");
            let _ = std::io::Read::read(&mut std::io::stdin(), &mut [0u8]);
            std::process::exit(1);
        }).ok();
}
