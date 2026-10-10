import { useState, useEffect, useRef, type CSSProperties } from "react";
import bs58 from "bs58";
import { usePrivy, useLogin, useLogout } from "@privy-io/react-auth";
import { useWallets, useSignMessage, useSignTransaction, useCreateWallet } from "@privy-io/react-auth/solana";
import type { WalletSource } from "./wallet/types";
import { connectExtension } from "./wallet/extension";
import { privyWalletSource } from "./wallet/privy";
import { PRIVY_ENABLED, SOLANA_CHAIN } from "./privy/config";

// Production popups use their page port to address the owning bridge. Vite
// dev pages need an explicit bridge override because their port belongs to Vite.
const BRIDGE_PORT = import.meta.env.DEV
  ? (import.meta.env.VITE_BRIDGE_PORT ?? "7454")
  : (window.location.port || "7454");
// Call the IPv4-only bridge at 127.0.0.1 to avoid failed ::1 attempts.
// The popup page origin stays localhost for wallet and Privy trust.
const API_BASE = `http://127.0.0.1:${BRIDGE_PORT}`;

// Include the bridge port in the window title; Rust popup lookup uses
// the same format to distinguish concurrent instances.
document.title = `XFChess #${BRIDGE_PORT}`;

// Bound injected provider calls because broken extension background relays
// can leave promises pending indefinitely.
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

// Read sid from the popup URL and attach X-Session-Id to bridge calls.
// Direct dev loads mint a local fallback ID.
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

// Ask the bridge to close its popup; Chrome may reject window.close for
// windows not opened by script.
async function closePopup() {
  try {
    const pending = await fetch(`${API_BASE}/pending`, {
      headers: { "X-Session-Id": SESSION_ID },
    });
    if (pending.ok) {
      const state = await pending.json() as { request_id?: string | null };
      if (state.request_id) {
        await fetch(`${API_BASE}/cancel`, {
          method: "POST",
          headers: { "Content-Type": "application/json", "X-Session-Id": SESSION_ID },
          body: JSON.stringify({ request_id: state.request_id, reason: "Wallet window closed" }),
        });
      }
    }
    await fetch(`${API_BASE}/hide`, { method: "POST", headers: { "X-Session-Id": SESSION_ID } });
  } catch {
    window.close();
  }
}

// Sign through the same extension used for authentication, even when
// multiple providers are installed.
export function getConnectedProvider(expectedKind?: string | null): any {
  const kind = expectedKind ?? localStorage.getItem("xfchess_wallet_provider");
  if (kind === "solflare") return (window as any).solflare;
  if (kind === "phantom") return (window as any).phantom?.solana;
  // Embedded-wallet sessions must use Privy; extension fallback would select
  // a different signing key.
  if (kind === "privy") return null;
  // Unknown (e.g. state from before this was tracked) — fall back to the old
  // best-effort behavior rather than refusing to sign at all.
  return (window as any).phantom?.solana ?? (window as any).solflare;
}

/// Best-effort switch the extension wallet to devnet before signing.
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

// Wallets may report a cluster mismatch when their finalized blockhash lookup
// cannot find a newer blockhash. Wallet-signed transactions use finalized
// blockhashes; retain this detection for actual network mismatches or lagging RPCs.
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

// Pass Bearer auth for proxied wallet-protected routes; local bridge routes
// may omit the token.
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

// Retry stale-blockhash rejection separately from funds, program, network,
// or user-rejection errors; approval may outlast a freshly fetched hash.
export function isStaleBlockhashError(e: any): boolean {
  const msg = String(e?.message ?? e ?? "").toLowerCase();
  return msg.includes("blockhash not found") || msg.includes("-32002");
}

/**
 * Refresh the blockhash in place immediately before signing. On fetch
 * failure, leave the transaction untouched for the caller's retry/error path.
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
    // Log refresh failure while preserving best-effort signing with the existing hash.
    apiPost("/api/debug-log", { msg: `refreshBlockhash: threw ${e?.message || e}` }).catch(() => {});
    return false;
  }
}

/** On-chain username setup determines the profile step; KYC is checked at wager time. */
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
 * Resolve a chosen handle from on-chain profile or off-chain auth/me.
 * Exclude registration pubkey-slice placeholders; an unwagered user may have
 * a real off-chain handle before on-chain setup.
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

