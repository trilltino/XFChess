use bevy::prelude::info;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    instruction::Instruction,
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::{Keypair, NullSigner, Signature, Signer},
    transaction::{Transaction, VersionedTransaction},
};

const SIGN_TIMEOUT_SECS: u64 = 120;

// wallet_bridge_port lives outside the solana feature gate so UI status polling can use it.
use crate::multiplayer::network::vps::wallet_bridge_port;

pub fn wallet_bridge_base_url() -> String {
    format!("http://127.0.0.1:{}", wallet_bridge_port())
}

pub fn open_profile_step() {
    crate::core::net_task::spawn_detached("profile-step", || {
        if let Err(error) = configure_wallet_bridge() {
            bevy::prelude::warn!(%error, "Wallet bridge configuration failed");
            return;
        }
        let url = format!("{}/api/open-profile-step", wallet_bridge_base_url());
        match bridge_http_client().post(&url).send() {
            Ok(resp) if resp.status().is_success() => {}
            Ok(resp) => bevy::prelude::warn!(
                "[open_profile_step] bridge at {url} responded {} — popup will not open",
                resp.status()
            ),
            Err(e) => bevy::prelude::warn!(
                "[open_profile_step] could not reach this instance's Tauri bridge at {url}: {e} — \
                 popup will not open. If running two local instances, confirm this one's \
                 XFCHESS_WALLET_PORT was actually set before xfchess-tauri started (a mismatch \
                 leaves that bridge silently running with no HTTP server bound)."
            ),
        }
    });
}

fn configure_wallet_bridge() -> Result<(), String> {
    let client = bridge_http_client();
    let response = client
        .get(format!("{}/status", wallet_bridge_base_url()))
        .header("X-Game-Pid", std::process::id())
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| error.to_string())?;
    #[cfg(windows)]
    if let Ok(status) = response.json::<serde_json::Value>() {
        if let Some(pid) = status["bridge_pid"]
            .as_u64()
            .and_then(|pid| u32::try_from(pid).ok())
        {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(pid);
            }
        }
    }
    #[cfg(not(windows))]
    drop(response);
    client
        .post(format!("{}/api/set-backend-url", wallet_bridge_base_url()))
        .json(&serde_json::json!({ "url": crate::multiplayer::network::vps::vps_base() }))
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn tcp_port_range() -> std::ops::RangeInclusive<u16> {
    let base: u16 = wallet_bridge_port();
    base.saturating_sub(11)..=base.saturating_sub(2)
}

fn wallet_bridge_port_file() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "xfchess-wallet-bridge-{}.port",
        wallet_bridge_port()
    ))
}

fn discovered_bridge_port() -> Option<u16> {
    std::fs::read_to_string(wallet_bridge_port_file())
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .filter(|port| tcp_port_range().contains(port))
}

pub fn candidate_ports() -> Vec<u16> {
    let mut ports = Vec::with_capacity(11);
    if let Some(p) = discovered_bridge_port() {
        ports.push(p);
    }
    for p in tcp_port_range() {
        if !ports.contains(&p) {
            ports.push(p);
        }
    }
    ports
}

pub fn open_wallet_browser() {
    crate::core::net_task::spawn_detached("open-wallet", || {
        if let Err(error) = configure_wallet_bridge() {
            bevy::prelude::warn!(%error, "Wallet bridge configuration failed");
            return;
        }
        let client = bridge_http_client();
        let _ = client
            .post(format!("{}/api/wallet/open", wallet_bridge_base_url()))
            .send();
    });
}

fn bridge_http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("wallet bridge HTTP client")
}

fn wallet_signable_blockhash(rpc: &RpcClient) -> Result<solana_sdk::hash::Hash, String> {
    rpc.get_latest_blockhash_with_commitment(CommitmentConfig::finalized())
        .map(|(hash, _last_valid_block_height)| hash)
        .map_err(|e| format!("get_latest_blockhash: {}", e))
}

