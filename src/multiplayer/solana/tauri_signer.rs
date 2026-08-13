//! Tauri signing bridge for Phantom / Solflare wallet.
//!
//! Bevy builds unsigned [`VersionedTransaction`]s; this module forwards them
//! over the local TCP channel to the Tauri signing server, which prompts the
//! browser wallet, receives the signed bytes, and returns them here.
//! The fully-signed transaction is then submitted to the Solana RPC.
//!
//! # Wire protocol (per `tauri/src/main.rs`)
//! - Client → Server : `4-byte LE label length` + `label utf8 bytes` +
//!                      `4-byte LE tx length` + `raw VersionedTransaction bytes`
//! - Server → Client : `4-byte LE length` + `signed VersionedTransaction bytes`
//!                      OR `0xFFFF_FFFF` on rejection / error
//!
//! The label is a short human-readable description of what's being signed
//! (e.g. "Joining game", "Placing wager") shown in the wallet popup instead
//! of a generic "Awaiting signature" message.

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

/// Maximum seconds to wait for the user to approve the transaction in Phantom.
const SIGN_TIMEOUT_SECS: u64 = 60;

fn nominal_wallet_bridge_port() -> u16 {
    std::env::var("XFCHESS_WALLET_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7454)
}

/// Path the Tauri bridge's HTTP server writes its actual bound port to.
/// Must match `http_bridge_port_file()` in `tauri/src/main.rs`. The bridge
/// tries the nominal port first and only falls back to scanning upward if
/// that's already taken (e.g. a second local instance with no
/// XFCHESS_WALLET_PORT override) — without reading this file back, every
/// HTTP call this client makes to the bridge would still target the
/// nominal port even when the bridge itself is actually listening
/// somewhere else, which looked identical to the bridge not running at all.
fn http_bridge_port_file() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "xfchess-wallet-http-{}.port",
        nominal_wallet_bridge_port()
    ))
}

/// HTTP port for this instance's Tauri wallet bridge: the bridge's
/// announced actual port if its port file is present and parses, else the
/// nominal XFCHESS_WALLET_PORT-derived value (default 7454) — matching the
/// common single-instance case where they're always the same.
pub fn wallet_bridge_port() -> u16 {
    if let Some(port) = std::env::var("XFCHESS_ACTUAL_WALLET_PORT")
        .ok()
        .and_then(|s| s.trim().parse().ok())
    {
        return port;
    }

    std::fs::read_to_string(http_bridge_port_file())
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or_else(nominal_wallet_bridge_port)
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
/// The Tauri side binds TCP on (base-11)..=(base-2). Only used as a
/// fallback — see `discovered_bridge_port()` — since scanning a shared
/// range can hit an unrelated live listener (e.g. another local instance's
/// HTTP wallet-bridge server) and stall for seconds before it closes the
/// connection, instead of failing fast.
fn tcp_port_range() -> std::ops::RangeInclusive<u16> {
    let base: u16 = wallet_bridge_port();
    base.saturating_sub(11)..=base.saturating_sub(2)
}

/// Path the Tauri side writes its actual bound TCP port to on startup.
/// Must match `wallet_bridge_port_file()` in `tauri/src/main.rs`.
fn wallet_bridge_port_file() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "xfchess-wallet-bridge-{}.port",
        wallet_bridge_port()
    ))
}

