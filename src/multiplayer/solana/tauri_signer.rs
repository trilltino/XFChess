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

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    signer::null_signer::NullSigner,
    transaction::VersionedTransaction,
};

/// Maximum seconds to wait for the user to approve the transaction in Phantom.
const SIGN_TIMEOUT_SECS: u64 = 60;

/// TCP port range derived from XFCHESS_WALLET_PORT (default 7454).
/// The Tauri side binds TCP on (base-11)..=(base-2).
fn tcp_port_range() -> std::ops::RangeInclusive<u16> {
    let base: u16 = std::env::var("XFCHESS_WALLET_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7454);
    base.saturating_sub(11)..=base.saturating_sub(2)
}

/// Fire-and-forget: tells Tauri to open `http://localhost:7454` in the default browser.
/// Spawns a background thread so Bevy is never blocked.
pub fn open_wallet_browser() {
    std::thread::spawn(|| {
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
    let message = v0::Message::try_compile(&wallet_pubkey, instructions, &[], blockhash)
        .map_err(|e| format!("compile_message: {}", e))?;
    let wallet_null = NullSigner::new(&wallet_pubkey);
    let mut dyn_signers: Vec<&dyn Signer> = vec![&wallet_null as &dyn Signer];
    for k in local_signers {
        dyn_signers.push(*k as &dyn Signer);
    }
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        dyn_signers.as_slice(),
    )
    .map_err(|e| format!("build_tx: {}", e))?;
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

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        dyn_signers.as_slice(),
    )
    .map_err(|e| format!("build_tx: {}", e))?;

    let tx_bytes =
        bincode::serialize(&tx).map_err(|e| format!("serialize_tx: {}", e))?;

    let signed_bytes = send_to_tauri_blocking(&tx_bytes)?;

    submit_signed_to_rpc(rpc_url, &signed_bytes)
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
        if stream.write_all(&len.to_le_bytes()).is_err()
            || stream.write_all(tx_bytes).is_err()
        {
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
    Err(format!("Could not connect to Tauri signing server on ports {}-{}", range.start(), range.end()))
}

/// Deserialise the wire-format signed bytes into a `VersionedTransaction` and
/// submit it to the Solana RPC, then block until confirmed.
fn submit_signed_to_rpc(rpc_url: &str, signed_bytes: &[u8]) -> Result<Signature, String> {
    let signed_tx: VersionedTransaction = bincode::deserialize(signed_bytes)
        .map_err(|e| format!("deserialize_signed_tx: {}", e))?;

    let rpc = RpcClient::new(rpc_url.to_string());

    rpc.send_and_confirm_transaction(&signed_tx)
        .map_err(|e| format!("send_and_confirm: {}", e))
}