pub fn sign_via_tauri_only(
    rpc_url: &str,
    wallet_pubkey: Pubkey,
    instructions: &[Instruction],
    local_signers: &[&Keypair],
    label: &str,
) -> Result<Vec<u8>, String> {
    use std::time::Instant;

    let start = Instant::now();
    let rpc = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    let step_start = Instant::now();
    let blockhash = wallet_signable_blockhash(&rpc)?;
    info!(
        "[TAURI-SIGN] latest blockhash fetched for '{label}' in {:?}",
        step_start.elapsed()
    );

    // Use legacy Transaction to match wallet UI
    let mut tx = Transaction::new_with_payer(instructions, Some(&wallet_pubkey));

    for keypair in local_signers {
        tx.try_partial_sign(&[*keypair], blockhash)
            .map_err(|e| format!("local_sign: {}", e))?;
    }

    tx.try_partial_sign(&[&NullSigner::new(&wallet_pubkey)], blockhash)
        .map_err(|e| format!("partial_sign: {}", e))?;

    let tx_bytes = bincode::serialize(&tx).map_err(|e| format!("serialize_tx: {}", e))?;
    simulate_before_signing(&rpc, &VersionedTransaction::from(tx))?;
    let step_start = Instant::now();
    let signed = send_to_tauri_blocking(&tx_bytes, label)?;
    info!(
        "[TAURI-SIGN] wallet bridge returned '{label}' in {:?} (total {:?})",
        step_start.elapsed(),
        start.elapsed()
    );
    Ok(signed)
}

pub fn sign_and_send_via_tauri(
    rpc_url: &str,
    wallet_pubkey: Pubkey,
    instructions: &[Instruction],
    local_signers: &[&Keypair],
    label: &str,
) -> Result<Signature, String> {
    let rpc = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let step_start = std::time::Instant::now();
    let blockhash = wallet_signable_blockhash(&rpc)?;
    info!(
        "[TAURI-SIGN] latest blockhash fetched for '{label}' in {:?}",
        step_start.elapsed()
    );

    let message = v0::Message::try_compile(&wallet_pubkey, instructions, &[], blockhash)
        .map_err(|e| format!("compile_message: {}", e))?;

    // NullSigner produces a zero-signature placeholder for the wallet (fee-payer).
    // Phantom replaces it with the real signature via the Tauri signing bridge.
    let wallet_null = NullSigner::new(&wallet_pubkey);
    let mut dyn_signers: Vec<&dyn Signer> = vec![&wallet_null as &dyn Signer];
    for k in local_signers {
        dyn_signers.push(*k as &dyn Signer);
    }

    let tx = VersionedTransaction::try_new(VersionedMessage::V0(message), dyn_signers.as_slice())
        .map_err(|e| format!("build_tx: {}", e))?;

    let tx_bytes = bincode::serialize(&tx).map_err(|e| format!("serialize_tx: {}", e))?;

    simulate_before_signing(&rpc, &tx)?;

    let step_start = std::time::Instant::now();
    let signed_bytes = send_to_tauri_blocking(&tx_bytes, label)?;
    info!(
        "[TAURI-SIGN] wallet bridge returned '{label}' in {:?}",
        step_start.elapsed()
    );

    submit_signed_to_rpc(rpc_url, &signed_bytes)
}

pub fn sign_and_send_b64_via_tauri(
    rpc_url: &str,
    tx_b64: &str,
    label: &str,
) -> Result<Signature, String> {
    use base64::{engine::general_purpose, Engine as _};

    let tx_bytes = general_purpose::STANDARD
        .decode(tx_b64)
        .map_err(|e| format!("decode_b64: {}", e))?;

    let tx: VersionedTransaction =
        bincode::deserialize(&tx_bytes).map_err(|e| format!("deserialize_tx: {e}"))?;
    let rpc = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    simulate_before_signing(&rpc, &tx)?;

    let signed_bytes = send_to_tauri_blocking(&tx_bytes, label)?;

    submit_signed_to_rpc(rpc_url, &signed_bytes)
}

