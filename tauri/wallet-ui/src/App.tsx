import { useState, useEffect, useRef, type CSSProperties } from "react";
import bs58 from "bs58";
import { usePrivy, useLogin, useLogout } from "@privy-io/react-auth";
import { useWallets, useSignMessage, useSignTransaction, useCreateWallet } from "@privy-io/react-auth/solana";
import type { WalletSource } from "./wallet/types";
import { connectExtension } from "./wallet/extension";
import { privyWalletSource } from "./wallet/privy";
import { PRIVY_ENABLED, SOLANA_CHAIN } from "./privy/config";

// ---------------------------------------------------------------------------
// REST API bridge — works in Chrome AND Tauri webview
// ---------------------------------------------------------------------------
// In dev (`npm run dev`), this page is served by Vite's own dev server
// (port 5174) — window.location.port would be *that*, not the bridge's, so
// dev needs the explicit override/default below. In every real build,
// though, the bridge serves this page itself (see wallet_ui_dist_path in
// tauri/src/main.rs), so window.location.port IS the bridge's actual port —
// and it must be read from there, not hardcoded at build time. A second
// local instance (different XFCHESS_WALLET_PORT, e.g. two windows open at
// once) binds its bridge to a *different* port than the first; a
// build-time constant here would make every instance's popup talk to
// whichever one happened to grab the shared default port first — wrong
// wallet, wrong pending signature, no error, just silently talking to the
// other window's bridge.
const BRIDGE_PORT = import.meta.env.DEV
  ? (import.meta.env.VITE_BRIDGE_PORT ?? "7454")
  : (window.location.port || "7454");
const API_BASE = `http://localhost:${BRIDGE_PORT}`;

// Every instance's popup window is otherwise titled the same static
// "XFChess" (see index.html) — indistinguishable to Windows' EnumWindows.
// tauri/src/main.rs's kill_wallet_popup() closes a popup by finding a
// chrome.exe/msedge.exe top-level window with this exact title; without a
// per-port suffix, running two local instances (e.g. `just dev2`) means
// either instance closing its own popup closes *both* players' popups,
// since the match is desktop-wide by title text alone, not scoped to which
// Tauri sidecar spawned it. main.rs must match this exact format.
document.title = `XFChess #${BRIDGE_PORT}`;

// Phantom/Solflare's page-injected `provider` proxies every call through a
// content script to the extension's background service worker. When that
// relay is broken (MV3 background asleep and failing to wake, or the
// content script itself never injected into this specific popup window —
// surfaces in DevTools as "Could not establish connection. Receiving end
// does not exist") the injected provider methods don't reject, they just
// never resolve. Without this wrapper `provider.connect()` hangs forever:
// the "Connect" button's spinner never clears and no error ever reaches
// setError, so the user has no signal anything went wrong and no way to
// retry short of closing and reopening the whole popup.
export function withTimeout<T>(p: Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error(`${label} timed out — try closing this window and reopening it.`)),
      ms,
    );
    p.then((v) => { clearTimeout(timer); resolve(v); },
           (e) => { clearTimeout(timer); reject(e); });
  });
}

