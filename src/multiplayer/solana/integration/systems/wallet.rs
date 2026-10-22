use super::*;

pub fn query_wallet_pubkey_from_tauri() -> Option<String> {
    use crate::multiplayer::solana::tauri_signer::candidate_ports;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    // Prefer the announced bridge port; scan only as a fallback through the shared helper.
    for port in candidate_ports() {
        let mut stream = match TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => s,
            Err(_) => continue,
        };
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .ok();
        let _ = stream.write_all(b"PKEY");
        let mut len_buf = [0u8; 4];
        if stream.read_exact(&mut len_buf).is_err() {
            return None;
        }
        // A base58 pubkey is at most 44 characters; refuse to allocate for anything
        // implausible from whatever is listening on the port.
        const MAX_PUBKEY_LEN: usize = 64;
        let len = u32::from_le_bytes(len_buf) as usize;
        if len == 0 || len > MAX_PUBKEY_LEN {
            return None;
        }
        let mut buf = vec![0u8; len];
        if stream.read_exact(&mut buf).is_err() {
            return None;
        }
        return String::from_utf8(buf).ok();
    }
    None
}

pub fn update_wallet_balance(
    mut solana_state: ResMut<SolanaIntegrationState>,
    mut timer: ResMut<BalanceRefreshTimer>,
    time: Res<Time>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
    mut rx: Local<Option<crossbeam_channel::Receiver<Result<u64, String>>>>,
    mut last_pubkey: Local<Option<Pubkey>>,
) {
    if let Some(ref receiver) = *rx {
        match receiver.try_recv() {
            Ok(Ok(lamports)) => {
                let sol = lamports as f64 / 1_000_000_000.0;
                solana_state.balance = sol;
                if let Some(rate) = solana_state.sol_usd_rate {
                    solana_state.cached_usd_balance = Some(sol * rate);
                }
                *rx = None;
            }
            Ok(Err(e)) => {
                warn!("[SOLANA] Balance fetch failed: {}", e);
                *rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                *rx = None;
            }
        }
        return;
    }

    // Fetch balance immediately on connection so wager eligibility does not
    // wait for the periodic timer's first tick.
    let just_connected = solana_state.wallet_pubkey != *last_pubkey;
    *last_pubkey = solana_state.wallet_pubkey;

    if !just_connected && !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    if just_connected {
        timer.0.reset();
    }
    let (Some(pubkey), Some(rpc_url)) = (
        solana_state.wallet_pubkey,
        solana_state.rpc_client.as_ref().map(|c| c.url()),
    ) else {
        return;
    };

    let (tx, receiver) = crossbeam_channel::bounded(1);
    *rx = Some(receiver);
    tokio_runtime.0.spawn_blocking(move || {
        let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
        let _ = tx.send(rpc.get_balance(&pubkey).map_err(|e| e.to_string()));
    });
}

pub fn update_wallet_usd_rate(
    mut solana_state: ResMut<SolanaIntegrationState>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
    time: Res<Time>,
    mut timer: Local<f32>,
    mut rx: Local<Option<crossbeam_channel::Receiver<Result<f64, String>>>>,
) {
    *timer -= time.delta_secs();

    if let Some(ref receiver) = *rx {
        match receiver.try_recv() {
            Ok(Ok(rate)) => {
                solana_state.sol_usd_rate = Some(rate);
                solana_state.cached_usd_balance = Some(solana_state.balance * rate);
                // No per-refresh log — see `wager_rate.rs`'s matching comment.
                *rx = None;
            }
            Ok(Err(e)) => {
                warn!("[SOLANA] USD rate fetch failed: {}", e);
                *rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                *rx = None;
            }
        }
        return;
    }

    if *timer > 0.0 {
        return;
    }
    *timer = 60.0;

    if solana_state.wallet_pubkey.is_none() {
        return;
    }

    let (tx, receiver) = crossbeam_channel::bounded(1);
    *rx = Some(receiver);

    tokio_runtime.0.spawn(async move {
        let _ = tx.send(fetch_sol_usd_rate().await);
    });
}

