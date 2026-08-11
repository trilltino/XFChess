import { useState, useEffect, useRef, type CSSProperties } from "react";
import bs58 from "bs58";

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

async function apiGet<T = unknown>(path: string): Promise<T> {
  const resp = await fetch(`${API_BASE}${path}`);
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
    await fetch(`${API_BASE}/hide`, { method: "POST" });
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

async function apiPost<T = unknown>(path: string, body?: unknown): Promise<T> {
  const resp = await fetch(`${API_BASE}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
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
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!resp.ok) throw new Error(`sync-profile failed: ${resp.status}`);
  return resp.json();
}

async function fetchMe(token: string): Promise<{ username: string }> {
  const resp = await fetch(`${API_BASE}/api/auth/me`, {
    headers: { Authorization: `Bearer ${token}` },
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

function WalletStep({
  onContinue, onAuth, onClose
}: {
  onContinue: (pubkey: string, provider: any) => void;
  onAuth: (token: string, user: string, pubkey: string) => void;
  onClose?: () => void;
}) {
  const [error, setError] = useState<string | null>(null);
  const [connecting, setConnecting] = useState<"phantom" | "solflare" | null>(null);

  const WALLET_META = {
    phantom: { label: "Phantom", icon: "", installUrl: "https://phantom.app/", provider: () => (window as any).phantom?.solana },
    solflare: { label: "Solflare", icon: "", installUrl: "https://solflare.com/", provider: () => (window as any).solflare },
  };

  const handleConnect = async (walletName: "phantom" | "solflare") => {
    setError(null);
    setConnecting(walletName);
    try {
      let pubkey: string;
      let provider: any;
      // Always signs to prove key ownership before we treat the user as
      // logged in — no path (there used to be a "hot" local-keypair option
      // that self-signed silently, with no wallet popup at all) skips this.
      let signRaw: (msg: string) => Promise<string>;

      provider = WALLET_META[walletName].provider();
      if (!provider) {
        throw new Error(`${WALLET_META[walletName].label} extension not detected.`);
      }
      // Always a real approval prompt — no silent onlyIfTrusted reconnect.
      // That fast path resolved with no popup at all once an extension had
      // trusted this origin once, which is exactly what made "Connect
      // Wallet" silently reuse whichever wallet was approved first instead
      // of asking every time, especially confusing across multiple windows
      // sharing one real browser profile (each window is its own bridge
      // origin by port, but the extension's own trust memory isn't
      // necessarily scoped that finely).
      const resp: any = await provider.connect();
      // Phantom: publicKey is on the response object
      // Solflare: publicKey is on the provider after connect, not on resp
      pubkey = resp?.publicKey?.toBase58?.()
        ?? resp?.publicKey?.toString?.()
        ?? provider.publicKey?.toBase58?.()
        ?? provider.publicKey?.toString?.();
      // Signs raw bytes — no "utf8" arg to avoid Phantom>=0.16 off-chain prefix.
      signRaw = async (msg: string): Promise<string> => {
        const bytes = new TextEncoder().encode(msg);
        const { signature: sig } = await provider.signMessage(bytes);
        return bs58.encode(sig);
      };

      if (!pubkey) throw new Error("No public key returned from wallet");
      localStorage.setItem("xfchess_wallet", pubkey);
      localStorage.setItem("xfchess_wallet_provider", walletName);
      const _walletUsername = localStorage.getItem("xfchess_username") ?? "";

      // Check registration status first — avoids redundant signing requests.
      const checkResp = await fetch(`${API_BASE}/api/auth/check-wallet/${pubkey}`);
      const isRegistered = checkResp.ok;

      let auth: AuthResponse;
      if (isRegistered) {
        const ts = Math.floor(Date.now() / 1000);
        const sig = await signRaw(`xfchess:login:${ts}`);
        auth = await apiPost<AuthResponse>("/api/auth/login", {
          wallet: pubkey, signature: sig, timestamp: ts,
        });
      } else {
        const ts = Math.floor(Date.now() / 1000);
        const sig = await signRaw(`xfchess:register:${ts}`);
        auth = await apiPost<AuthResponse>("/api/auth/register", {
          wallet: pubkey, signature: sig, timestamp: ts,
          username: pubkey.slice(0, 8),
        });
      }

      // Only tell the bridge (and thus the game client's /status poll) that
      // a wallet is "connected" once ownership has been proven by a valid
      // signature — posting this earlier (e.g. right after provider.connect())
      // let a rejected sign-message prompt still leave the game client
      // believing the wallet was connected and unlocking wagered play.
      await apiPost("/wallet", { pubkey, username: _walletUsername });

      onAuth(auth.token, auth.username, pubkey);

      onContinue(pubkey, provider ?? null);
    } catch (e: any) {
      setError(e.message || String(e));
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
    const timer = setTimeout(() => { onComplete(); }, 2500);
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
  // Tracks which pending tx we've already auto-attempted, so the polling
  // effect doesn't re-fire signTransaction() every second while the user is
  // busy approving (or rejecting) it inside Phantom's own popup.
  const autoAttempted = useRef<string | null>(null);

  const resolveAndHide = async (signedB64: string) => {
    await fetch(`${API_BASE}/resolved`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ signed: signedB64 }),
    });
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
          await provider.connect({ onlyIfTrusted: true });
        } catch {
          await provider.connect();
        }
      }
      const txBytes = Buffer.from(txB64, "base64");
      const tx = deserializeTx(txBytes);
      await ensureDevnet(provider, localStorage.getItem("xfchess_wallet_provider"));
      const signed = await provider.signTransaction(tx);
      await resolveAndHide(Buffer.from(signed.serialize()).toString("base64"));
    } catch (e: any) {
      setError(e.message || String(e));
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
        if (secret) {
          handleAutoSign(data.tx, secret);
        } else if (getConnectedProvider() && autoAttempted.current !== data.tx) {
          autoAttempted.current = data.tx;
          signWithExtension(data.tx);
        }
      } else if (!data.tx) {
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
        <PrimaryBtn onClick={() => signWithExtension(pendingTx)}>Sign with Extension</PrimaryBtn>
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
      if (requireOnchain) {
        const walletPubkey =
          pubkey ?? localStorage.getItem("xfchess_wallet_pubkey") ?? localStorage.getItem("xfchess_wallet");
        if (!walletPubkey) {
          throw new Error("No wallet connected in this window — reopen from the game client and try again.");
        }
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const provider: any = walletProvider ?? getConnectedProvider();
        if (!provider) {
          throw new Error("No Phantom/Solflare extension detected in this window.");
        }
        if (!provider.publicKey) {
          try {
            await provider.connect({ onlyIfTrusted: true });
          } catch {
            await provider.connect();
          }
        }

        const dateOfBirth = Math.floor(new Date(`${dob}T00:00:00Z`).getTime() / 1000);
        const built = await apiPost<{ tx_b64: string; profile_pda: string }>("/api/auth/init-profile-tx", {
          username: handle,
          country: country.trim().toUpperCase(),
          date_of_birth: dateOfBirth,
        });

        const txBytes = Buffer.from(built.tx_b64, "base64");
        const tx = web3.Transaction.from(txBytes);
        let signed: web3.Transaction;
        try {
          await ensureDevnet(provider, localStorage.getItem("xfchess_wallet_provider"));
          signed = await provider.signTransaction(tx);
        } catch {
          throw new Error("Signature rejected — try again to finish on-chain setup.");
        }
        const signedB64 = Buffer.from(signed.serialize()).toString("base64");
        await apiPost<{ signature: string }>("/api/auth/broadcast-tx", { tx_b64: signedB64 });
      }

      const token = localStorage.getItem("xfchess_token");
      if (token) {
        const r = await fetch(`${API_BASE}/api/auth/username`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
          body: JSON.stringify({ username: handle }),
        });
        if (!r.ok) throw new Error(await r.text().catch(() => "Failed to save username"));
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
  const [requireOnchain] = useState<boolean>(
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

  // Poll for profile-step requests from the game client (e.g. "Wagered PVP" clicked)
  useEffect(() => {
    if (step !== "splash") return;
    const interval = setInterval(async () => {
      try {
        const r = await apiGet<{ needs_profile: boolean }>("/api/needs-profile-step");
        if (r.needs_profile) setStep("profile");
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
    // as a real display name directly. But for a returning player, `user`
    // is exactly what /api/auth/login already read out of the same DB row
    // sync-profile/auth-me would re-derive — trust it immediately rather
    // than re-verifying over the network first. A transient hiccup in the
    // on-chain RPC read (fetchProfileStatus) or the off-chain lookup
    // (fetchMe) must never override a value we already know is good, or a
    // returning player gets re-asked to pick a handle every time devnet RPC
    // has a bad moment. Only the registration placeholder itself falls
    // through to the slower on-chain/off-chain resolution below.
    const registrationPlaceholder = nextPubkey.slice(0, 8);
    if (user && user !== registrationPlaceholder) {
      localStorage.setItem("xfchess_username", user);
      setUsername(user);
      handleGameLaunch(nextPubkey, user);
      // Splash's "Welcome, X" is onboarding feedback for a first-ever login —
      // a returning session already knows who it is, so just close and drop
      // the player straight into the game instead of an extra screen+delay.
      if (wasReturningSession) {
        closePopup();
      } else {
        setStep("splash");
      }
      return;
    }

    // resolveExistingUsername checks both the on-chain PlayerProfile
    // (sync-profile) and the off-chain account username (auth/me, set by a
    // prior ProfileStep completion that hasn't been followed by a wager
    // yet), excluding that same placeholder — so a returning player with a
    // chosen handle but no on-chain profile isn't asked to pick a new one.
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
    } catch { /* on-chain lookup failed — fall through to profile step */ }

    if (needsProfile) {
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

