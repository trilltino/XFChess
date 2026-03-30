//! Phantom / Solflare wallet signing via the Tauri IPC bridge.
//!
//! When the game binary is launched from the Tauri wrapper
//! (`XFCHESS_WALLET_MODE=tauri`), a TCP signing server is available on
//! `127.0.0.1:7443`.  This module sends a raw transaction to that server
//! and receives back the wallet-signed version.
//!
//! Binary protocol (one TCP connection per request):
//!   Client → Server: `[u32 le: len][tx_bytes]`
//!   Server → Client: `[u32 le: len][signed_bytes]`  (success)
//!                 or `[u32 le: 0xFFFF_FFFF]`         (failure)
//!
//! If the server is not available (no Tauri wrapper, or wallet not connected)
//! this function returns `None` so the caller can fall back to local signing.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const SIGN_SERVER_ADDR: &str = "127.0.0.1:7443";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const READ_TIMEOUT: Duration = Duration::from_secs(65);

/// Returns `true` if the game is running inside the Tauri wrapper and
/// the Phantom signing bridge may be available.
pub fn is_wallet_mode() -> bool {
    std::env::var("XFCHESS_WALLET_MODE")
        .map(|v| v == "tauri")
        .unwrap_or(false)
}

/// Send `tx_bytes` to the Tauri signing bridge and return the signed bytes.
///
/// Returns `None` if the bridge is not reachable or the wallet is not
/// connected — the caller should fall back to local keypair signing.
pub fn phantom_sign(tx_bytes: &[u8]) -> Option<Vec<u8>> {
    let mut stream = TcpStream::connect_timeout(
        &SIGN_SERVER_ADDR.parse().ok()?,
        CONNECT_TIMEOUT,
    )
    .ok()?;

    stream.set_read_timeout(Some(READ_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(CONNECT_TIMEOUT)).ok()?;

    // Send length-prefixed tx bytes.
    let len = tx_bytes.len() as u32;
    stream.write_all(&len.to_le_bytes()).ok()?;
    stream.write_all(tx_bytes).ok()?;

    // Read response length prefix.
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).ok()?;
    let resp_len = u32::from_le_bytes(len_buf);

    if resp_len == 0xFFFF_FFFF {
        tracing::warn!("[PHANTOM] Signing bridge returned error");
        return None;
    }

    // Read signed bytes.
    let mut signed = vec![0u8; resp_len as usize];
    stream.read_exact(&mut signed).ok()?;
    Some(signed)
}