pub(super) async fn fetch_sol_usd_rate() -> Result<f64, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| format!("HTTP client build error: {}", e))?;

    let url = format!(
        "{}/api/rates/all",
        crate::multiplayer::network::vps::vps_base()
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("backend rates fetch error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("backend rates HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("backend rates JSON parse error: {}", e))?;

    let price = json["rates"]["usd"]
        .as_f64()
        .ok_or("Missing rates.usd in backend response")?;

    Ok(price)
}

pub(super) fn get_hot_wallet_path() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        crate::core::android_paths::internal_data_dir().map(|dir| {
            hot_wallet_filename(&dir, std::env::var("XFCHESS_WALLET_PORT").ok().as_deref())
        })
    }
    #[cfg(not(target_os = "android"))]
    {
        ProjectDirs::from("com", "trilltino", "XFChess").map(|proj_dirs| {
            hot_wallet_filename(
                proj_dirs.config_dir(),
                std::env::var("XFCHESS_WALLET_PORT").ok().as_deref(),
            )
        })
    }
}

pub(super) fn hot_wallet_filename(
    config_dir: &std::path::Path,
    wallet_port: Option<&str>,
) -> PathBuf {
    match wallet_port.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) if p != "7454" => config_dir.join(format!("hot_wallet_{p}.json")),
        _ => config_dir.join("hot_wallet.json"),
    }
}

pub(super) fn load_or_create_hot_wallet() -> Result<Keypair, String> {
    let path = get_hot_wallet_path().ok_or("Wallet configuration directory unavailable")?;
    hot_wallet_at(&path).map_err(|error| format!("Hot wallet {}: {error}", path.display()))
}

fn read_hot_wallet(path: &std::path::Path) -> Result<Keypair, String> {
    use std::io::Read as _;

    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(4097)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > 4096 {
        return Err("Key file exceeds 4096 bytes".into());
    }
    let contents = std::str::from_utf8(&bytes).map_err(|error| error.to_string())?;
    let decoded = match serde_json::from_str::<Vec<u8>>(contents) {
        Ok(bytes) => bytes,
        Err(_) => bs58::decode(contents.trim())
            .into_vec()
            .map_err(|error| format!("Invalid key encoding: {error}"))?,
    };
    Keypair::try_from(decoded.as_slice()).map_err(|error| format!("Invalid keypair: {error}"))
}

fn hot_wallet_at(path: &std::path::Path) -> Result<Keypair, String> {
    use std::io::Write as _;

    match fs::metadata(path) {
        Ok(_) => return read_hot_wallet(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return read_hot_wallet(path)
        }
        Err(error) => return Err(error.to_string()),
    };
    let keypair = Keypair::new();
    let json =
        serde_json::to_vec(keypair.to_bytes().as_slice()).map_err(|error| error.to_string())?;
    file.write_all(&json).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    Ok(keypair)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hot_wallet_creation_and_reloads_preserve_identity() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!(
            "xfchess-hot-wallet-{}.json",
            Keypair::new().pubkey()
        ));
        let first = hot_wallet_at(&path).map_err(anyhow::Error::msg)?;
        assert_eq!(
            first.pubkey(),
            hot_wallet_at(&path).map_err(anyhow::Error::msg)?.pubkey()
        );
        fs::write(&path, bs58::encode(first.to_bytes()).into_string())?;
        assert_eq!(
            first.pubkey(),
            hot_wallet_at(&path).map_err(anyhow::Error::msg)?.pubkey()
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn invalid_hot_wallet_is_never_replaced() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!(
            "xfchess-bad-wallet-{}.json",
            Keypair::new().pubkey()
        ));
        for invalid in ["invalid key", "[1, 2, 3]"] {
            fs::write(&path, invalid)?;
            assert!(hot_wallet_at(&path).is_err());
            assert_eq!(fs::read_to_string(&path)?, invalid);
        }
        fs::remove_file(path)?;
        Ok(())
    }
}