/// The port the Tauri bridge announced it actually bound to, if its port
/// file is present and parses. Trying this first (before falling back to
/// `tcp_port_range()`) skips the scan entirely on the common path.
fn discovered_bridge_port() -> Option<u16> {
    std::fs::read_to_string(wallet_bridge_port_file())
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

/// Ports to attempt, in order: the bridge's announced port first (if known),
/// then the full scan range as a fallback for a stale/missing port file.
/// `pub` so other call sites that need to reach the raw TCP bridge (e.g.
/// `integration::systems::query_wallet_pubkey_from_tauri`) go through this
/// single implementation instead of hand-rolling their own scan that can
/// drift out of sync with it — see the wallet-bridge port-discovery audit.
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

/// Fire-and-forget: requests the Tauri wallet popup window for wallet connection.
/// Spawns a background thread so Bevy is never blocked.
pub fn open_wallet_browser() {
    bring_wallet_popup_to_front();
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

/// Bring the wallet popup window to the front — called from *this* process
/// (the game client) rather than the Tauri sidecar, because that's the only
/// way it reliably works on Windows.
///
/// `SetForegroundWindow` only succeeds for a caller whose own process
/// currently owns the foreground window, was started by it, or just
/// received real user input (see the Win32 docs for the exact rule). The
/// Tauri sidecar (`xfchess-tauri.exe`) is a background process that never
/// itself has focus, so it satisfies none of those — neither
/// `AttachThreadInput` nor a synthesized Alt keystroke made its foreground
/// calls reliable in practice (both tried and dropped; see git history on
/// `tauri/src/main.rs`'s `force_foreground_window`). This process
/// (`xfchess.exe`), by contrast, unquestionably *is* the foreground process
/// at the exact moment the user clicks "Connect Wallet" or triggers a
/// signing request, so a plain `SetForegroundWindow` call from here hits
/// Windows' first, unconditional exemption and needs no workaround at all.
///
/// Finds the window by the same `XFChess #<port>` title Tauri's own
/// hide/show logic uses (stamped by `tauri/wallet-ui/src/App.tsx`) and polls
/// briefly since a freshly-spawned Chrome/Edge process needs a moment to
/// create its window. Tauri still owns actually spawning/reusing the
/// popup process — this only makes an existing (or about-to-exist) window
/// visible once it appears.
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

/// Like `sign_and_send_via_tauri` but returns the signed transaction bytes
/// without submitting to RPC. Used by the VPS flow where the VPS submits.
///
/// `label` is a short human-readable description shown in the wallet popup,
/// e.g. "Joining game".
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
    let blockhash = rpc
        .get_latest_blockhash()
        .map_err(|e| format!("get_latest_blockhash: {}", e))?;
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

/// Build a `VersionedTransaction` (v0), partially sign with `local_signers`
/// (e.g. a session keypair), send to the Tauri signing bridge for Phantom to
/// co-sign as fee-payer, then submit and confirm on-chain.
///
/// Pass an empty slice for `local_signers` when only the wallet needs to sign.
/// `label` is a short human-readable description shown in the wallet popup.
pub fn sign_and_send_via_tauri(
    rpc_url: &str,
    wallet_pubkey: Pubkey,
    instructions: &[Instruction],
    local_signers: &[&Keypair],
    label: &str,
) -> Result<Signature, String> {
    let rpc = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let step_start = std::time::Instant::now();
    let blockhash = rpc
        .get_latest_blockhash()
        .map_err(|e| format!("get_latest_blockhash: {}", e))?;
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

/// Sign and send a pre-built base64-encoded transaction via the Tauri bridge.
/// `label` is a short human-readable description shown in the wallet popup.
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

/// Sign an arbitrary message (e.g. for TEE authentication) via the Tauri signing bridge.
/// `label` is a short human-readable description shown in the wallet popup.
pub fn sign_message_via_tauri(message: &str, label: &str) -> Result<Vec<u8>, String> {
    info!("[TAURI-SIGN] Requesting message signature: '{}'", message);
    send_to_tauri_blocking(message.as_bytes(), label)
}

/// Real transactions are a few KB; this mirrors the Tauri side's own
/// `MAX_TX_LEN` sanity check so a bogus/huge length read off a port that
/// isn't actually our bridge can't trigger a giant allocation.
const MAX_RESP_LEN: u32 = 64 * 1024;

/// Send raw transaction bytes to the Tauri signing server and block until the
/// signed bytes are returned or an error occurs.
///
/// `tcp_port_range()` spans 10 ports so multiple local dev instances can each
/// claim their own slot — but a `connect()` succeeding doesn't prove the peer
/// is actually our bridge (a stale/orphaned instance, or any other unrelated
/// service, could be squatting on an earlier port). So a connection that
/// fails immediately (EOF / bad length) is treated as "not our bridge" and
/// scanning continues to the next port. A real timeout, by contrast, means
/// we did reach a live bridge that's genuinely waiting on the user — that's
/// surfaced as-is rather than retried, so we don't fire a second signing
/// popup on top of one still pending.
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

/// Deserialise the wire-format signed bytes into a `VersionedTransaction` and
/// submit it to the Solana RPC. Wait briefly for an immediate failure, but do
/// not block the UI on devnet confirmation latency.
///
/// Delegates the actual send+poll to `submit::submit_and_poll` — the shared
/// strategy (`skip_preflight`, 150ms polling, 2s deadline) used by every
/// locally-submitted transaction in this codebase, not just wallet-signed
/// ones. See that module's doc comment for why this exists as one function.
fn submit_signed_to_rpc(rpc_url: &str, signed_bytes: &[u8]) -> Result<Signature, String> {
    use super::submit::{submit_and_poll, SubmitConfig};

    let signed_tx: VersionedTransaction =
        bincode::deserialize(signed_bytes).map_err(|e| format!("deserialize_signed_tx: {}", e))?;

    let rpc = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    submit_and_poll(&rpc, &signed_tx, SubmitConfig::fast())
}