type Step = "wallet" | "profile" | "splash" | "sign";

interface AuthResponse {
  token: string;
  username: string;
  wallet?: string;
}

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

const page: CSSProperties = {
  width: "100vw", minHeight: "100vh", display: "flex", flexDirection: "column",
  alignItems: "center", justifyContent: "center", background: BG,
  position: "relative", overflowY: "auto", padding: "24px 0",
};

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

import * as web3 from "@solana/web3.js";

/**
 * After explicit Google login, pass its embedded wallet through the same
 * signature-verified backend auth flow as extension wallets.
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
   * Persisted Privy authentication must not trigger signing until Google is clicked.
   * Use a counter so repeated clicks retrigger the effects and permit retries.
   */
  const [attempt, setAttempt] = useState(0);
  const chosen = attempt > 0;

  // Hand over each embedded address once; rerenders must not start duplicate login/signing flows.
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
   * Use linkedAccounts to determine wallet existence; useWallets can be empty
   * during hydration and must not trigger duplicate provisioning.
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
   * Provision with createWallet so failures are surfaced and retryable,
   * including authenticated sessions whose login flow created no wallet.
   */
  const provision = async () => {
    if (provisioning) return;
    setProvisioning(true);
    setError(null);
    logLifecycle("PRIVY_CREATE_WALLET_START");
    try {
      await withTimeout(createWallet(), 45000, "Google wallet setup");
      logLifecycle("PRIVY_CREATE_WALLET_OK");
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      logLifecycle("PRIVY_CREATE_WALLET_FAILED", { error: msg });
      setError(`Could not create your wallet: ${msg}`);
    } finally {
      setProvisioning(false);
    }
  };

  // Repair authenticated sessions missing a wallet without requiring another login click.
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

  // Show wallet hydration status while Privy reports a wallet but useWallets is still empty.
  const awaitingWallet =
    chosen && authenticated && hasEmbeddedWallet && !wallets[0]?.address;

  return (
    <div style={{ marginBottom: 20 }}>
      {error && <ErrorMsg msg={error} />}

      <button
        style={btnStyle}
        disabled={!ready || busy || provisioning}
        onClick={() => {
          // The recorded choice enables effects to resume a persisted session without another Google login.
          setAttempt((n) => n + 1);
          // Clear hand-off suppression on explicit retry. The busy state prevents
          // a second concurrent hand-off.
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
   * Prove wallet ownership with a signature before POST /wallet. Use the username
   * from this auth response: the popup profile and localStorage are shared across wallets.
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

    // Post this auth response's username and provider; handleAuth later refines
    // the profile-aware name, and provider gates embedded-session setup.
    await apiPost("/wallet", { pubkey, username: auth.username, provider: kind });

    logLifecycle("TX_COMPLETE", { pubkey });
    onAuth(auth.token, auth.username, pubkey);
    onContinue(pubkey, src.provider ?? null);
  };

  /**
   * Log provider error properties; SDK failures may be plain objects rather
   * than Error instances with stacks.
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

      // Attempt extension cluster switching before automatic signing. Embedded
      // wallets have no selected cluster to change.
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


function TransactionSigner({ pubkey: _pubkey }: { pubkey: string }) {
  const [pendingTx, setPendingTx] = useState<string | null>(null);
  const [pendingLabel, setPendingLabel] = useState<string | null>(null);
  const [currentRequestId, setCurrentRequestId] = useState<string | null>(null);
  const [signing, setSigning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Privy hooks return empty state safely when the provider wrapper is a passthrough.
  const { ready: privyReady, authenticated: privyAuthenticated } = usePrivy();
  const { wallets: privyWallets } = useWallets();
  const { signTransaction: privySignTransaction } = useSignTransaction();
  const hasPrivyWallet = PRIVY_ENABLED && !!privyWallets[0]?.address;
  const [bridgeProvider, setBridgeProvider] = useState<string | null>(null);
  const [bridgeStatusLoaded, setBridgeStatusLoaded] = useState(false);
  const isPrivySession = bridgeProvider === "privy";
  // Attempt each pending transaction once to avoid duplicate wallet prompts during polling.
  const autoAttempted = useRef<string | null>(null);

  // Read provider identity from the bridge; stale localStorage could route signing to the wrong wallet.
  useEffect(() => {
    let cancelled = false;
    void withTimeout(apiGet<{ pubkey?: string; provider?: string | null }>("/status"), 5000, "Wallet session lookup")
      .then((status) => {
        if (cancelled) return;
        if (status.pubkey && status.pubkey !== _pubkey) {
          setError("The wallet session changed. Close this window and reopen the signing request.");
          return;
        }
        setBridgeProvider(status.provider ?? null);
        setBridgeStatusLoaded(true);
      })
      .catch((e: any) => {
        if (!cancelled) setError(e?.message || String(e));
      });
    return () => { cancelled = true; };
  }, [_pubkey]);

  const cancelRequest = async (requestId: string, reason: string) => {
    const response = await fetch(`${API_BASE}/cancel`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Session-Id": SESSION_ID },
      body: JSON.stringify({ request_id: requestId, reason }),
    });
    if (!response.ok && response.status !== 409) {
      throw new Error(`Wallet bridge rejected cancellation (${response.status})`);
    }
  };

  const resolveAndHide = async (requestId: string, signedB64: string) => {
    const response = await fetch(`${API_BASE}/resolved`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Session-Id": SESSION_ID },
      body: JSON.stringify({ request_id: requestId, signed: signedB64 }),
    });
    if (!response.ok) throw new Error(`Wallet bridge rejected signing response (${response.status})`);
    sessionStorage.removeItem("xfchess_auto_attempted_tx");
    setPendingTx(null);
    setPendingLabel(null);
    setCurrentRequestId(null);
    setError(null);
    await closePopup();
  };

  // Accept versioned and legacy transaction bytes; tauri_signer sends legacy transactions.
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

  const handleAutoSign = async (requestId: string, txB64: string, secret: string) => {
    setSigning(true);
    try {
      const kp = web3.Keypair.fromSecretKey(new Uint8Array(JSON.parse(secret)));
      await resolveAndHide(requestId, await signTxBytes(txB64, kp));
    } catch (e: any) {
      await cancelRequest(requestId, e.message).catch(() => {});
      setError(e.message);
    } finally {
      setSigning(false);
    }
  };

  // Privy signs raw serialized bytes. Skip extension cluster switching because
  // embedded wallets have no user-selected network.
  const signWithPrivy = async (requestId: string, txB64: string) => {
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

      await resolveAndHide(requestId, Buffer.from(signedTransaction).toString("base64"));
      logLifecycle("TX_COMPLETE");
    } catch (e: any) {
      logLifecycle("FAILED", { message: e?.message || String(e) });
      await cancelRequest(requestId, e?.message || String(e)).catch(() => {});
      setError(e?.message || String(e));
    } finally {
      setSigning(false);
    }
  };

  // Automatically prompt the connected extension for a pending transaction;
  // keep the button for retry after rejection or late connection.
  const signWithExtension = async (requestId: string, txB64: string) => {
    setSigning(true);
    setError(null);
    try {
      const provider = getConnectedProvider(bridgeProvider);
      if (!provider) throw new Error("No Phantom/Solflare extension detected");
      // Reconnect this popup's provider before signing; stored preference and
      // extension presence do not establish a live page session.
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
      await resolveAndHide(requestId, Buffer.from(signed.serialize()).toString("base64"));
      logLifecycle("TX_COMPLETE");
    } catch (e: any) {
      logLifecycle("FAILED", { message: e?.message || String(e) });
      await cancelRequest(requestId, e?.message || String(e)).catch(() => {});
      setError(isNetworkMismatchError(e) ? NETWORK_MISMATCH_MESSAGE : e.message || String(e));
    } finally {
      setSigning(false);
    }
  };

  // SSE pushes pending state immediately and on changes; EventSource reconnects natively.
  useEffect(() => {
    const handleUpdate = (data: { tx?: string | null; label?: string | null; request_id?: string | null }) => {
      if (data.tx && data.tx !== pendingTx) {
        if (!data.request_id) {
          setError("Wallet bridge sent a signing request without a request ID.");
          return;
        }
        setPendingTx(data.tx);
        setPendingLabel(typeof data.label === "string" && data.label ? data.label : null);
        setCurrentRequestId(data.request_id);
        logLifecycle("SIGN_REQUEST_RECEIVED", {
          request_id: data.request_id,
          label: data.label ?? null,
        });
        const secret = sessionStorage.getItem("xfchess_session_key");
        // Persist attempted transaction identity across reloads so reconnecting SSE
        // cannot create a competing wallet prompt. Clear it on resolution or a new tx.
        const alreadyAttempted = sessionStorage.getItem("xfchess_auto_attempted_tx") === data.tx;
        if (secret) {
          handleAutoSign(data.request_id, data.tx, secret);
        } else if (
          bridgeStatusLoaded &&
          ((hasPrivyWallet && isPrivySession) ||
            (!isPrivySession && (bridgeProvider === "phantom" || bridgeProvider === "solflare") &&
              getConnectedProvider(bridgeProvider))) &&
          autoAttempted.current !== data.tx &&
          !alreadyAttempted
        ) {
          autoAttempted.current = data.tx;
          sessionStorage.setItem("xfchess_auto_attempted_tx", data.tx);
          // Prefer the embedded wallet for its authenticated session; an installed
          // extension or stored preference may belong to another wallet.
          if (hasPrivyWallet) signWithPrivy(data.request_id, data.tx);
          else signWithExtension(data.request_id, data.tx);
        }
      } else if (!data.tx) {
        sessionStorage.removeItem("xfchess_auto_attempted_tx");
        setPendingTx(null);
        setPendingLabel(null);
        setCurrentRequestId(null);
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
    source.onopen = () => logLifecycle("SSE_CONNECTED");
    source.onerror = (e) => console.warn(
      `[SIGNER] SSE connection error (will auto-retry) base=${API_BASE} readyState=${source.readyState}`,
      e,
    );
    return () => source.close();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pendingTx, hasPrivyWallet, isPrivySession, bridgeProvider, bridgeStatusLoaded]);

  // Retry pending SSE transactions when the embedded wallet finishes hydrating.
  useEffect(() => {
    if (
      !pendingTx ||
      !hasPrivyWallet ||
      !isPrivySession ||
      !bridgeStatusLoaded ||
      sessionStorage.getItem("xfchess_session_key") ||
      autoAttempted.current === pendingTx
    ) return;
    autoAttempted.current = pendingTx;
    sessionStorage.setItem("xfchess_auto_attempted_tx", pendingTx);
    void signWithPrivy(currentRequestId ?? "", pendingTx);
  }, [pendingTx, currentRequestId, hasPrivyWallet, isPrivySession, bridgeStatusLoaded]);

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
          <PrimaryBtn onClick={() => signWithPrivy(currentRequestId ?? "", pendingTx)}>Sign with Google</PrimaryBtn>
        ) : !bridgeStatusLoaded ? (
          <div style={{ color: TEXT_DIM, fontSize: 12, textAlign: "center" }}>
            Checking your wallet session...
          </div>
        ) : isPrivySession ? (
          <div style={{ color: TEXT_DIM, fontSize: 12, textAlign: "center" }}>
            {!privyReady || !privyAuthenticated
              ? "Reconnecting to Google..."
              : "Loading your Google wallet..."}
          </div>
        ) : (
          <PrimaryBtn onClick={() => signWithExtension(currentRequestId ?? "", pendingTx)}>Sign with Extension</PrimaryBtn>
        )
      )}
    </div>
  );
}

// Normal login chooses an off-chain handle. requireOnchain also submits
// init_profile when the game blocks a wager on a missing PlayerProfile.
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
  // Read Privy hooks here for embedded wallets and reopened profile steps;
  // the extension-provider prop is null for this flow.
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

        // Retry one stale-blockhash failure with a new hash and signature. Do not
        // retry program errors, insufficient funds, or user rejection.
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
              // Embedded wallets have no user-selected cluster; skip ensureDevnet.
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
          // Mirror the confirmed on-chain handle through sync-profile; off-chain
          // PATCH rejects usernames already set on-chain.
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


function Onboarding() {
  // A reopened sign popup with a stored session skips onboarding and displays
  // the pending transaction directly.
  const hasExistingSession = () =>
    !!(localStorage.getItem("xfchess_wallet_pubkey") || localStorage.getItem("xfchess_wallet"));

  // Determine returning-session status once at mount; a new login writes
  // storage keys that must not reclassify the current flow.
  const [wasReturningSession] = useState<boolean>(hasExistingSession);

  const [step, setStep] = useState<Step>(() => {
    const params = new URLSearchParams(window.location.search);
    const s = params.get("step");
    if (s === "connect_wallet") return "wallet";
    if (s === "profile") return "profile";
    // Devnet skips consent and opens wallet selection directly.
    if (s === "sign") return "sign";
    return "wallet";
  });
  // Require on-chain setup for profile deep links and bridge profile-step requests.
  // A reused popup may keep its old URL, so the bridge flag also updates this state.
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

  useEffect(() => {
    setReady(true);
    // Report mounted React readiness; a discovered OS window alone cannot prove it.
    logLifecycle("REACT_READY");
    apiPost("/api/ready", { sid: SESSION_ID }).catch(() => { /* bridge unreachable — nothing to report to */ });
    apiPost("/api/debug-log", {
      msg: `App mounted — initial step="${step}", url="${window.location.href}"`,
    }).catch(() => {});
    apiGet<{ url: string; explicit: boolean }>("/api/backend-url")
      .then((r) => logLifecycle("BACKEND_TARGET", r))
      .catch(() => { /* bridge unreachable */ });
  }, []);

  // Fall back to bridge wallet state when popup storage is empty so pending
  // transactions can still render their signer.
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

  // Poll for profile-step requests in every step except sign; a reused popup
  // can be idle on wallet or splash, but an active signature must not be interrupted.
  useEffect(() => {
    if (step === "sign") return;
    const interval = setInterval(async () => {
      try {
        const r = await apiGet<{ needs_profile: boolean }>("/api/needs-profile-step");
        if (r.needs_profile) {
          apiPost("/api/debug-log", {
            msg: `needs-profile-step poll: flag was set (was on step="${step}") — setStep(profile), requireOnchain=true`,
          }).catch(() => {});
          // Profile-step requests require on-chain initialization, not just an off-chain handle.
          setRequireOnchain(true);
          setStep("profile");
        }
      } catch { /* ignore — bridge may not be running */ }
    }, 1500);
    return () => clearInterval(interval);
  }, [step]);

  // Resolve the current wallet's handle before mounting ProfileStep; shared
  // localStorage may contain another wallet's name, and the form seeds its input once.
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

    // An off-chain username does not prove an on-chain PlayerProfile exists.
    // Check on-chain setup even for returning users so failed initialization can be retried.

    // A known off-chain handle can finish normal login, but needs_profile_step
    // requires on-chain setup even in a reused popup with a stale URL.
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
      // RPC failure does not prove a profile is missing. Keep the username verified
      // by this wallet's auth response unless it is a registration placeholder.
      const registrationPlaceholder = nextPubkey.slice(0, 8);
      if (user && user !== registrationPlaceholder) {
        resolvedUser = user;
        localStorage.setItem("xfchess_username", resolvedUser);
        setUsername(resolvedUser);
        needsProfile = false;
      }
    }

    // Mirror the resolved handle to the bridge; explicitly clear it when setup
    // is required so a prior wallet's cached name is not retained.
    apiPost("/wallet", { pubkey: nextPubkey, username: needsProfile ? "" : resolvedUser }).catch(
      () => {},
    );

    if (needsProfile) {
      apiPost("/api/debug-log", { msg: "handleAuth: needsProfile=true — setStep(profile)" }).catch(
        () => {},
      );
      // Clear both localStorage and React username state before an unchosen handle
      // can seed ProfileStep.
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
    // handleAuth owns step routing; onContinue must not overwrite its decision.
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

      {/* A signing deep link with an existing session skips login and waits for the pending transaction. */}
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

      {/*
       * Mount the signer only for step=sign so pending transactions cannot
       * interrupt a connect or onboarding popup.
       */}
      {step === "sign" && pubkey && <TransactionSigner pubkey={pubkey} />}
    </div>
  );
}

export default function App() {
  return (
    <>
      <style>{KEYFRAMES}</style>
      <Onboarding />
    </>
  );
}