// ---------------------------------------------------------------------------
// Session correlation — a fresh `sid` is minted by Tauri every time it opens
// this popup (see begin_session/open_wallet_popup_with_step in
// tauri/src/main.rs) and passed in the URL. Reading it back here and
// stamping it on every request (as X-Session-Id, forwarded by the bridge to
// the backend as x-request-id) means one login/sign attempt's Chrome-spawn
// log lines, this page's own console output, and the backend's request logs
// all carry the same id end to end. A direct `npm run dev` load (or an old
// cached page with no `sid`) falls back to a locally-minted id so logging
// still works, it just won't correlate with anything outside this page.
const SESSION_ID =
  new URLSearchParams(window.location.search).get("sid") ||
  `local-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;

function logLifecycle(event: string, detail?: unknown) {
  // eslint-disable-next-line no-console
  console.log(`[Lifecycle sid=${SESSION_ID}] ${event}`, detail ?? "");
}

async function apiGet<T = unknown>(path: string): Promise<T> {
  const resp = await fetch(`${API_BASE}${path}`, {
    headers: { "X-Session-Id": SESSION_ID },
  });
  if (!resp.ok) throw new Error(`GET ${path} failed: ${resp.status}`);
  return resp.json() as Promise<T>;
}

// Closing the popup: we always run as a real OS-level Chrome window (never an
// embedded Tauri webview — see open_wallet_popup in tauri/src/main.rs), so
// `window.close()` is unreliable — Chrome blocks scripts from closing windows
// they didn't open themselves. Ask the Tauri sidecar to kill the process it
// spawned instead; that's the only reliable way to close this window. Only
// fall back to window.close() if the bridge itself is unreachable.
async function closePopup() {
  try {
    await fetch(`${API_BASE}/hide`, { method: "POST", headers: { "X-Session-Id": SESSION_ID } });
  } catch {
    window.close();
  }
}

// Which wallet extension the user actually authenticated with, persisted at
// connect time (see WalletStep.handleConnect). Signing must always go back
// through this SAME extension — if both Phantom and Solflare are installed,
// blindly preferring one (Phantom used to always win) silently signs with
// the wrong wallet: the popup shows a real "Confirm Transaction" dialog with
// a valid-looking fee, but for an account the player never funded, which
// surfaces as a confusing "not enough SOL" even though their actual wallet
// has plenty.
export function getConnectedProvider(): any {
  const kind = localStorage.getItem("xfchess_wallet_provider");
  if (kind === "solflare") return (window as any).solflare;
  if (kind === "phantom") return (window as any).phantom?.solana;
  // An embedded wallet has no extension to hand back, and the fallback below
  // would return Phantom — which is a *different keypair*. That is how profile
  // creation for a Google user ended up asking Phantom to sign a transaction
  // whose fee payer and profile owner were the Privy address: the signature
  // could never be valid for it. Callers must take their Privy branch.
  if (kind === "privy") return null;
  // Unknown (e.g. state from before this was tracked) — fall back to the old
  // best-effort behavior rather than refusing to sign at all.
  return (window as any).phantom?.solana ?? (window as any).solflare;
}

/// Best-effort switch the connected wallet to devnet so that transactions
/// built against the devnet RPC are not rejected as "mainnet" txs.
/// Solflare in particular infers the transaction cluster from the dApp's
/// declared network; if none is declared it can default to mainnet and
/// refuse to sign devnet blockhashes.
async function ensureDevnet(provider: any, kind: string | null): Promise<void> {
  if (!provider) return;
  // Phantom >=0.16 supports switchNetwork via request()
  if (kind === "phantom" && provider.request) {
    try {
      await provider.request({ method: "switchNetwork", params: { network: "devnet" } });
      return;
    } catch { /* ignore — may be unsupported or already on devnet */ }
  }
  // Solflare supports cluster selection through connect() or setCluster()
  if (kind === "solflare") {
    if (provider.setCluster) {
      try {
        await provider.setCluster("devnet");
        return;
      } catch { /* ignore */ }
    }
    if (provider.request) {
      try {
        await provider.request({ method: "switchNetwork", params: { network: "devnet" } });
        return;
      } catch { /* ignore */ }
    }
  }
}

// `ensureDevnet` above is best-effort (neither wallet exposes a documented
// readback to verify the switch actually took), but it is NOT what the
// "network mismatch" repro was about, despite what this comment used to say.
//
// Root cause, measured: a wallet extension cannot tell which cluster a
// transaction targets from the transaction bytes — the only cluster-specific
// thing in there is the recent blockhash — so it looks that blockhash up on
// whichever cluster it is currently set to. That lookup (`isBlockhashValid`)
// defaults to `finalized` commitment, and we were handing it blockhashes
// fetched at `confirmed`, i.e. younger than the ~32 slot (~13s) finalization
// lag. Against our own devnet endpoint, checked from a second devnet node:
//
//   getLatestBlockhash{confirmed} -> isBlockhashValid{finalized} = false
//   getLatestBlockhash{finalized} -> isBlockhashValid{finalized} = true
//
// So Solflare could not confirm the blockhash existed on devnet, concluded the
// transaction must belong to the other cluster, and refused to sign with "your
// current network is set to devnet, but this transaction is for mainnet" —
// while the user was already correctly on devnet, which is why the old advice
// to go switch networks was useless. Fixed at the source: every blockhash that
// goes into a wallet-signed transaction is now fetched at `finalized` — see
// `/api/fresh-blockhash` in tauri/src/main.rs, `wallet_signable_blockhash` in
// the backend's signing/solana/rpc.rs, and the same-named helper in
// src/multiplayer/solana/tauri_signer.rs.
//
// The reactive detection below stays as a safety net — it still catches the
// genuine case (the wallet really is on mainnet) and any wallet whose own RPC
// lags further behind than finalization — but the message it maps to no longer
// assumes the user did something wrong.
export function isNetworkMismatchError(e: any): boolean {
  const msg = String(e?.message ?? e ?? "").toLowerCase();
  const mentionsNetwork = msg.includes("network") || msg.includes("cluster");
  const mentionsClusterNames = msg.includes("devnet") || msg.includes("mainnet");
  const mentionsMismatch =
    msg.includes("mismatch") || msg.includes("but this transaction is for") || msg.includes("wrong network");
  return mentionsNetwork && mentionsClusterNames && mentionsMismatch;
}

export const NETWORK_MISMATCH_MESSAGE =
  "Your wallet could not verify this as a Devnet transaction. Try again — if it " +
  "keeps happening, check that the extension's network is set to Devnet " +
  "(extension → network/cluster settings → Devnet).";

// `token` is optional because most `apiPost` call sites hit unauthenticated
// bridge routes (e.g. `/token`, `/api/game/launch`) — but any route proxied
// through to a `authed_wallet`-gated backend endpoint (init-profile-tx,
// broadcast-tx, ...) needs a Bearer token forwarded, or the Tauri bridge's
// `forward_client_headers` (tauri/src/main.rs) has nothing to forward and
// the backend 401s with "Missing Authorization header." Omitting it here was
// a real, reproduced bug for `init-profile-tx`/`broadcast-tx` specifically —
// every other authenticated call site in this file already builds its own
// raw `fetch` with the header by hand (see the `/api/auth/username` PATCH
// above `ProfileStep.submit`) rather than going through this helper.
async function apiPost<T = unknown>(path: string, body?: unknown, token?: string | null): Promise<T> {
  const headers: Record<string, string> = { "Content-Type": "application/json", "X-Session-Id": SESSION_ID };
  if (token) headers["Authorization"] = `Bearer ${token}`;
  const resp = await fetch(`${API_BASE}${path}`, {
    method: "POST",
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(text || `POST ${path} failed: ${resp.status}`);
  }
  const ct = resp.headers.get("content-type") ?? "";
  if (ct.includes("application/json")) return resp.json() as Promise<T>;
  return null as T;
}

// Detects the specific on-chain rejection a stale blockhash produces
// ("Blockhash not found" from simulation, RPC error -32002) so a broadcast
// failure caused by *that* — as opposed to a real rejection (insufficient
// funds, program error, network down) — can be retried instead of just
// failing outright. `refreshBlockhash` closes most of this gap by fetching
// as late as possible, but it can't do anything about a human who takes
// their time approving inside the wallet extension itself: that delay
// happens strictly *after* the refresh, so the freshly-fetched blockhash can
// still expire before the signed transaction is actually broadcast.
export function isStaleBlockhashError(e: any): boolean {
  const msg = String(e?.message ?? e ?? "").toLowerCase();
  return msg.includes("blockhash not found") || msg.includes("-32002");
}

/**
 * Overwrites `tx`'s blockhash with a freshly-fetched one, in place, right
 * before signing. The blockhash a backend route baked in at build time
 * (e.g. /api/auth/init-profile-tx) can go stale by the time a real human
 * finishes clicking through the wallet extension's own approval popup —
 * Solana blockhashes are only valid ~60-90s, with no upper bound on how
 * long that click takes. Reproduced live: broadcast-tx 502ing with "RPC
 * response error -32002: Transaction simulation failed: Blockhash not
 * found" even though signing itself had already succeeded. Best-effort: on
 * any failure, leaves `tx` untouched and lets the caller's existing
 * error/retry path handle it exactly as before this existed.
 */
export async function refreshBlockhash(tx: web3.Transaction | web3.VersionedTransaction): Promise<boolean> {
  try {
    const resp = await fetch(`${API_BASE}/api/fresh-blockhash`, { headers: { "X-Session-Id": SESSION_ID } });
    if (!resp.ok) {
      apiPost("/api/debug-log", { msg: `refreshBlockhash: bridge responded ${resp.status}` }).catch(() => {});
      return false;
    }
    const { blockhash } = await resp.json();
    if (typeof blockhash !== "string" || !blockhash) {
      apiPost("/api/debug-log", { msg: "refreshBlockhash: response had no blockhash string" }).catch(() => {});
      return false;
    }
    if (tx instanceof web3.VersionedTransaction) {
      tx.message.recentBlockhash = blockhash;
    } else {
      tx.recentBlockhash = blockhash;
    }
    return true;
  } catch (e: any) {
    // Best-effort by design — caller proceeds with whatever blockhash it
    // already had rather than blocking the sign flow entirely — but this
    // used to be completely silent even on failure, which is exactly the
    // kind of gap that made the original stale-blockhash bug hard to
    // distinguish from "refreshed, but the user still took too long to
    // approve in their wallet extension." Now both failure modes are
    // distinguishable from the Tauri console.
    apiPost("/api/debug-log", { msg: `refreshBlockhash: threw ${e?.message || e}` }).catch(() => {});
    return false;
  }
}

/**
 * A wallet's on-chain profile status — the single source of truth for
 * whether the connect flow needs to show the profile step. Mirrors
 * programs/xfchess-game's PlayerProfile account (decoded server-side in
 * POST /api/auth/sync-profile). KYC (`is_verified`) is intentionally not
 * gated on here — that's checked later, at wager time, same as the
 * existing CACF compliance flow.
 */
interface ProfileStatus {
  has_profile: boolean;
  username_set: boolean;
  is_verified: boolean;
  username: string | null;
}

async function fetchProfileStatus(token: string): Promise<ProfileStatus> {
  const resp = await fetch(`${API_BASE}/api/auth/sync-profile`, {
    method: "POST",
    headers: { Authorization: `Bearer ${token}`, "X-Session-Id": SESSION_ID },
  });
  if (!resp.ok) throw new Error(`sync-profile failed: ${resp.status}`);
  return resp.json();
}

async function fetchMe(token: string): Promise<{ username: string }> {
  const resp = await fetch(`${API_BASE}/api/auth/me`, {
    headers: { Authorization: `Bearer ${token}`, "X-Session-Id": SESSION_ID },
  });
  if (!resp.ok) throw new Error(`auth/me failed: ${resp.status}`);
  return resp.json();
}

/**
 * Whether this wallet already has a real, user-chosen display name — checked
 * two ways because "profile" means two different things here:
 *  - on-chain PlayerProfile.username_set (ProfileStatus) — only becomes true
 *    once the player's first wager creates the on-chain profile.
 *  - the off-chain account username (GET /auth/me) — set immediately by
 *    ProfileStep's PATCH /api/auth/username, with no wager required.
 * A player who already completed ProfileStep but hasn't wagered yet has a
 * real off-chain name and an unset on-chain one — checking sync-profile
 * alone would re-show "Choose Your Handle" on every reconnect. Wallet
 * registration also seeds a throwaway `pubkey.slice(0, 8)` placeholder
 * (see WalletStep's /api/auth/register call) into that same off-chain
 * field, so it must be excluded here or every fresh wallet would look like
 * it already has a name.
 */
export async function resolveExistingUsername(
  token: string,
  pubkey: string,
  onChain: ProfileStatus,
): Promise<string | null> {
  if (onChain.username_set && onChain.username) return onChain.username;
  try {
    const me = await fetchMe(token);
    const registrationPlaceholder = pubkey.slice(0, 8);
    if (me.username && me.username !== registrationPlaceholder) return me.username;
  } catch { /* /auth/me unavailable — caller falls back to needsProfile */ }
  return null;
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------
type Step = "wallet" | "profile" | "splash" | "sign";

interface AuthResponse {
  token: string;
  username: string;
  wallet?: string;
}

// ---------------------------------------------------------------------------
// Design tokens — matches xfchessdotcom color scheme
// ---------------------------------------------------------------------------
const PRIMARY    = "#ffffff";
const PRIMARY_DIM    = "rgba(255,255,255,0.08)";
const PRIMARY_BORDER = "rgba(255,255,255,0.30)";
const ACCENT     = "#ffffff";
const BG         = "#000000";
const SURFACE    = "#0d0d0d";
const CARD_BG    = "#111111";
const BORDER     = "rgba(255,255,255,0.12)";
const TEXT       = "#ffffff";
const TEXT_DIM   = "#888888";
const TEXT_MUTED = "rgba(255,255,255,0.25)";
const INPUT_BG   = "rgba(255,255,255,0.04)";
// Keep old names as aliases so unchanged code still compiles
const RED        = PRIMARY;
const RED_DIM    = PRIMARY_DIM;
const RED_BORDER = PRIMARY_BORDER;

// ---------------------------------------------------------------------------
// Keyframes
// ---------------------------------------------------------------------------
const KEYFRAMES = `
  @import url('https://fonts.googleapis.com/css2?family=Cinzel:wght@400;600;700;800;900&display=swap');
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { font-family: 'Cinzel', serif; background: ${BG}; color: ${TEXT}; overflow-y: auto; -webkit-font-smoothing: antialiased; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes fadeUp { from { opacity: 0; transform: translateY(16px); } to { opacity: 1; transform: translateY(0); } }
  @keyframes wave { 0%,100% { transform: translateY(0); } 50% { transform: translateY(-6px); } }
  @keyframes glow { 0%,100% { text-shadow: 0 0 20px rgba(255,255,255,0.3); } 50% { text-shadow: 0 0 40px rgba(255,255,255,0.6); } }
  @keyframes progress { from { width: 0%; } to { width: 100%; } }
  @keyframes pulse { 0%,100% { opacity:1; transform: scale(1); } 50% { opacity:0.6; transform: scale(0.97); } }
  @keyframes shimmer { 0% { background-position: -200% center; } 100% { background-position: 200% center; } }
  input { outline: none; font-family: 'Cinzel', serif; }
  input::placeholder { color: ${TEXT_MUTED}; }
  button { cursor: pointer; font-family: 'Cinzel', serif; }
  a { color: ${TEXT_DIM}; text-decoration: none; }
  ::-webkit-scrollbar { width: 4px; }
  ::-webkit-scrollbar-track { background: transparent; }
  ::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.15); border-radius: 2px; }