pub fn sign_message_via_tauri(message: &str, label: &str) -> Result<Vec<u8>, String> {
    let status: serde_json::Value = bridge_http_client()
        .get(format!("{}/status", wallet_bridge_base_url()))
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;
    let wallet: Pubkey = status["pubkey"]
        .as_str()
        .ok_or("Wallet is disconnected")?
        .parse()
        .map_err(|e| format!("Invalid wallet: {e}"))?;
    let bytes = send_request_blocking(message.as_bytes(), label, true)?;
    let signature = Signature::try_from(bytes.as_slice())
        .map_err(|e| format!("Invalid message signature: {e}"))?;
    if !signature.verify(wallet.as_ref(), message.as_bytes()) {
        return Err("Wallet returned an invalid message signature".into());
    }
    Ok(bytes)
}

fn simulate_before_signing(rpc: &RpcClient, tx: &VersionedTransaction) -> Result<(), String> {
    let response = rpc
        .simulate_transaction_with_config(
            tx,
            solana_client::rpc_config::RpcSimulateTransactionConfig {
                sig_verify: false,
                commitment: Some(CommitmentConfig::confirmed()),
                ..Default::default()
            },
        )
        .map_err(|e| format!("Transaction simulation: {e}"))?;
    if let Some(error) = response.value.err {
        return Err(format!("Transaction simulation failed: {error}"));
    }
    Ok(())
}

fn validate_signed_transaction(original: &[u8], signed: &[u8]) -> Result<(), String> {
    let original: VersionedTransaction = bincode::deserialize(original)
        .map_err(|e| format!("Invalid requested transaction: {e}"))?;
    let signed: VersionedTransaction =
        bincode::deserialize(signed).map_err(|e| format!("Invalid wallet transaction: {e}"))?;
    if original.message != signed.message {
        return Err("Wallet changed the requested transaction".into());
    }
    signed
        .verify_and_hash_message()
        .map_err(|e| format!("Invalid wallet signatures: {e}"))?;
    if original.signatures.len() != signed.signatures.len()
        || original
            .signatures
            .iter()
            .zip(&signed.signatures)
            .any(|(before, after)| *before != Signature::default() && before != after)
    {
        return Err("Wallet replaced an existing signature".into());
    }
    Ok(())
}

const MAX_RESP_LEN: u32 = 64 * 1024;

fn send_to_tauri_blocking(tx_bytes: &[u8], label: &str) -> Result<Vec<u8>, String> {
    let signed = send_request_blocking(tx_bytes, label, false)?;
    validate_signed_transaction(tx_bytes, &signed)?;
    Ok(signed)
}

fn send_request_blocking(tx_bytes: &[u8], label: &str, message: bool) -> Result<Vec<u8>, String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    fn is_timeout(e: &std::io::Error) -> bool {
        matches!(
            e.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        )
    }

    configure_wallet_bridge()?;
    let write_timeout = Duration::from_secs(5);
    let read_timeout = Duration::from_secs(SIGN_TIMEOUT_SECS + 5);

    // Server rejects anything longer (`MAX_LABEL_LEN` in tauri/src/main.rs) —
    // truncate defensively rather than let a long label sink the whole request.
    let label_bytes = &label.as_bytes()[..label.len().min(256)];

    let mut last_err: Option<String> = None;
    let start = Instant::now();

    for port in candidate_ports() {
        let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let mut probe = match TcpStream::connect_timeout(&address, Duration::from_millis(500)) {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        let _ = probe.set_read_timeout(Some(Duration::from_secs(1)));
        let _ = probe.set_write_timeout(Some(Duration::from_secs(1)));
        let mut identity = [0u8; 4];
        if probe.write_all(b"INFO").is_err()
            || probe.read_exact(&mut identity).is_err()
            || u32::from_le_bytes(identity) != u32::from(wallet_bridge_port())
        {
            continue;
        }
        let mut stream = match TcpStream::connect_timeout(&address, Duration::from_millis(500)) {
            Ok(s) => s,
            Err(_) => continue,
        };
        info!("[TAURI-SIGN] connected to wallet bridge port {port} for '{label}'");

        let _ = stream.set_write_timeout(Some(write_timeout));
        let _ = stream.set_read_timeout(Some(read_timeout));

        let label_len = label_bytes.len() as u32;
        let len = tx_bytes.len() as u32;
        let write_ok = (!message || stream.write_all(b"MSIG").is_ok())
            && stream.write_all(&label_len.to_le_bytes()).is_ok()
            && stream.write_all(label_bytes).is_ok()
            && stream.write_all(&len.to_le_bytes()).is_ok()
            && stream.write_all(tx_bytes).is_ok();
        if !write_ok {
            last_err = Some(format!("write to port {port} failed"));
            continue;
        }
        info!(
            "[TAURI-SIGN] sent '{}' request to bridge port {port} in {:?}; waiting for wallet",
            label,
            start.elapsed()
        );

        let mut len_buf = [0u8; 4];
        if let Err(e) = stream.read_exact(&mut len_buf) {
            if is_timeout(&e) {
                return Err(format!(
                    "Signing server closed connection before responding: {e}"
                ));
            }
            return Err(format!("Wallet bridge closed before responding: {e}"));
        }
        let resp_len = u32::from_le_bytes(len_buf);
        if resp_len == 0xFFFF_FFFF {
            return Err("Signing server rejected the transaction (user cancelled?)".to_string());
        }
        if resp_len > MAX_RESP_LEN {
            last_err = Some(format!(
                "port {port} sent implausible response length {resp_len}"
            ));
            continue;
        }

        let mut buf = vec![0u8; resp_len as usize];
        match stream.read_exact(&mut buf) {
            Ok(_) => {
                info!(
                    "[TAURI-SIGN] received '{}' response from bridge port {port} in {:?}",
                    label,
                    start.elapsed()
                );
                return Ok(buf);
            }
            Err(e) if is_timeout(&e) => return Err(format!("read_signed_bytes: {}", e)),
            Err(e) => {
                last_err = Some(format!("read_signed_bytes (port {port}): {}", e));
                continue;
            }
        }
    }

    let range = tcp_port_range();
    Err(last_err.unwrap_or_else(|| {
        format!(
            "Could not connect to Tauri signing server on ports {}-{}",
            range.start(),
            range.end()
        )
    }))
}

