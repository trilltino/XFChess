//! Tauri signing bridge for Phantom / Solflare wallet.
//!
//! Bevy builds unsigned [`VersionedTransaction`]s; this module forwards them
//! over the local TCP channel to the Tauri signing server, which prompts the
//! browser wallet, receives the signed bytes, and returns them here.
//! The fully-signed transaction is then submitted to the Solana RPC.
//!
//! # Wire protocol (per `tauri/src/main.rs`)
//! - Client → Server : `4-byte LE length` + `raw VersionedTransaction bytes`
//! - Server → Client : `4-byte LE length` + `signed VersionedTransaction bytes`
//!                      OR `0xFFFF_FFFF` on rejection / error

use bevy::prelude::info;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::{Keypair, NullSigner, Signature, Signer},
    transaction::{Transaction, VersionedTransaction},
};

/// Maximum seconds to wait for the user to approve the transaction in Phantom.
const SIGN_TIMEOUT_SECS: u64 = 60;

/// HTTP port for this instance's Tauri wallet bridge, from XFCHESS_WALLET_PORT (default 7454).
pub fn wallet_bridge_port() -> u16 {
    std::env::var("XFCHESS_WALLET_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7454)
}

/// Base HTTP URL for this instance's Tauri wallet bridge, e.g. `http://127.0.0.1:7454`.
/// Every call site that talks to the local Tauri sidecar must go through this so two
/// dev instances on different `XFCHESS_WALLET_PORT` values never cross-talk.
pub fn wallet_bridge_base_url() -> String {
    format!("http://127.0.0.1:{}", wallet_bridge_port())
}

/// Fire-and-forget POST to this instance's Tauri bridge asking it to open the
/// profile-creation step in the wallet popup.
pub fn open_profile_step() {
    std::thread::spawn(|| {
        let url = format!("{}/api/open-profile-step", wallet_bridge_base_url());
        let _ = reqwest::blocking::Client::new().post(url).send();
    });
}

/// TCP port range derived from XFCHESS_WALLET_PORT (default 7454).
/// The Tauri side binds TCP on (base-11)..=(base-2).
fn tcp_port_range() -> std::ops::RangeInclusive<u16> {
    let base: u16 = wallet_bridge_port();
    base.saturating_sub(11)..=base.saturating_sub(2)
}

/// Fire-and-forget: requests the Tauri wallet popup window for wallet connection.
/// Spawns a background thread so Bevy is never blocked.
pub fn open_wallet_browser() {
    std::thread::spawn(|| {
        // Send OPEN command over TCP to the Tauri wallet bridge.
        use std::io::Write;
        use std::net::TcpStream;
        for port in tcp_port_range() {
            if let Ok(mut s) = TcpStream::connect(format!("127.0.0.1:{}", port)) {
                let _ = s.write_all(b"OPEN");
                break;
            }
        }
    });
}

/// Like `sign_and_send_via_tauri` but returns the signed transaction bytes
/// without submitting to RPC. Used by the VPS flow where the VPS submits.
pub fn sign_via_tauri_only(
    rpc_url: &str,
    wallet_pubkey: Pubkey,
    instructions: &[Instruction],
    local_signers: &[&Keypair],
) -> Result<Vec<u8>, String> {
    let rpc = RpcClient::new(rpc_url.to_string());
    let blockhash = rpc
        .get_latest_blockhash()
        .map_err(|e| format!("get_latest_blockhash: {}", e))?;

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
    send_to_tauri_blocking(&tx_bytes)
}

/// Build a `VersionedTransaction` (v0), partially sign with `local_signers`
/// (e.g. a session keypair), send to the Tauri signing bridge for Phantom to
/// co-sign as fee-payer, then submit and confirm on-chain.
///
/// Pass an empty slice for `local_signers` when only the wallet needs to sign.
pub fn sign_and_send_via_tauri(
    rpc_url: &str,
    wallet_pubkey: Pubkey,
    instructions: &[Instruction],
    local_signers: &[&Keypair],
) -> Result<Signature, String> {
    let rpc = RpcClient::new(rpc_url.to_string());

    let blockhash = rpc
        .get_latest_blockhash()
        .map_err(|e| format!("get_latest_blockhash: {}", e))?;

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

    let signed_bytes = send_to_tauri_blocking(&tx_bytes)?;

    submit_signed_to_rpc(rpc_url, &signed_bytes)
}

/// Sign and send a pre-built base64-encoded transaction via the Tauri bridge.
pub fn sign_and_send_b64_via_tauri(rpc_url: &str, tx_b64: &str) -> Result<Signature, String> {
    use base64::{engine::general_purpose, Engine as _};

    let tx_bytes = general_purpose::STANDARD
        .decode(tx_b64)
        .map_err(|e| format!("decode_b64: {}", e))?;

    let signed_bytes = send_to_tauri_blocking(&tx_bytes)?;

    submit_signed_to_rpc(rpc_url, &signed_bytes)
}

/// Sign an arbitrary message (e.g. for TEE authentication) via the Tauri signing bridge.
pub fn sign_message_via_tauri(message: &str) -> Result<Vec<u8>, String> {
    info!("[TAURI-SIGN] Requesting message signature: '{}'", message);
    send_to_tauri_blocking(message.as_bytes())
}

/// Send raw transaction bytes to the Tauri signing server and block until the
/// signed bytes are returned or an error occurs.
fn send_to_tauri_blocking(tx_bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let write_timeout = Duration::from_secs(5);
    let read_timeout = Duration::from_secs(SIGN_TIMEOUT_SECS);

    for port in tcp_port_range() {
        let mut stream = match TcpStream::connect(format!("127.0.0.1:{}", port)) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let _ = stream.set_write_timeout(Some(write_timeout));
        let _ = stream.set_read_timeout(Some(read_timeout));

        let len = tx_bytes.len() as u32;
        if stream.write_all(&len.to_le_bytes()).is_err() || stream.write_all(tx_bytes).is_err() {
            continue;
        }

        let mut len_buf = [0u8; 4];
        if stream.read_exact(&mut len_buf).is_err() {
            return Err("Signing server closed connection before responding".to_string());
        }
        let resp_len = u32::from_le_bytes(len_buf);
        if resp_len == 0xFFFF_FFFF {
            return Err("Signing server rejected the transaction (user cancelled?)".to_string());
        }

        let mut buf = vec![0u8; resp_len as usize];
        return stream
            .read_exact(&mut buf)
            .map(|_| buf)
            .map_err(|e| format!("read_signed_bytes: {}", e));
    }

    let range = tcp_port_range();
    Err(format!(
        "Could not connect to Tauri signing server on ports {}-{}",
        range.start(),
        range.end()
    ))
}

/// Deserialise the wire-format signed bytes into a `VersionedTransaction` and
/// submit it to the Solana RPC, then block until confirmed.
fn submit_signed_to_rpc(rpc_url: &str, signed_bytes: &[u8]) -> Result<Signature, String> {
    let signed_tx: VersionedTransaction =
        bincode::deserialize(signed_bytes).map_err(|e| format!("deserialize_signed_tx: {}", e))?;

    let rpc = RpcClient::new(rpc_url.to_string());

    rpc.send_and_confirm_transaction(&signed_tx)
        .map_err(|e| format!("send_and_confirm: {}", e))
}
