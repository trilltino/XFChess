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

const SIGN_TIMEOUT_SECS: u64 = 60;

// `wallet_bridge_port()` itself lives in `multiplayer::network::vps::client`
// (pure local port-file/env-var lookup, no Solana SDK dependency) so it stays
// reachable from callers outside the `solana`-feature-gated module tree, e.g.
// main_menu.rs's wallet-bridge status poller.
use crate::multiplayer::network::vps::wallet_bridge_port;

pub fn wallet_bridge_base_url() -> String {
    format!("http://127.0.0.1:{}", wallet_bridge_port())
}

pub fn open_profile_step() {
    bring_wallet_popup_to_front();
    std::thread::spawn(|| {
        let url = format!("{}/api/open-profile-step", wallet_bridge_base_url());
        match reqwest::blocking::Client::new().post(&url).send() {
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

fn sync_backend_url_to_bridge() {
    let url = crate::multiplayer::network::vps::vps_base();
    let bridge_url = format!("{}/api/set-backend-url", wallet_bridge_base_url());
    if let Err(e) = reqwest::blocking::Client::new()
        .post(&bridge_url)
        .json(&serde_json::json!({ "url": url }))
        .send()
    {
        bevy::prelude::warn!(
            "[sync_backend_url_to_bridge] could not reach this instance's Tauri bridge at \
             {bridge_url}: {e} — the wallet popup may end up talking to the wrong backend"
        );
    }
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
    bring_wallet_popup_to_front();
    std::thread::spawn(sync_backend_url_to_bridge);
    std::thread::spawn(|| {
        // Send OPEN command over TCP to the Tauri wallet bridge.
        use std::io::Write;
        use std::net::TcpStream;
        for port in candidate_ports() {
            if let Ok(mut s) = TcpStream::connect(format!("127.0.0.1:{}", port)) {
                let _ = s.write_all(b"OPEN");
                break;
            }
        }
    });
}

#[cfg(windows)]
fn bring_wallet_popup_to_front() {
    let expected_title = format!("XFChess #{}", wallet_bridge_port());
    std::thread::spawn(move || {
        use windows::core::BOOL;
        use windows::Win32::Foundation::{HWND, LPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextW, SetForegroundWindow, ShowWindow, SW_SHOW,
        };

        extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            unsafe {
                let ctx = &mut *(lparam.0 as *mut (String, HWND));
                let mut buf = [0u16; 256];
                let len = GetWindowTextW(hwnd, &mut buf);
                if len <= 0 || String::from_utf16_lossy(&buf[..len as usize]) != ctx.0 {
                    return BOOL(1);
                }
                ctx.1 = hwnd;
                BOOL(0)
            }
        }

        // Poll for ~3s — a fresh Chrome/Edge process needs a moment to
        // create its window and stamp its title.
        for _ in 0..60 {
            let mut ctx: (String, HWND) = (expected_title.clone(), HWND(std::ptr::null_mut()));
            unsafe {
                let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut _ as isize));
            }
            if !ctx.1 .0.is_null() {
                unsafe {
                    let _ = ShowWindow(ctx.1, SW_SHOW);
                    let _ = SetForegroundWindow(ctx.1);
                }
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });
}

#[cfg(not(windows))]
fn bring_wallet_popup_to_front() {}

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

    // Add local signers first (if any)
    for keypair in local_signers {
        tx.try_sign(&[*keypair], blockhash)
            .map_err(|e| format!("local_sign: {}", e))?;
    }

    // Partially sign with wallet as NullSigner placeholder
    tx.try_partial_sign(&[&NullSigner::new(&wallet_pubkey)], blockhash)
        .map_err(|e| format!("partial_sign: {}", e))?;

    let tx_bytes = bincode::serialize(&tx).map_err(|e| format!("serialize_tx: {}", e))?;
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

    let signed_bytes = send_to_tauri_blocking(&tx_bytes, label)?;

    submit_signed_to_rpc(rpc_url, &signed_bytes)
}

pub fn sign_message_via_tauri(message: &str, label: &str) -> Result<Vec<u8>, String> {
    info!("[TAURI-SIGN] Requesting message signature: '{}'", message);
    send_to_tauri_blocking(message.as_bytes(), label)
}

const MAX_RESP_LEN: u32 = 64 * 1024;

fn send_to_tauri_blocking(tx_bytes: &[u8], label: &str) -> Result<Vec<u8>, String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    // Every signing request needs the popup raised, not just the initial
    // Connect Wallet click — see `bring_wallet_popup_to_front`'s doc comment
    // for why this has to be issued from this process, not the Tauri sidecar.
    bring_wallet_popup_to_front();

    fn is_timeout(e: &std::io::Error) -> bool {
        matches!(
            e.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        )
    }

    let write_timeout = Duration::from_secs(5);
    let read_timeout = Duration::from_secs(SIGN_TIMEOUT_SECS);

    // Server rejects anything longer (`MAX_LABEL_LEN` in tauri/src/main.rs) —
    // truncate defensively rather than let a long label sink the whole request.
    let label_bytes = &label.as_bytes()[..label.len().min(256)];

    let mut last_err: Option<String> = None;
    let start = Instant::now();

    for port in candidate_ports() {
        let mut stream = match TcpStream::connect(format!("127.0.0.1:{}", port)) {
            Ok(s) => s,
            Err(_) => continue,
        };
        info!("[TAURI-SIGN] connected to wallet bridge port {port} for '{label}'");

        let _ = stream.set_write_timeout(Some(write_timeout));
        let _ = stream.set_read_timeout(Some(read_timeout));

        let label_len = label_bytes.len() as u32;
        let len = tx_bytes.len() as u32;
        let write_ok = stream.write_all(&label_len.to_le_bytes()).is_ok()
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
            last_err = Some(format!("port {port} closed before sending a length prefix"));
            continue;
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