fn submit_signed_to_rpc(rpc_url: &str, signed_bytes: &[u8]) -> Result<Signature, String> {
    use super::submit::{submit_and_poll, SubmitConfig};

    let signed_tx: VersionedTransaction =
        bincode::deserialize(signed_bytes).map_err(|e| format!("deserialize_signed_tx: {}", e))?;

    let rpc = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    submit_and_poll(&rpc, &signed_tx, SubmitConfig::fast())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transaction_pair() -> (Vec<u8>, Vec<u8>) {
        let wallet = Keypair::new();
        let local = Keypair::new();
        let instruction = Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![solana_sdk::instruction::AccountMeta::new_readonly(
                local.pubkey(),
                true,
            )],
            data: vec![1],
        };
        let message = v0::Message::try_compile(
            &wallet.pubkey(),
            &[instruction],
            &[],
            solana_sdk::hash::Hash::new_unique(),
        )
        .unwrap();
        let original = VersionedTransaction::try_new(
            VersionedMessage::V0(message.clone()),
            &[
                &NullSigner::new(&wallet.pubkey()) as &dyn Signer,
                &local as &dyn Signer,
            ],
        )
        .unwrap();
        let signed =
            VersionedTransaction::try_new(VersionedMessage::V0(message), &[&wallet, &local])
                .unwrap();
        (
            bincode::serialize(&original).unwrap(),
            bincode::serialize(&signed).unwrap(),
        )
    }

    #[test]
    fn accepts_the_requested_message_with_all_signatures() {
        let (original, signed) = transaction_pair();
        assert!(validate_signed_transaction(&original, &signed).is_ok());
    }

    #[test]
    fn rejects_an_altered_transaction() {
        let (original, signed) = transaction_pair();
        let mut signed: VersionedTransaction = bincode::deserialize(&signed).unwrap();
        if let VersionedMessage::V0(message) = &mut signed.message {
            message.recent_blockhash = solana_sdk::hash::Hash::new_unique();
        }
        assert!(
            validate_signed_transaction(&original, &bincode::serialize(&signed).unwrap()).is_err()
        );
    }

    #[test]
    fn rejects_an_unsigned_wallet_response() {
        let (original, _) = transaction_pair();
        assert!(validate_signed_transaction(&original, &original).is_err());
    }
}