`;

// ---------------------------------------------------------------------------
// Layout helpers
// ---------------------------------------------------------------------------
const page: CSSProperties = {
  width: "100vw", minHeight: "100vh", display: "flex", flexDirection: "column",
  alignItems: "center", justifyContent: "center", background: BG,
  position: "relative", overflowY: "auto", padding: "24px 0",
};

// ---------------------------------------------------------------------------
// Navbar — matches xfchessdotcom pill style; links back to /
// ---------------------------------------------------------------------------
function SiteNav() {
  const HOME = window.location.origin + "/";
  return (
    <nav style={{
      position: "fixed", top: 16, left: "50%", transform: "translateX(-50%)",
      width: "92%", maxWidth: 520, height: 48, padding: "0 20px",
      display: "flex", alignItems: "center", justifyContent: "space-between",
      zIndex: 100,
      background: "rgba(0,0,0,0.80)",
      border: `1px solid ${BORDER}`,
      borderRadius: 100,
      backdropFilter: "blur(24px)", WebkitBackdropFilter: "blur(24px)",
      boxShadow: `0 10px 40px rgba(0,0,0,0.6), 0 0 50px rgba(255,255,255,0.04)`,
      transition: "all 0.3s ease",
    }}>
      <a href={HOME} style={{
        display: "flex", alignItems: "center", gap: 0,
        textDecoration: "none", userSelect: "none",
        fontSize: 13, fontWeight: 700, letterSpacing: "0.06em", color: TEXT,
        padding: "5px 12px", borderRadius: 20,
        border: `1px solid rgba(255,255,255,0.08)`,
        background: "rgba(255,255,255,0.05)",
      }}>
        XFCHESS
      </a>
      <a href={HOME} style={{
        fontSize: 11, fontWeight: 600, color: TEXT_DIM,
        textDecoration: "none", letterSpacing: "0.04em",
        padding: "5px 14px", borderRadius: 20,
        border: `1px solid ${BORDER}`,
        transition: "all 0.2s",
      }}
        onMouseEnter={e => { (e.currentTarget as HTMLAnchorElement).style.color = TEXT; (e.currentTarget as HTMLAnchorElement).style.background = "rgba(255,255,255,0.06)"; }}
        onMouseLeave={e => { (e.currentTarget as HTMLAnchorElement).style.color = TEXT_DIM; (e.currentTarget as HTMLAnchorElement).style.background = "transparent"; }}
      >Home</a>
    </nav>
  );
}

function GridBg() {
  return (
    <>
      {/* Subtle white radial glow — matches xfchessdotcom bg */}
      <div style={{
        position: "fixed", inset: 0, zIndex: 0, pointerEvents: "none",
        background: `radial-gradient(ellipse 80% 60% at 50% 0%, rgba(255,255,255,0.06) 0%, transparent 70%),
                     radial-gradient(ellipse 60% 40% at 80% 80%, rgba(255,255,255,0.03) 0%, transparent 60%)`,
      }} />
    </>
  );
}

function LogoMark({ size = 40 }: { size?: number }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 0, userSelect: "none" }}>
      <span style={{ fontSize: size * 0.55, fontFamily: "'Cinzel', serif", fontWeight: 800, letterSpacing: "0.08em", color: TEXT }}>
        XFCHESS
      </span>
    </div>
  );
}

function Card({ children, style, showClose = true, onClose }: { children: React.ReactNode; style?: CSSProperties; showClose?: boolean; onClose?: () => void }) {
  const close = async () => {
    if (onClose) {
      onClose();
      return;
    }
    await closePopup();
  };

  return (
    <div style={{
      width: "92%", maxWidth: 400, maxHeight: "calc(100vh - 48px)", overflowY: "auto",
      padding: "28px 32px", background: CARD_BG,
      border: `1px solid ${BORDER}`, borderRadius: 20,
      backdropFilter: "blur(24px)", WebkitBackdropFilter: "blur(24px)",
      boxShadow: `0 10px 40px rgba(0,0,0,0.6), 0 0 50px rgba(255,255,255,0.03)`,
      animation: "fadeUp 0.4s ease", position: "relative", zIndex: 1, ...style,
    }}>
      {showClose && (
        <button 
          onClick={close}
          style={{
            position: "absolute", top: 12, right: 12, 
            background: "rgba(255,255,255,0.1)", border: "none", color: "#ffffff",
            fontSize: 16, cursor: "pointer", width: 32, height: 32, borderRadius: "50%",
            display: "flex", alignItems: "center", justifyContent: "center",
            transition: "all 0.2s", zIndex: 100, fontWeight: "bold",
            boxShadow: "0 2px 8px rgba(0,0,0,0.3)",
          }}
          onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = "rgba(255,255,255,0.25)"; }}
          onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = "rgba(255,255,255,0.1)"; }}
        >X</button>
      )}
      {children}
    </div>
  );
}

function PrimaryBtn({
  children, onClick, disabled, loading, style,
}: {
  children: React.ReactNode; onClick?: () => void; disabled?: boolean; loading?: boolean; style?: CSSProperties;
}) {
  return (
    <button onClick={onClick} disabled={disabled || loading} style={{
      width: "100%", padding: "14px 0", borderRadius: 10, border: "none",
      background: disabled || loading ? "rgba(255,255,255,0.12)" : "#ffffff",
      color: disabled || loading ? TEXT_DIM : "#000000", fontSize: 15, fontWeight: 700, letterSpacing: "0.02em",
      transition: "all 0.2s", boxShadow: disabled || loading ? "none" : `0 4px 20px rgba(255,255,255,0.15)`,
      display: "flex", alignItems: "center", justifyContent: "center", gap: 8, ...style,
    }}>
      {loading && <div style={{ width: 16, height: 16, border: "2px solid rgba(255,255,255,0.3)", borderTop: "2px solid #fff", borderRadius: "50%", animation: "spin 0.7s linear infinite" }} />}
      {children}
    </button>
  );
}

function GhostBtn({ children, onClick }: { children: React.ReactNode; onClick?: () => void }) {
  return (
    <button onClick={onClick} style={{
      width: "100%", padding: "12px 0", borderRadius: 12, border: `1px solid ${BORDER}`,
      background: "transparent", color: TEXT_DIM, fontSize: 14, fontWeight: 500, transition: "all 0.2s",
    }}>
      {children}
    </button>
  );
}

function InputField({
  label, value, onChange, type = "text", placeholder,
}: {
  label: string; value: string; onChange: (v: string) => void; type?: string; placeholder?: string;
}) {
  return (
    <div style={{ marginBottom: 14 }}>
      <label style={{ fontSize: 12, fontWeight: 600, color: TEXT_DIM, letterSpacing: "0.06em", textTransform: "uppercase" as const, display: "block", marginBottom: 6 }}>
        {label}
      </label>
      <input type={type} value={value} onChange={e => onChange(e.target.value)} placeholder={placeholder} style={{
        width: "100%", padding: "12px 14px", borderRadius: 10, border: `1px solid ${BORDER}`,
        background: INPUT_BG, color: TEXT, fontSize: 15, transition: "border-color 0.2s",
      }} onFocus={e => (e.target.style.borderColor = RED_BORDER)} onBlur={e => (e.target.style.borderColor = BORDER)} />
    </div>
  );
}

function ErrorMsg({ msg }: { msg: string }) {
  return (
    <div style={{
      padding: "10px 14px", borderRadius: 10, background: "rgba(255,255,255,0.04)",
      border: `1px solid rgba(255,255,255,0.20)`, color: TEXT, fontSize: 13, marginBottom: 16,
    }}>
      {msg}
    </div>
  );
}

function StepDots({ step }: { step: Step }) {
  const steps: Step[] = ["wallet", "profile", "splash"];
  const idx = steps.indexOf(step);
  return (
    <div style={{ display: "flex", gap: 6, justifyContent: "center", marginBottom: 28 }}>
      {steps.map((_, i) => (
        <div key={i} style={{
          width: i === idx ? 20 : 6, height: 6, borderRadius: 3,
          background: i <= idx ? RED : "rgba(255,255,255,0.12)", transition: "all 0.3s",
        }} />
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 1 — Wallet Connection (Tauri Embedded)
// ---------------------------------------------------------------------------
import * as web3 from "@solana/web3.js";

/**
 * "Continue with Google" block.
 *
 * Privy authenticates the user and creates a Solana embedded wallet
 * (`createOnLogin: 'users-without-wallets'`). Once that wallet shows up in
 * `useWallets()`, this hands it to `onWallet` as a `WalletSource`, and it goes
 * through the *same* `authenticateWithBackend` flow every extension wallet
 * goes through — there is no privileged shortcut for social users.
 *
 * For a player with no wallet extension installed, this is the only path in the
 * popup that works at all: the Phantom/Solflare rows both render as
 * "not installed" for them, which was the dead end this whole feature exists to
 * remove.
 */
function SocialLoginBlock({
  onWallet,
  busy,
}: {
  onWallet: (src: WalletSource) => void;
  busy: boolean;
}) {
  const { ready, authenticated, user } = usePrivy();
  const { login } = useLogin();
  const { logout } = useLogout();
  const { wallets } = useWallets();
  const { createWallet } = useCreateWallet();
  const { signMessage } = useSignMessage();
  const { signTransaction } = useSignTransaction();
  const handedOff = useRef<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [provisioning, setProvisioning] = useState(false);
  const provisionTried = useRef(false);

  /**
   * Whether the player actually picked "Continue with Google" in THIS popup.
   *
   * Privy persists an authenticated session in the popup's Chrome profile, so a
   * player who used Google once came back to `authenticated === true` with a
   * populated `useWallets()` on the very first render. Both effects below then
   * fired unprompted: the wallet was handed to `onWallet`, which runs the full
   * `authenticateWithBackend` flow and raises a signature prompt. The player
   * never got to choose between Google, Phantom and Solflare — the popup had
   * already committed them to Google, and declining that prompt left "The user
   * rejected the request." sitting above three buttons, none of which had been
   * pressed.
   *
   * Nothing on the Privy path may run until the player has pressed the button.
   * A persisted session is still used — clicking Google skips straight past
   * `login()` — it just no longer acts on its own.
   *
   * Counted rather than a boolean because a boolean cannot express "pressed it
   * again": a second press would leave state and every effect dependency
   * unchanged, so nothing would re-run and a retry would be silently ignored.
   */
  const [attempt, setAttempt] = useState(0);
  const chosen = attempt > 0;

  // Hand the embedded wallet over exactly once per address. Privy re-renders
  // this array on many state transitions, and re-firing would start a second
  // login/register round-trip (and a second signature prompt) for a wallet
  // already being processed.
  useEffect(() => {
    if (!chosen || !authenticated) return;
    const wallet = wallets[0];
    if (!wallet?.address) return;
    if (handedOff.current === wallet.address) return;
    handedOff.current = wallet.address;
    logLifecycle("WALLET_CONNECT_START", { wallet: "privy" });
    onWallet(privyWalletSource(wallet, signMessage, signTransaction));
    // `onWallet` is recreated per render by the parent; including it would
    // re-run this effect constantly. The `handedOff` ref is the real guard.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [attempt, authenticated, wallets, signMessage, signTransaction]);

  /**
   * Whether this Privy session already has a Solana embedded wallet.
   *
   * `useWallets()` is empty for a moment after login while it hydrates, so an
   * empty array on its own is not evidence of "no wallet" — provisioning off
   * that alone races the hydration and asks Privy for a second wallet. The
   * user object's `linkedAccounts` is the authoritative record and settles
   * first, so it is the one that decides.
   */
  const hasEmbeddedWallet =
    wallets.length > 0 ||
    !!user?.linkedAccounts?.some(
      (a) =>
        a.type === "wallet" &&
        a.chainType === "solana" &&
        !!a.walletClientType?.startsWith("privy")
    );

  /**
   * Create the embedded wallet ourselves rather than leaving it to Privy's
   * `createOnLogin`.
   *
   * `createOnLogin` only fires inside the login flow, and the screen it drives
   * (`EmbeddedWalletOnAccountCreateScreen`) silently does nothing at all if
   * `user`, the access token, or the wallet proxy is missing — no error, no
   * navigation, no close, just the "Creating your wallet" spinner forever.
   * Worse, it leaves an authenticated session with no wallet, and `login()`
   * then refuses to run ("user is already logged in"), so the button becomes
   * inert and there is no way out of the popup.
   *
   * `createWallet()` is the headless equivalent: it either resolves or throws
   * something we can show the user, and it can be retried from the button.
   */
  const provision = async () => {
    if (provisioning) return;
    setProvisioning(true);
    setError(null);
    logLifecycle("PRIVY_CREATE_WALLET_START");
    try {
      await createWallet();
      logLifecycle("PRIVY_CREATE_WALLET_OK");
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      logLifecycle("PRIVY_CREATE_WALLET_FAILED", { error: msg });
      setError(`Could not create your wallet: ${msg}`);
    } finally {
      setProvisioning(false);
    }
  };

  // A session that came back authenticated but wallet-less — either from a
  // previous run that hung, or from `createOnLogin` no-opping — is repaired
  // on sight rather than waiting for the user to work out that the button
  // needs pressing again.
  useEffect(() => {
    if (!chosen || !ready || !authenticated || !user) return;
    if (hasEmbeddedWallet || provisionTried.current) return;
    provisionTried.current = true;
    void provision();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [attempt, ready, authenticated, user, hasEmbeddedWallet]);

  const btnStyle: CSSProperties = {
    width: "100%", padding: "14px 20px", borderRadius: 12,
    border: `1px solid ${BORDER}`, background: "rgba(255,255,255,0.03)",
    color: TEXT, fontSize: 15, fontWeight: 700, display: "flex",
    alignItems: "center", gap: 14, cursor: ready && !busy ? "pointer" : "wait",
    opacity: ready && !busy ? 1 : 0.6, transition: "all 0.2s",
  };

  // Signed in but wallet-less: the one state where the old button did nothing
  // but log "user is already logged in" to a console nobody was watching.
  const needsWallet = authenticated && !hasEmbeddedWallet;

  // Chosen, signed in, Privy says a wallet exists — but `useWallets()` has not
  // produced it yet. Both effects above bail in this window by design; without
  // a label for it the button reads as broken.
  const awaitingWallet =
    chosen && authenticated && hasEmbeddedWallet && !wallets[0]?.address;

  return (
    <div style={{ marginBottom: 20 }}>
      {error && <ErrorMsg msg={error} />}

      <button
        style={btnStyle}
        disabled={!ready || busy || provisioning}
        onClick={() => {
          // Recording the choice is what releases the two effects above; for a
          // persisted session that alone is enough to continue, with no second
          // trip through Google.
          setAttempt((n) => n + 1);
          // An explicit click means "do it now", so the duplicate-suppression
          // latch is dropped here. `handedOff` exists only to stop the effect
          // above starting a SECOND hand-off for a wallet already in flight; it
          // was never meant to be permanent. Nothing cleared it when a hand-off
          // failed, so after one declined signature prompt the effect saw an
          // address it had already handed off and returned on every subsequent
          // click — the button did nothing, with no error, for the rest of the
          // popup's life. This cannot race a live hand-off: the button is
          // disabled while `busy`, which is exactly when one is in flight.
          handedOff.current = null;
          if (!authenticated) { login({ loginMethods: ["google"] }); return; }
          if (!hasEmbeddedWallet) { void provision(); }
        }}
      >
        <span style={{ fontSize: 18, width: 20, textAlign: "center" }}>G</span>
        <span style={{ flex: 1 }}>
          {provisioning
            ? "Creating your wallet..."
            : awaitingWallet
              ? "Connecting your wallet..."
              : needsWallet
                ? "Finish setting up your wallet"
                : "Continue with Google"}
        </span>
      </button>

      {needsWallet && !provisioning && (
        <button
          onClick={async () => {
            provisionTried.current = false;
            setError(null);
            handedOff.current = null;
            setAttempt(0);
            await logout();
          }}
          style={{
            width: "100%", marginTop: 8, padding: "8px 0", background: "none",
            border: "none", color: TEXT_DIM, fontSize: 12, cursor: "pointer",
            textDecoration: "underline",
          }}
        >
          Sign out of Google and start over
        </button>
      )}

      <div style={{
        display: "flex", alignItems: "center", gap: 10, margin: "20px 0 4px",
        opacity: 0.45, fontSize: 11, letterSpacing: "0.08em",
      }}>
        <span style={{ flex: 1, height: 1, background: BORDER }} />
        <span>OR USE A WALLET</span>
        <span style={{ flex: 1, height: 1, background: BORDER }} />
      </div>
    </div>
  );
}

function WalletStep({
  onContinue, onAuth, onClose
}: {
  onContinue: (pubkey: string, provider: any) => void;
  onAuth: (token: string, user: string, pubkey: string) => void;
  onClose?: () => void;
}) {
  const [error, setError] = useState<string | null>(null);
  const [connecting, setConnecting] = useState<"phantom" | "solflare" | "privy" | null>(null);

  const WALLET_META = {
    phantom: { label: "Phantom", icon: "", installUrl: "https://phantom.app/", provider: () => (window as any).phantom?.solana },
    solflare: { label: "Solflare", icon: "", installUrl: "https://solflare.com/", provider: () => (window as any).solflare },
  };

  /**
   * The XFChess half of connecting: prove ownership, get a JWT, tell the bridge.
   *
   * Shared by every provider — extension or Privy — so the invariants below
   * exist once instead of once per provider. Each of them is a fixed bug:
   *
   *  1. `POST /wallet` happens only AFTER a signature verifies. Posting it
   *     earlier (right after `provider.connect()`) let a rejected sign-message
   *     prompt still leave the game client believing a wallet was connected,
   *     which unlocked wagered play.
   *  2. `username` comes from THIS call's auth response, never from
   *     localStorage. The popup's Chrome profile is shared across wallets, so a
   *     previous unrelated wallet's cached name leaked through here and was
   *     adopted by the game client's poller as authoritative (see
   *     `sync_bridge_pubkey_to_solana` in src/states/main_menu.rs, which
   *     explicitly trusts this POST).
   *  3. Ownership is always proven by a real signature. There used to be a
   *     "hot" local-keypair path that self-signed silently with no prompt at
   *     all; nothing may reintroduce that.
   */
  const authenticateWithBackend = async (src: WalletSource) => {
    const { pubkey, signRaw, kind } = src;
    if (!pubkey) throw new Error("No public key returned from wallet");
    localStorage.setItem("xfchess_wallet", pubkey);
    localStorage.setItem("xfchess_wallet_provider", kind);

    // Check registration status first — avoids redundant signing requests.
    logLifecycle("BACKEND_VERIFY", { path: "check-wallet" });
    const checkResp = await fetch(`${API_BASE}/api/auth/check-wallet/${pubkey}`, {
      headers: { "X-Session-Id": SESSION_ID },
    });
    const isRegistered = checkResp.ok;

    let auth: AuthResponse;
    logLifecycle("SIGN_REQUEST_START");
    if (isRegistered) {
      const ts = Math.floor(Date.now() / 1000);
      const sig = await signRaw(`xfchess:login:${ts}`);
      logLifecycle("SIGNATURE_RECEIVED");
      auth = await apiPost<AuthResponse>("/api/auth/login", {
        wallet: pubkey, signature: sig, timestamp: ts,
      });
    } else {
      const ts = Math.floor(Date.now() / 1000);
      const sig = await signRaw(`xfchess:register:${ts}`);
      logLifecycle("SIGNATURE_RECEIVED");
      auth = await apiPost<AuthResponse>("/api/auth/register", {
        wallet: pubkey, signature: sig, timestamp: ts,
        username: pubkey.slice(0, 8),
      });
    }

    // Invariants 1 and 2 from this function's doc comment land here. On (2):
    // `auth.username` is not yet the fully on-chain-aware answer — that is
    // `handleAuth`'s job, which posts its own resolved value once it has one —
    // but it is guaranteed to be about THIS wallet, unlike the old
    // localStorage read.
    // `provider` lets the game client tell an embedded wallet from an
    // extension. It gates the no-popup global-session flow, which is enabled
    // only for `privy` — see WalletProvider in tauri/src/main.rs.
    await apiPost("/wallet", { pubkey, username: auth.username, provider: kind });

    logLifecycle("TX_COMPLETE", { pubkey });
    onAuth(auth.token, auth.username, pubkey);
    onContinue(pubkey, src.provider ?? null);
  };

  /**
   * Provider errors (Phantom/Solflare, and Privy's) are usually plain
   * `{code, message}` objects rather than real Errors, so `console.error(e)`
   * alone often prints "Unexpected error" with no stack. Log every own property
   * so a failure is diagnosable from DevTools instead of only surfacing the
   * same generic string in the UI.
   */
  const reportFailure = (e: any) => {
    console.error("[WalletStep] connect failed:", e, JSON.stringify(e, Object.getOwnPropertyNames(e)));
    logLifecycle("FAILED", { message: e?.message || String(e) });
    setError(e?.message || String(e));
  };

  const handleConnect = async (walletName: "phantom" | "solflare") => {
    setError(null);
    setConnecting(walletName);
    try {
      logLifecycle("WALLET_CONNECT_START", { wallet: walletName });
      const src = await connectExtension(walletName);
      logLifecycle("WALLET_CONNECTED", { pubkey: src.pubkey });

      // Nudge cluster as early as possible in the session — the first automatic
      // signing popup for a fresh wallet is usually the global quick-sign
      // authorize (`authorize_global_session_if_needed` in
      // integration/systems.rs, which fires the moment a profile is detected),
      // so by the time that popup exists this has already had its one
      // best-effort chance to run instead of racing it.
      //
      // Extension-only: an embedded wallet has no user-selected cluster.
      await ensureDevnet(src.provider, walletName);

      await authenticateWithBackend(src);
    } catch (e: any) {
      reportFailure(e);
    } finally {
      setConnecting(null);
    }
  };

  const handlePrivyWallet = async (src: WalletSource) => {
    setError(null);
    setConnecting("privy");
    try {
      logLifecycle("WALLET_CONNECTED", { pubkey: src.pubkey, wallet: "privy" });
      await authenticateWithBackend(src);
    } catch (e: any) {
      reportFailure(e);
    } finally {
      setConnecting(null);
    }
  };

  const walletBtnStyle: CSSProperties = {
    width: "100%", padding: "16px 20px", borderRadius: 12, border: `1px solid ${BORDER}`,
    background: "rgba(255,255,255,0.03)", color: TEXT, fontSize: 15, fontWeight: 700,
    display: "flex", alignItems: "center", gap: 14, cursor: "pointer", transition: "all 0.2s",
  };

  return (
    <Card showClose={true} onClose={onClose}>
      <StepDots step="wallet" />
      <div style={{ textAlign: "center" as const, marginBottom: 28 }}>
        <h2 style={{ fontSize: 22, fontWeight: 800, fontFamily: "'Cinzel', serif", color: TEXT }}>
          Wallet Sign-In
        </h2>
        <p style={{ fontSize: 13, color: TEXT_DIM, marginTop: 4 }}>
          Verify ownership to access your account
        </p>
      </div>

      {error && <ErrorMsg msg={error} />}

      {PRIVY_ENABLED && (
        <SocialLoginBlock onWallet={handlePrivyWallet} busy={connecting !== null} />
      )}

      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        {(["phantom", "solflare"] as const).map((w) => {
          const meta = WALLET_META[w];
          const isInstalled = !!meta.provider();
          if (!isInstalled) {
            return (
              <a
                key={w}
                href={meta.installUrl}
                target="_blank"
                rel="noreferrer"
                style={{ ...walletBtnStyle, textDecoration: "none", opacity: 0.75, border: `1px dashed ${BORDER}` }}
                onMouseEnter={e => { (e.currentTarget as HTMLAnchorElement).style.borderColor = PRIMARY; (e.currentTarget as HTMLAnchorElement).style.opacity = "1"; }}
                onMouseLeave={e => { (e.currentTarget as HTMLAnchorElement).style.borderColor = BORDER; (e.currentTarget as HTMLAnchorElement).style.opacity = "0.75"; }}
              >
                <span style={{ fontSize: 20 }}>{meta.icon}</span>
                <span style={{ flex: 1, color: TEXT_DIM }}>{meta.label} - not installed</span>
                <span style={{ fontSize: 11, color: PRIMARY, fontWeight: 700 }}>Install</span>
              </a>
            );
          }
          return (
            <button
              key={w}
              style={walletBtnStyle}
              disabled={connecting !== null}
              onClick={() => handleConnect(w)}
              onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.borderColor = PRIMARY; (e.currentTarget as HTMLButtonElement).style.background = PRIMARY_DIM; }}
              onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.borderColor = BORDER; (e.currentTarget as HTMLButtonElement).style.background = "rgba(255,255,255,0.03)"; }}
            >
              <span style={{ fontSize: 20 }}>{meta.icon}</span>
              <span style={{ flex: 1 }}>{meta.label}</span>
              {connecting === w && <div style={{ width: 16, height: 16, border: `2px solid ${PRIMARY_BORDER}`, borderTop: `2px solid ${PRIMARY}`, borderRadius: "50%", animation: "spin 0.7s linear infinite" }} />}
            </button>
          );
        })}
      </div>

    </Card>
  );
}

// ---------------------------------------------------------------------------
// Splash — shown after login is complete
// ---------------------------------------------------------------------------
function SplashStep({ username, onComplete }: { username: string; onComplete: () => void }) {
  // Auto-close a couple seconds after showing the welcome message — the
  // game is already running, nothing further needs the popup open.
  useEffect(() => {
    const timer = setTimeout(() => {
      apiPost("/api/debug-log", { msg: `SplashStep: auto-closing after 2.5s, username="${username}"` }).catch(
        () => {},
      );
      onComplete();
    }, 2500);
    return () => clearTimeout(timer);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div style={{ textAlign: "center" as const, position: "relative" as const, zIndex: 1, animation: "fadeUp 0.5s ease" }}>
      <div style={{ marginBottom: 8 }}>
        <div style={{
          fontSize: 32, fontWeight: 900, fontFamily: "'Cinzel', serif",
          color: TEXT, letterSpacing: "0.1em",
        }}>XFCHESS</div>
      </div>
      <p style={{ fontSize: 14, color: TEXT_DIM, marginBottom: 24 }}>
        Welcome, <span style={{ color: TEXT, fontWeight: 600 }}>{username}</span>
      </p>
      <button
        onClick={onComplete}
        style={{
          padding: "14px 32px", borderRadius: 10, border: "none",
          background: "#ffffff",
          color: "#000000", fontSize: 15, fontWeight: 700, letterSpacing: "0.02em",
          cursor: "pointer", boxShadow: `0 4px 20px rgba(255,255,255,0.15)`,
          transition: "all 0.2s",
        }}
      >
        Continue
      </button>
    </div>
  );
}


// ---------------------------------------------------------------------------
// Background Transaction Signer
// ---------------------------------------------------------------------------
function TransactionSigner({ pubkey: _pubkey }: { pubkey: string }) {
  const [pendingTx, setPendingTx] = useState<string | null>(null);
  const [pendingLabel, setPendingLabel] = useState<string | null>(null);
  const [signing, setSigning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Privy hooks are safe to call unconditionally: PrivyProviderWrapper renders
  // a passthrough when VITE_PRIVY_APP_ID is unset, and these return empty
  // state rather than throwing.
  const { wallets: privyWallets } = useWallets();
  const { signTransaction: privySignTransaction } = useSignTransaction();
  const hasPrivyWallet = PRIVY_ENABLED && !!privyWallets[0]?.address;
  // Tracks which pending tx we've already auto-attempted, so the polling
  // effect doesn't re-fire signTransaction() every second while the user is
  // busy approving (or rejecting) it inside Phantom's own popup.
  const autoAttempted = useRef<string | null>(null);

  const resolveAndHide = async (signedB64: string) => {
    await fetch(`${API_BASE}/resolved`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Session-Id": SESSION_ID },
      body: JSON.stringify({ signed: signedB64 }),
    });
    sessionStorage.removeItem("xfchess_auto_attempted_tx");
    setPendingTx(null);
    setPendingLabel(null);
    setError(null);
    await closePopup();
  };

  // tauri_signer::sign_via_tauri_only (used by create_game and most other
  // signing calls) sends legacy `Transaction` bytes, not `VersionedTransaction`
  // — try versioned first since that's what most wallet-adapter code expects,
  // then fall back to legacy. Both branches used by every signing path here.
  const deserializeTx = (txBytes: Buffer): web3.VersionedTransaction | web3.Transaction => {
    try {
      return web3.VersionedTransaction.deserialize(txBytes);
    } catch {
      return web3.Transaction.from(txBytes);
    }
  };

  const signTxBytes = async (txB64: string, kp: web3.Keypair): Promise<string> => {
    const txBytes = Buffer.from(txB64, "base64");
    const tx = deserializeTx(txBytes);
    if (tx instanceof web3.VersionedTransaction) {
      tx.sign([kp]);
      return Buffer.from(tx.serialize()).toString("base64");
    }
    tx.partialSign(kp);
    return tx.serialize().toString("base64");
  };

  const handleAutoSign = async (txB64: string, secret: string) => {
    setSigning(true);
    try {
      const kp = web3.Keypair.fromSecretKey(new Uint8Array(JSON.parse(secret)));
      await resolveAndHide(await signTxBytes(txB64, kp));
    } catch (e: any) {
      setError(e.message);
    } finally {
      setSigning(false);
    }
  };

  // Signs via Privy's embedded wallet. Bytes in, bytes out — the same raw
  // transaction the Rust side serialized, straight back to the bridge — so this
  // path never has to reconcile Privy's `@solana/kit` types with this app's
  // `@solana/web3.js` ones.
  //
  // In practice this fires rarely for a social user: it covers the onboarding
  // transactions (`init_profile`, `authorize_global_session`) and any moment no
  // session key is active. Once a global session is authorized, gameplay goes
  // through `handleAutoSign` and never wakes this window at all.
  //
  // `ensureDevnet` is deliberately NOT called here: an embedded wallet has no
  // user-selected cluster to nudge, so there is nothing to correct and
  // `isNetworkMismatchError` is unreachable on this path.
  const signWithPrivy = async (txB64: string) => {
    setSigning(true);
    setError(null);
    try {
      const wallet = privyWallets[0];
      if (!wallet?.address) throw new Error("No Privy wallet available to sign with");

      const tx = deserializeTx(Buffer.from(txB64, "base64"));
      await refreshBlockhash(tx);

      logLifecycle("SIGN_REQUEST_START");
      const { signedTransaction } = await withTimeout(
        privySignTransaction({
          transaction: new Uint8Array(
            tx instanceof web3.VersionedTransaction ? tx.serialize() : tx.serialize({
              requireAllSignatures: false,
              verifySignatures: false,
            }),
          ),
          wallet,
          chain: SOLANA_CHAIN,
        }),
        60000,
        "Privy signature",
      );
      logLifecycle("SIGNATURE_RECEIVED");

      await resolveAndHide(Buffer.from(signedTransaction).toString("base64"));
      logLifecycle("TX_COMPLETE");
    } catch (e: any) {
      logLifecycle("FAILED", { message: e?.message || String(e) });
      setError(e?.message || String(e));
    } finally {
      setSigning(false);
    }
  };

  // Signs via the connected browser-extension wallet (Phantom/Solflare) —
  // this itself triggers the extension's own native popup. Called
  // automatically the moment a pending tx shows up (see the poll loop
  // below) so the user lands straight on Phantom's popup instead of having
  // to click "Sign with Extension" on this page first. The button stays as
  // a manual retry for when no provider was connected yet, or the user
  // dismissed/rejected the extension popup and wants to try again.
  const signWithExtension = async (txB64: string) => {
    setSigning(true);
    setError(null);
    try {
      const provider = getConnectedProvider();
      if (!provider) throw new Error("No Phantom/Solflare extension detected");
      // `getConnectedProvider()` only checks a persisted preference plus
      // whether the extension object exists on `window` — it says nothing
      // about whether *this* page/window actually has a live session with
      // it. Popups are real OS-level browser windows (see closePopup's
      // comment above), so a freshly (re)opened one hasn't run `.connect()`
      // in its own JS context yet even though the extension remembers this
      // origin as trusted — calling `signTransaction` straight away then
      // fails with "Not connected". Silently reconnect first, same as
      // `handleConnect` and `ProfileStep` already do.
      if (!provider.publicKey) {
        try {
          await withTimeout(provider.connect({ onlyIfTrusted: true }), 15000, "Wallet reconnect");
        } catch {
          await withTimeout(provider.connect(), 30000, "Wallet connection");
        }
      }
      const txBytes = Buffer.from(txB64, "base64");
      const tx = deserializeTx(txBytes);
      await refreshBlockhash(tx);
      await ensureDevnet(provider, localStorage.getItem("xfchess_wallet_provider"));
      logLifecycle("SIGN_REQUEST_START");
      const signed = await withTimeout<web3.VersionedTransaction | web3.Transaction>(
        provider.signTransaction(tx),
        60000,
        "Wallet signature",
      );
      logLifecycle("SIGNATURE_RECEIVED");
      await resolveAndHide(Buffer.from(signed.serialize()).toString("base64"));
      logLifecycle("TX_COMPLETE");
    } catch (e: any) {
      logLifecycle("FAILED", { message: e?.message || String(e) });
      setError(isNetworkMismatchError(e) ? NETWORK_MISMATCH_MESSAGE : e.message || String(e));
    } finally {
      setSigning(false);
    }
  };

  // Pushed via SSE instead of polled: the Tauri bridge emits the current
  // pending-tx state immediately on connect, then again the instant it
  // changes (see /pending/stream in tauri/src/main.rs), so a new signing
  // request is picked up right away instead of up to 1s later on average.
  // EventSource retries the connection natively on drop, so no manual
  // reconnect/backoff logic is needed here.
  useEffect(() => {
    const handleUpdate = (data: { tx?: string | null; label?: string | null }) => {
      if (data.tx && data.tx !== pendingTx) {
        setPendingTx(data.tx);
        setPendingLabel(typeof data.label === "string" && data.label ? data.label : null);
        const secret = sessionStorage.getItem("xfchess_session_key");
        // Persisted (not just the in-memory ref) so a popup reload mid-sign
        // — the exact "browser refresh during signing" case — doesn't
        // forget an attempt already made: the bridge's SSE stream re-emits
        // the SAME still-pending tx immediately on reconnect (see
        // get_pending_stream in tauri/src/main.rs), and a fresh mount's
        // useRef would read that as brand new, firing a second competing
        // signTransaction() call while the extension might already have a
        // native approval prompt open for the first one. Cleared once the
        // tx actually resolves (see resolveAndHide) or a fresh, different
        // tx shows up.
        const alreadyAttempted = sessionStorage.getItem("xfchess_auto_attempted_tx") === data.tx;
        if (secret) {
          handleAutoSign(data.tx, secret);
        } else if (
          (hasPrivyWallet || getConnectedProvider()) &&
          autoAttempted.current !== data.tx &&
          !alreadyAttempted
        ) {
          autoAttempted.current = data.tx;
          sessionStorage.setItem("xfchess_auto_attempted_tx", data.tx);
          // Privy first: if an embedded wallet is present it is definitionally
          // the wallet this session authenticated with, whereas
          // `getConnectedProvider()` only checks a persisted preference plus
          // whether an extension object exists on `window` — it can be true for
          // an extension that has nothing to do with the current session.
          if (hasPrivyWallet) signWithPrivy(data.tx);
          else signWithExtension(data.tx);
        }
      } else if (!data.tx) {
        sessionStorage.removeItem("xfchess_auto_attempted_tx");
        setPendingTx(null);
        setPendingLabel(null);
      }
    };

    const source = new EventSource(`${API_BASE}/pending/stream`);
    source.onmessage = (ev) => {
      try {
        handleUpdate(JSON.parse(ev.data));
      } catch (e) {
        console.warn("[SIGNER] Bad SSE payload", e);
      }
    };
    source.onerror = (e) => console.warn("[SIGNER] SSE connection error (will auto-retry)", e);
    return () => source.close();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pendingTx]);

  if (!pendingTx) return null;

  return (
    <div style={{
      position: "fixed", bottom: 20, right: 20, zIndex: 100,
      width: 300, padding: 20, background: CARD_BG, border: `1px solid ${PRIMARY_BORDER}`,
      borderRadius: 16, backdropFilter: "blur(20px)", animation: "fadeUp 0.3s ease",
    }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 12 }}>
        <div style={{ width: 10, height: 10, borderRadius: "50%", background: PRIMARY, animation: "pulse 1s infinite" }} />
        <span style={{ fontWeight: 800, fontSize: 13, color: TEXT }}>
          {pendingLabel ? pendingLabel.toUpperCase() : "PENDING TRANSACTION"}
        </span>
      </div>
      <p style={{ fontSize: 12, color: TEXT_DIM, marginBottom: 16 }}>
        {signing
          ? "Signing..."
          : pendingLabel
            ? `You're signing: ${pendingLabel}`
            : "Awaiting signature."}
      </p>
      {error && <ErrorMsg msg={error} />}
      {!signing && !sessionStorage.getItem("xfchess_session_key") && (
        hasPrivyWallet ? (
          <PrimaryBtn onClick={() => signWithPrivy(pendingTx)}>Sign</PrimaryBtn>
        ) : (
          <PrimaryBtn onClick={() => signWithExtension(pendingTx)}>Sign with Extension</PrimaryBtn>
        )
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 3 — Choose a username handle.
//
// Two invocations, distinguished by `requireOnchain`:
//  - Normal first-login (requireOnchain=false): off-chain handle only.
//    On-chain Solana profile creation stays deferred to first wager attempt.
//  - Deep-linked via `open_profile_step()`/`?step=profile` (requireOnchain=
//    true): the game client is blocking a wager on a missing on-chain
//    PlayerProfile, so this must actually submit the on-chain `init_profile`
//    transaction (via /api/auth/init-profile-tx + broadcast-tx), not just
//    PATCH the off-chain username — otherwise this popup resurfaces on every
//    future wager attempt, forever.
// ---------------------------------------------------------------------------
function ProfileStep({
  onComplete,
  onClose,
  defaultHandle = "",
  pubkey,
  walletProvider,
  requireOnchain = false,
}: {
  onComplete: (handle: string) => void;
  pubkey?: string | null;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  walletProvider?: any;
  onClose?: () => void;
  defaultHandle?: string;
  requireOnchain?: boolean;
}) {
  useEffect(() => {
    apiPost("/api/debug-log", {
      msg: `ProfileStep mounted — requireOnchain=${requireOnchain} defaultHandle="${defaultHandle}"`,
    }).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  // Privy hooks are safe to call unconditionally — PrivyProviderWrapper renders
  // a passthrough when VITE_PRIVY_APP_ID is unset, and these return empty state
  // rather than throwing. Read here rather than threaded down from the parent's
  // `walletProvider`, which is `null` for an embedded wallet by design (see
  // WalletSource.provider) and is also lost entirely when the popup is reopened
  // straight onto this step via the needs-profile-step flag.
  const { wallets: privyWallets } = useWallets();
  const { signTransaction: privySignTransaction } = useSignTransaction();
  const privyWallet = PRIVY_ENABLED ? privyWallets[0] : undefined;
  const signWithEmbedded =
    localStorage.getItem("xfchess_wallet_provider") === "privy" && !!privyWallet?.address;

  const [handle, setHandle] = useState(defaultHandle || localStorage.getItem("xfchess_username") || "");
  const [country, setCountry] = useState("");
  const [dob, setDob] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const countryValid = /^[A-Za-z]{2}$/.test(country.trim());
  const dobValid = !!dob;
  const canSubmit = handle.length >= 3 && (!requireOnchain || (countryValid && dobValid));

  const submit = async () => {
    if (!canSubmit) return;
    setSaving(true);
    setError(null);
    try {
      const token = localStorage.getItem("xfchess_token");
      if (requireOnchain) {
        const walletPubkey =
          pubkey ?? localStorage.getItem("xfchess_wallet_pubkey") ?? localStorage.getItem("xfchess_wallet");
        if (!walletPubkey) {
          throw new Error("No wallet connected in this window — reopen from the game client and try again.");
        }
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        let provider: any = null;
        if (signWithEmbedded) {
          // Guard against signing with the wrong key if Privy ever hands back a
          // different wallet than the one that authenticated.
          if (privyWallet!.address !== walletPubkey) {
            throw new Error(
              "Your signed-in wallet changed — close this window and sign in again.",
            );
          }
        } else {
          provider = walletProvider ?? getConnectedProvider();
          if (!provider) {
            throw new Error("No Phantom/Solflare extension detected in this window.");
          }
          if (!provider.publicKey) {
            try {
              await withTimeout(provider.connect({ onlyIfTrusted: true }), 15000, "Wallet reconnect");
            } catch {
              await withTimeout(provider.connect(), 30000, "Wallet connection");
            }
          }
        }

        if (!token) {
          throw new Error("Not signed in — reopen from the game client and try again.");
        }
        const dateOfBirth = Math.floor(new Date(`${dob}T00:00:00Z`).getTime() / 1000);
        const built = await apiPost<{ tx_b64: string; profile_pda: string }>(
          "/api/auth/init-profile-tx",
          {
            username: handle,
            country: country.trim().toUpperCase(),
            date_of_birth: dateOfBirth,
          },
          token,
        );

        const txBytes = Buffer.from(built.tx_b64, "base64");
        const tx = web3.Transaction.from(txBytes);

        // Up to 2 attempts total: `refreshBlockhash` already fetches as late
        // as possible (right before signing), but it can't account for how
        // long the user themselves takes to click "Approve" inside the
        // wallet extension — that delay happens strictly after the refresh,
        // so the freshly-fetched blockhash can still expire before broadcast
        // by the time a slower approval comes back. A stale-blockhash
        // rejection specifically (not a real one — insufficient funds,
        // program error, actual rejection) gets exactly one automatic retry:
        // fetch a new blockhash and ask for a fresh signature again, rather
        // than failing outright and forcing the player to close and restart
        // the whole ProfileStep form from scratch.
        const MAX_ATTEMPTS = 2;
        for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
          let signedB64: string;
          try {
            await refreshBlockhash(tx);
            logLifecycle("SIGN_REQUEST_START", {
              attempt,
              wallet: signWithEmbedded ? "privy" : "extension",
            });
            if (signWithEmbedded) {
              // `ensureDevnet` is deliberately skipped: an embedded wallet has
              // no user-selected cluster to nudge, so `isNetworkMismatchError`
              // is unreachable on this path.
              const { signedTransaction } = await withTimeout(
                privySignTransaction({
                  transaction: new Uint8Array(
                    tx.serialize({ requireAllSignatures: false, verifySignatures: false }),
                  ),
                  wallet: privyWallet!,
                  chain: SOLANA_CHAIN,
                }),
                60000,
                "Privy signature",
              );
              signedB64 = Buffer.from(signedTransaction).toString("base64");
            } else {
              await ensureDevnet(provider, localStorage.getItem("xfchess_wallet_provider"));
              const signed = await withTimeout<web3.Transaction>(
                provider.signTransaction(tx),
                60000,
                "Wallet signature",
              );
              signedB64 = Buffer.from(signed.serialize()).toString("base64");
            }
            logLifecycle("SIGNATURE_RECEIVED");
          } catch (e: any) {
            if (isNetworkMismatchError(e)) throw new Error(NETWORK_MISMATCH_MESSAGE);
            throw new Error("Signature rejected — try again to finish on-chain setup.");
          }
          try {
            await apiPost<{ signature: string }>(
              "/api/auth/broadcast-tx",
              { tx_b64: signedB64 },
              token,
            );
            logLifecycle("TX_COMPLETE", { attempt });
            break;
          } catch (e: any) {
            if (isStaleBlockhashError(e) && attempt < MAX_ATTEMPTS) {
              apiPost("/api/debug-log", {
                msg: `ProfileStep broadcast-tx: stale blockhash on attempt ${attempt}, retrying with a fresh signature`,
              }).catch(() => {});
              continue;
            }
            throw e;
          }
        }
      }

      if (token) {
        if (requireOnchain) {
          // The on-chain init_profile submitted above already set this exact
          // handle as PlayerProfile.username — PATCH /auth/username now
          // rejects with 409 once an on-chain username is set (it would
          // otherwise write a redundant off-chain copy that could later
          // diverge from the on-chain value on some future rename, which
          // neither surface would ever display again — see the backend's
          // doc comment on `set_username`). Force the SQLite mirror via
          // sync-profile instead, which reads the value we just wrote
          // on-chain rather than re-asserting it off-chain.
          await fetchProfileStatus(token).catch(() => { /* best-effort mirror */ });
        } else {
          const r = await fetch(`${API_BASE}/api/auth/username`, {
            method: "PATCH",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${token}`,
              "X-Session-Id": SESSION_ID,
            },
            body: JSON.stringify({ username: handle }),
          });
          if (!r.ok) throw new Error(await r.text().catch(() => "Failed to save username"));
        }
      }
      localStorage.setItem("xfchess_username", handle);
      onComplete(handle);
    } catch (e: any) {
      setError(e.message || String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card showClose={true} onClose={onClose}>
      <StepDots step="profile" />
      <div style={{ textAlign: "center" as const, marginBottom: 28 }}>
        <h2 style={{ fontSize: 22, fontWeight: 800, fontFamily: "'Cinzel', serif", color: TEXT }}>
          Choose Your Handle
        </h2>
        <p style={{ fontSize: 13, color: TEXT_DIM, marginTop: 4 }}>
          {requireOnchain
            ? "Confirm your details to create your on-chain profile"
            : "Pick a display name for the arena"}
        </p>
      </div>
      {error && <ErrorMsg msg={error} />}
      <InputField label="Chess Handle" value={handle} onChange={setHandle} placeholder="e.g. DragonKnight99" />
      {requireOnchain && (
        <>
          <InputField label="Country (2-letter code)" value={country} onChange={(v) => setCountry(v.toUpperCase())} placeholder="e.g. GB" />
          <InputField label="Date of Birth" value={dob} onChange={setDob} type="date" />
        </>
      )}
      <p style={{ fontSize: 11, color: TEXT_MUTED, textAlign: "center" as const, marginBottom: 16 }}>
        {requireOnchain
          ? "This submits a one-time on-chain transaction to create your Solana profile."
          : "Your handle is saved to your account. On-chain Solana setup happens when you first wager."}
      </p>
      <PrimaryBtn
        onClick={submit}
        loading={saving}
        disabled={!canSubmit}
        style={{ marginTop: 4 }}
      >
        {requireOnchain ? "Create Profile & Continue" : "Save & Enter Arena"}
      </PrimaryBtn>
    </Card>
  );
}


// ---------------------------------------------------------------------------
// Root orchestrator
// ---------------------------------------------------------------------------
function Onboarding() {
  // A signing request (see tauri/src/main.rs's open_wallet_popup_for_signing)
  // reopens this popup from scratch — a brand new page load with no React
  // state carried over from whatever window handled the original login. If
  // there's already a session on disk, `?step=sign` must skip straight past
  // consent/entry/wallet/profile so the pending-transaction prompt (rendered
  // unconditionally below via <TransactionSigner>) is what the user actually
  // sees, instead of being asked to log in or pick a handle all over again.
  const hasExistingSession = () =>
    !!(localStorage.getItem("xfchess_wallet_pubkey") || localStorage.getItem("xfchess_wallet"));

  // Computed once, at mount — a reconnect (skip Wallet-Sign-In's manual
  // button click, skip the Splash screen) vs. a genuine first-time login
  // (show both, they're the only feedback the user gets that anything
  // happened). Must not flip mid-flow: a brand-new login writes the same
  // localStorage keys hasExistingSession() reads, so re-evaluating it after
  // handleAuth runs would wrongly reclassify a first-timer as "returning"
  // for the rest of this same session.
  const [wasReturningSession] = useState<boolean>(hasExistingSession);

  const [step, setStep] = useState<Step>(() => {
    const params = new URLSearchParams(window.location.search);
    const s = params.get("step");
    if (s === "connect_wallet") return "wallet";
    if (s === "profile") return "profile";
    // No legal/consent gate on devnet — every entry point (fresh login,
    // returning session, or a signing deep link) goes straight to "wallet"
    // so Connect Wallet always lands directly on Phantom/Solflare.
    if (s === "sign") return "sign";
    return "wallet";
  });
  // Only the `?step=profile` deep link (opened by open_profile_step() when
  // the game client is blocking a wager on a missing on-chain profile) needs
  // to actually submit the on-chain init_profile tx here — the normal
  // first-login path reaches "profile" via handleAuth/handleWalletContinue
  // with no such param, and stays off-chain-only by design.
  //
  // This is mutable (not the one-shot `useState` it used to be) because the
  // URL alone can't be trusted as the ongoing signal: `open_profile_step`
  // re-shows an *already-open* popup window without navigating it (see
  // `open_in_browser`'s reuse path in tauri/src/main.rs) specifically so a
  // signing request doesn't cost a full respawn — which means a popup that
  // was first opened via a plain `?sid=...` URL and is later re-shown for a
  // `?step=profile` request never actually sees that URL change.
  // `handleAuth` and the needs-profile-step poll below both flip this to
  // `true` directly (via `setRequireOnchain`) once they learn — from the
  // server's one-shot `needs_profile_step` flag, not the stale URL — that
  // *this* session needs the on-chain submission, regardless of what the
  // popup's address bar still says.
  const [requireOnchain, setRequireOnchain] = useState<boolean>(
    () => new URLSearchParams(window.location.search).get("step") === "profile",
  );
  const [username, setUsername] = useState<string>(
    () => localStorage.getItem("xfchess_username") || "Player",
  );
  const [ready, setReady] = useState(false);
  const [pubkey, setPubkey] = useState<string | null>(
    () => localStorage.getItem("xfchess_wallet_pubkey") || localStorage.getItem("xfchess_wallet"),
  );
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [walletProvider, setWalletProvider] = useState<any>(null);

  // No consent gate to check on devnet (see `step` above) — nothing left to
  // await before rendering. `ready` stays purely to gate the spinner frame
  // that used to cover this async check.
  useEffect(() => {
    setReady(true);
    // Tells Tauri this page has actually mounted and can respond to
    // anything — see api_ready/mark_session_ready in tauri/src/main.rs.
    // Before this, the only readiness signal was the OS window-title poll,
    // which proves the Chrome window exists but says nothing about whether
    // React has taken over the page yet.
    logLifecycle("REACT_READY");
    apiPost("/api/ready", { sid: SESSION_ID }).catch(() => { /* bridge unreachable — nothing to report to */ });
    // A fresh log line every time this effect runs, i.e. every time the App
    // component actually mounts — proves whether a given popup show/hide
    // cycle was a real reuse (no new mount) or a hidden respawn (React state
    // reset even though the OS-level window handle looked "reused").
    apiPost("/api/debug-log", {
      msg: `App mounted — initial step="${step}", url="${window.location.href}"`,
    }).catch(() => {});
    // Surfaces which backend this session is actually talking to, in this
    // page's own console, so a prod/local mismatch (see get_backend_url's
    // doc comment in tauri/src/main.rs) is visible here instead of only
    // showing up as an unexplained 502 later.
    apiGet<{ url: string; explicit: boolean }>("/api/backend-url")
      .then((r) => logLifecycle("BACKEND_TARGET", r))
      .catch(() => { /* bridge unreachable */ });
  }, []);

  // Fallback when this popup has no wallet in its own localStorage yet the
  // bridge's in-memory /status already knows one (set by the earlier
  // Connect Wallet popup's POST /wallet) — this is exactly the gap that
  // left TransactionSigner permanently un-rendered (it's gated on `pubkey`)
  // even though a real pending signature was sitting in /pending the whole
  // time: nothing on screen ever showed a way to approve it. Whatever the
  // root cause of the popup not sharing localStorage this time around
  // (fresh process, cleared storage, etc.), the bridge's own state is the
  // one thing that's always current for *this* connected wallet, so fall
  // back to asking it directly instead of assuming an empty localStorage
  // means "no wallet connected".
  useEffect(() => {
    if (pubkey) return;
    apiGet<{ connected: boolean; pubkey: string | null }>("/status")
      .then((s) => {
        if (s.pubkey) {
          localStorage.setItem("xfchess_wallet_pubkey", s.pubkey);
          setPubkey(s.pubkey);
        }
      })
      .catch(() => { /* bridge unreachable — nothing to fall back to */ });
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Poll for profile-step requests from the game client (e.g. "Wagered PVP" clicked).
  //
  // Used to only run while `step === "splash"`, on the assumption that's the
  // only state a popup could be sitting idle in. Real bug this caused: a
  // returning-session `handleAuth` call that resolves an existing off-chain
  // username closes the popup directly (see the `needsProfile === false`
  // branch above) WITHOUT ever moving `step` off its initial `"wallet"` —
  // there was no reason to, since at that moment nothing needed ProfileStep.
  // If the game client's `open_profile_step()` request (setting this same
  // flag) arrives *after* that — a real, common race, since the game client
  // only learns it needs an on-chain profile once the wallet is already
  // connected — the flag gets set correctly, but this poll was never running
  // (step stuck at "wallet", not "splash"), so it just sat there forever,
  // unread, and every future re-show of the reused popup landed back on the
  // plain Wallet Sign-In screen with no way to ever reach ProfileStep again.
  // Only genuinely unsafe to run this poll during "sign" (an in-progress
  // transaction signature shouldn't be yanked away mid-flow); every other
  // step is safe to redirect out of the instant the flag says to.
  useEffect(() => {
    if (step === "sign") return;
    const interval = setInterval(async () => {
      try {
        const r = await apiGet<{ needs_profile: boolean }>("/api/needs-profile-step");
        if (r.needs_profile) {
          apiPost("/api/debug-log", {
            msg: `needs-profile-step poll: flag was set (was on step="${step}") — setStep(profile), requireOnchain=true`,
          }).catch(() => {});
          // Same fix as handleAuth's flag check: this poll only running means
          // the game client specifically needs the on-chain profile created,
          // not just the off-chain handle this session may already have —
          // without this, ProfileStep would render in its off-chain-only mode
          // and never actually submit init_profile.
          setRequireOnchain(true);
          setStep("profile");
        }
      } catch { /* ignore — bridge may not be running */ }
    }, 1500);
    return () => clearInterval(interval);
  }, [step]);

  // The `?step=profile` deep link (see requireOnchain above) jumps straight to
  // ProfileStep without ever going through handleAuth, so `username` here is
  // whatever was last cached in this browser's localStorage — potentially
  // from a completely different wallet that was logged into on this machine
  // at some earlier point (localStorage for this popup is intentionally the
  // user's real, persistent Chrome profile, not an isolated one — see
  // tauri/src/main.rs's kill_wallet_popup comment). Re-resolve against the
  // backend for the wallet that's ACTUALLY active right now before letting
  // ProfileStep prefill the handle field with it, so a stale name from a
  // prior wallet never gets submitted into a new on-chain profile. Gated on
  // `handleResolved` (not just re-running on every render) because
  // ProfileStep seeds its own input state from `defaultHandle` exactly once,
  // at mount — updating `username` after it has already mounted wouldn't
  // reach the visible field, so ProfileStep must not mount until this
  // resolves (see the `handleResolved` check around its render below).
  const [handleResolved, setHandleResolved] = useState(!requireOnchain);
  useEffect(() => {
    if (step !== "profile" || !requireOnchain || handleResolved) return;
    const token = localStorage.getItem("xfchess_token");
    const activePubkey = pubkey ?? localStorage.getItem("xfchess_wallet_pubkey") ?? localStorage.getItem("xfchess_wallet");
    if (!token || !activePubkey) { setHandleResolved(true); return; }
    (async () => {
      try {
        const status = await fetchProfileStatus(token);
        const existing = await resolveExistingUsername(token, activePubkey, status);
        if (existing) {
          setUsername(existing);
          localStorage.setItem("xfchess_username", existing);
        } else {
          localStorage.removeItem("xfchess_username");
          setUsername("Player");
        }
      } catch { /* backend unreachable — fall back to whatever was cached */ }
      finally { setHandleResolved(true); }
    })();
  }, [step, requireOnchain, handleResolved, pubkey]);

  const handleAuth = async (token: string, user: string, nextPubkey: string) => {
    localStorage.setItem("xfchess_token", token);
    localStorage.setItem("xfchess_wallet_pubkey", nextPubkey);
    setPubkey(nextPubkey);
    // Push JWT to bridge so the game client can pick it up via GET /token
    apiPost("/token", { token }).catch(() => {});

    // `user` here may just be the throwaway pubkey-slice placeholder
    // WalletStep sends as a required-but-unchosen value on first
    // registration (see handleConnect's register call) — never treat it
    // as a real display name directly.
    //
    // There used to be a "fast path" here that skipped straight to
    // closePopup/splash whenever `user` (the off-chain username /api/auth/login
    // returns) was already a real, non-placeholder value — on the assumption
    // that an off-chain username implies the player is fully set up. That
    // assumption is wrong: a player can have an off-chain username (set by a
    // prior ProfileStep attempt) while the ON-CHAIN PlayerProfile was never
    // actually created — e.g. if the on-chain init_profile transaction failed
    // partway (auth/blockhash issues, rejected signature, closed popup mid-flow).
    // The fast path never checked on-chain state at all, so it closed the
    // popup immediately every time, the player never saw ProfileStep again,
    // and the on-chain profile was permanently stuck at "missing" — a wallet
    // in that state could never complete setup again. Confirmed live via
    // debug-log instrumentation: `user="val"`, a real off-chain username, was
    // hitting the fast path on every single reconnect while the game client
    // kept reporting NoProfile forever. Always resolving through the
    // on-chain-aware path below (same one that already existed for the
    // placeholder case) is the only version of this check that's actually
    // correct — the extra network round trip is a fully acceptable cost
    // compared to silently soft-locking an account.

    // resolveExistingUsername checks both the on-chain PlayerProfile
    // (sync-profile) and the off-chain account username (auth/me, set by a
    // prior ProfileStep completion that hasn't been followed by a wager
    // yet), excluding that same placeholder — so a returning player with a
    // chosen handle but no on-chain profile isn't asked to pick a new one.
    // That's the right behavior for an ordinary reconnect (on-chain profile
    // creation is deliberately deferred to first-wager-attempt by design —
    // see ProfileStep's module doc). It is NOT the right behavior when the
    // game client specifically opened this popup because it's blocking a
    // wager on that exact missing on-chain profile — closing the popup here
    // would silently strand the player in the exact soft-locked state this
    // fix exists for. `needsOnchainRightNow` below is the one-shot
    // server-side signal (`s.needs_profile_step`, set by
    // `POST /api/open-profile-step`) that distinguishes the two: unlike the
    // popup's own URL, it reflects what THIS specific open was actually for,
    // regardless of whether the window was freshly navigated or reused.
    let resolvedUser = user;
    let needsProfile = true;
    try {
      const status = await fetchProfileStatus(token);
      const existing = await resolveExistingUsername(token, nextPubkey, status);
      if (existing) {
        resolvedUser = existing;
        localStorage.setItem("xfchess_username", resolvedUser);
        setUsername(resolvedUser);
        needsProfile = false;
      }
      if (needsProfile === false && !status.username_set) {
        const flagResp = await apiGet<{ needs_profile: boolean }>("/api/needs-profile-step");
        if (flagResp.needs_profile) {
          apiPost("/api/debug-log", {
            msg: "handleAuth: off-chain username resolved, but needs-profile-step flag was set — forcing ProfileStep(requireOnchain)",
          }).catch(() => {});
          setRequireOnchain(true);
          needsProfile = true;
        }
      }
    } catch {
      // The on-chain lookup itself failed (e.g. a flaky devnet RPC call inside
      // sync-profile) — that says nothing about whether this wallet actually
      // has a profile. `login`/`register` (just above, in
      // WalletStep.handleConnect) already returned a real username for this
      // exact pubkey; falling through to needsProfile=true unconditionally
      // here re-shows "Choose Your Handle" to an already-registered player
      // every time devnet RPC hiccups, even though their pubkey checked out
      // fine. Trust that already-known username instead, unless it's the
      // throwaway registration placeholder (see resolveExistingUsername).
      const registrationPlaceholder = nextPubkey.slice(0, 8);
      if (user && user !== registrationPlaceholder) {
        resolvedUser = user;
        localStorage.setItem("xfchess_username", resolvedUser);
        setUsername(resolvedUser);
        needsProfile = false;
      }
    }

    // Push the fully-resolved (on-chain + off-chain aware) answer back to
    // the bridge — `handleConnect`'s earlier POST /wallet only had the raw
    // login/register response to go on, not this deeper check. Explicitly
    // clearing to "" when `needsProfile` is true matters just as much as
    // setting the real value when it's false: without it, a stale username
    // from an earlier wallet/session that happened to still be sitting in
    // the bridge's cache (see WalletStep.handleConnect's doc comment on the
    // exact bug this closes) would keep being shown as "already known" by
    // the game client's poller while this popup is correctly asking for a
    // fresh handle — the two would visibly disagree, which is exactly what
    // made this bug obvious from the player's side.
    apiPost("/wallet", { pubkey: nextPubkey, username: needsProfile ? "" : resolvedUser }).catch(
      () => {},
    );

    if (needsProfile) {
      apiPost("/api/debug-log", { msg: "handleAuth: needsProfile=true — setStep(profile)" }).catch(
        () => {},
      );
      // No real username yet — make sure nothing (this session's state,
      // or a stale value from a previous wallet's session) pre-fills the
      // handle field with something that looks chosen but isn't. Clearing
      // localStorage alone isn't enough: `username` state was already read
      // from it at mount time, and ProfileStep's defaultHandle prop is
      // derived from that stale in-memory value, not from localStorage
      // again — so it must be reset here too, or the old value leaks
      // straight through into the "Choose Your Handle" field.
      localStorage.removeItem("xfchess_username");
      setUsername("Player");
      setStep("profile");
    } else {
      apiPost("/api/debug-log", {
        msg: `handleAuth: needsProfile=false, resolvedUser="${resolvedUser}" — ${wasReturningSession ? "closePopup" : "setStep(splash)"}`,
      }).catch(() => {});
      handleGameLaunch(nextPubkey, resolvedUser);
      if (wasReturningSession) {
        closePopup();
      } else {
        setStep("splash");
      }
    }
  };

  const handleWalletContinue = (pk: string, provider: any) => {
    localStorage.setItem("xfchess_wallet", pk);
    setPubkey(pk);
    setWalletProvider(provider);
    // Do NOT set step here. `WalletStep.handleConnect` calls `onAuth(...)`
    // immediately followed by `onContinue(...)` (this function) — always,
    // unconditionally, every connect. `handleAuth` already routes to
    // "splash" (known username, most of the fast synchronous path) or
    // "profile" (no username yet) correctly; a `setStep("profile")` here
    // used to run right after and clobber that decision back to "profile"
    // every single time, since both calls land in the same synchronous
    // tick when `handleAuth` takes its fast path. That's why a wallet with
    // an already-known handle still got asked "Choose Your Handle" on every
    // reconnect — this function's only remaining job is the wallet/provider
    // bookkeeping above.
  };

  const handleProfileComplete = (handle: string) => {
    setUsername(handle);
    setStep("splash");
    handleGameLaunch(pubkey || "dummy", handle);
  };

  const handleGameLaunch = async (pk: string, user: string) => {
    const token = localStorage.getItem("xfchess_token");
    try {
      // "hot" (guest/local-keypair) launches no longer exist — every launch
      // is backed by a real wallet that signed to prove ownership.
      await apiPost("/api/game/launch", { pubkey: pk, hot: false, username: user, token });
    } catch (e) {
      console.error("[API] launch_game failed:", e);
    }
  };


  if (!ready) {
    return (
      <div style={{ ...page }}>
        <GridBg />
        <SiteNav />
        <div style={{ width: 24, height: 24, border: `2px solid ${RED_BORDER}`, borderTop: `2px solid ${RED}`, borderRadius: "50%", animation: "spin 0.8s linear infinite" }} />
      </div>
    );
  }

  return (
    <div style={{ ...page }}>
      <GridBg />
      <SiteNav />

      {step === "wallet"  && <WalletStep
        onContinue={handleWalletContinue}
        onAuth={handleAuth}
        onClose={closePopup}
      />}

      {step === "profile" && !handleResolved && (
        <Card showClose={true} onClose={closePopup}>
          <div style={{ textAlign: "center" as const, padding: "20px 0" }}>
            <div style={{ width: 24, height: 24, margin: "0 auto", border: `2px solid ${RED_BORDER}`, borderTop: `2px solid ${RED}`, borderRadius: "50%", animation: "spin 0.8s linear infinite" }} />
          </div>
        </Card>
      )}
      {step === "profile" && handleResolved && (
        <ProfileStep
          onComplete={handleProfileComplete}
          pubkey={pubkey}
          walletProvider={walletProvider}
          onClose={closePopup}
          defaultHandle={username !== "Player" ? username : undefined}
          requireOnchain={requireOnchain}
        />
      )}

      {/* Game is already running — auto-close shortly after showing the
          welcome message; "View Profile Hub" also closes immediately. */}
      {step === "splash"  && <SplashStep username={username} onComplete={closePopup} />}

      {/* Reopened purely to approve a pending transaction (see hasExistingSession
          above) — no login walkthrough, just wait for <TransactionSigner> below
          to pick up the pending tx from /pending and show the sign prompt. */}
      {step === "sign" && (
        <Card showClose={true} onClose={closePopup}>
          <div style={{ textAlign: "center" as const }}>
            <LogoMark size={40} />
            <p style={{ fontSize: 13, color: TEXT_DIM, marginTop: 16 }}>
              Approve the pending transaction below to continue.
            </p>
          </div>
        </Card>
      )}

      {pubkey && <TransactionSigner pubkey={pubkey} />}
    </div>
  );
}

// ---------------------------------------------------------------------------
// App root (no wallet adapter library — direct connections only)
// ---------------------------------------------------------------------------
export default function App() {
  return (
    <>
      <style>{KEYFRAMES}</style>
      <Onboarding />
    </>
  );
}

