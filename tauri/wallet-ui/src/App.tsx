import { useState, useEffect, type CSSProperties } from "react";
import bs58 from "bs58";

// ---------------------------------------------------------------------------
// REST API bridge � works in Chrome AND Tauri webview
// ---------------------------------------------------------------------------
const BRIDGE_PORT = import.meta.env.VITE_BRIDGE_PORT ?? "7454";
const API_BASE = `http://localhost:${BRIDGE_PORT}`;

async function apiGet<T = unknown>(path: string): Promise<T> {
  const resp = await fetch(`${API_BASE}${path}`);
  if (!resp.ok) throw new Error(`GET ${path} failed: ${resp.status}`);
  return resp.json() as Promise<T>;
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

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------
type Step = "consent" | "entry" | "auth" | "wallet" | "profile" | "splash";

interface ConsentRecord {
  version: number;
  accepted_at: number;
}

interface AuthResponse {
  token: string;
  username: string;
  wallet?: string;
}

// ---------------------------------------------------------------------------
// Design tokens � matches web-solana color scheme
// ---------------------------------------------------------------------------
const PRIMARY    = "#ad5c2f";
const PRIMARY_DIM    = "rgba(173,92,47,0.15)";
const PRIMARY_BORDER = "rgba(173,92,47,0.4)";
const ACCENT     = "#f4bb44";
const BG         = "#081a14";
const SURFACE    = "#0a211a";
const CARD_BG    = "rgba(10,33,26,0.85)";
const BORDER     = "rgba(255,255,255,0.08)";
const TEXT       = "#ffffff";
const TEXT_DIM   = "#a0a0a0";
const TEXT_MUTED = "rgba(255,255,255,0.25)";
const INPUT_BG   = "rgba(255,255,255,0.04)";
// Keep old names as aliases so unchanged code still compiles
const RED        = PRIMARY;
const RED_DIM    = PRIMARY_DIM;
const RED_BORDER = PRIMARY_BORDER;

const CONSENT_VERSION = 1;


// ---------------------------------------------------------------------------
// Keyframes
// ---------------------------------------------------------------------------
const KEYFRAMES = `
  @import url('https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;500;600;700;800;900&family=Space+Grotesk:wght@400;500;600;700&display=swap');
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { font-family: 'Outfit', 'Inter', -apple-system, sans-serif; background: ${BG}; color: ${TEXT}; overflow: hidden; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes fadeUp { from { opacity: 0; transform: translateY(16px); } to { opacity: 1; transform: translateY(0); } }
  @keyframes wave { 0%,100% { transform: translateY(0); } 50% { transform: translateY(-6px); } }
  @keyframes glow { 0%,100% { text-shadow: 0 0 20px rgba(173,92,47,0.5); } 50% { text-shadow: 0 0 40px rgba(173,92,47,0.9), 0 0 80px rgba(244,187,68,0.3); } }
  @keyframes progress { from { width: 0%; } to { width: 100%; } }
  @keyframes pulse { 0%,100% { opacity:1; transform: scale(1); } 50% { opacity:0.6; transform: scale(0.97); } }
  @keyframes shimmer { 0% { background-position: -200% center; } 100% { background-position: 200% center; } }
  input { outline: none; font-family: 'Outfit', 'Inter', sans-serif; }
  input::placeholder { color: ${TEXT_MUTED}; }
  button { cursor: pointer; font-family: 'Outfit', 'Inter', sans-serif; }
  a { color: ${PRIMARY}; text-decoration: none; }
  ::-webkit-scrollbar { width: 4px; }
  ::-webkit-scrollbar-track { background: transparent; }
  ::-webkit-scrollbar-thumb { background: rgba(173,92,47,0.3); border-radius: 2px; }
`;

// ---------------------------------------------------------------------------
// Environment detection
// ---------------------------------------------------------------------------
const isTauri = !!(window as any).__TAURI__;

// ---------------------------------------------------------------------------
// Layout helpers
// ---------------------------------------------------------------------------
const page: CSSProperties = {
  width: "100vw", height: "100vh", display: "flex", flexDirection: "column",
  alignItems: "center", justifyContent: "center", background: BG,
  position: "relative", overflow: "hidden",
};

// ---------------------------------------------------------------------------
// Navbar � matches web-solana pill style; links back to /
// ---------------------------------------------------------------------------
function SiteNav() {
  const HOME = window.location.origin + "/";
  return (
    <nav style={{
      position: "fixed", top: 12, left: "50%", transform: "translateX(-50%)",
      width: "92%", maxWidth: 520, height: 42, padding: "0 20px",
      display: "flex", alignItems: "center", justifyContent: "space-between",
      zIndex: 100,
      background: "rgba(8,26,20,0.75)",
      border: `1px solid ${BORDER}`,
      borderRadius: 100,
      backdropFilter: "blur(24px)", WebkitBackdropFilter: "blur(24px)",
      boxShadow: `0 10px 40px rgba(0,0,0,0.6), 0 0 50px rgba(173,92,47,0.1)`,
    }}>
      <a href={HOME} style={{
        display: "flex", alignItems: "center", gap: 8,
        textDecoration: "none", userSelect: "none",
      }}>
        <span style={{ fontSize: 22, lineHeight: 1 }}></span>
        <span style={{ fontSize: 15, fontWeight: 800, letterSpacing: "-0.04em" }}>
          <span style={{ color: PRIMARY }}>XF</span>
          <span style={{ color: TEXT }}>Chess</span>
        </span>
      </a>
      <a href={HOME} style={{
        fontSize: 12, fontWeight: 600, color: TEXT_DIM,
        textDecoration: "none", letterSpacing: "0.04em",
        padding: "6px 14px", borderRadius: 20,
        border: `1px solid ${BORDER}`,
        transition: "all 0.2s",
      }}
        onMouseEnter={e => { (e.currentTarget as HTMLAnchorElement).style.borderColor = PRIMARY; (e.currentTarget as HTMLAnchorElement).style.color = TEXT; }}
        onMouseLeave={e => { (e.currentTarget as HTMLAnchorElement).style.borderColor = BORDER; (e.currentTarget as HTMLAnchorElement).style.color = TEXT_DIM; }}
      >Home</a>
    </nav>
  );
}

function GridBg() {
  return (
    <>
      {/* Deep green radial glow � matches web-solana bg */}
      <div style={{
        position: "fixed", inset: 0, zIndex: 0, pointerEvents: "none",
        background: `radial-gradient(ellipse 80% 60% at 50% 0%, rgba(173,92,47,0.12) 0%, transparent 70%),
                     radial-gradient(ellipse 60% 40% at 80% 80%, rgba(244,187,68,0.06) 0%, transparent 60%)`,
      }} />
      {/* Subtle chess-board grid */}
      <div style={{
        position: "fixed", inset: 0, zIndex: 0, pointerEvents: "none",
        backgroundImage: `linear-gradient(rgba(173,92,47,0.06) 1px, transparent 1px), linear-gradient(90deg, rgba(173,92,47,0.06) 1px, transparent 1px)`,
        backgroundSize: "56px 56px",
      }} />
    </>
  );
}

function LogoMark({ size = 40 }: { size?: number }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8, userSelect: "none" }}>
      <span style={{ fontSize: size * 0.7, lineHeight: 1 }}></span>
      <span style={{ fontSize: size * 0.55, fontFamily: "'Space Grotesk', sans-serif", fontWeight: 800, letterSpacing: "-0.04em" }}>
        <span style={{ color: RED }}>XF</span>
        <span style={{ color: TEXT }}>Chess</span>
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
    try {
      if ((window as any).__TAURI__) {
         await fetch(`${API_BASE}/hide`, { method: "POST" });
      } else {
         window.close();
      }
    } catch {
      window.close();
    }
  };

  return (
    <div style={{
      width: "92%", maxWidth: 400, padding: "28px 32px", background: CARD_BG,
      border: `1px solid ${BORDER}`, borderRadius: 20,
      backdropFilter: "blur(24px)", WebkitBackdropFilter: "blur(24px)",
      boxShadow: `0 10px 40px rgba(0,0,0,0.6), 0 0 50px rgba(173,92,47,0.08)`,
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
          onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = "rgba(173,92,47,0.8)"; }}
          onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = "rgba(255,255,255,0.1)"; }}
        >�</button>
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
      background: disabled || loading ? "rgba(173,92,47,0.3)" : `linear-gradient(135deg, ${PRIMARY}, #8c4a26)`,
      color: "#fff", fontSize: 15, fontWeight: 700, letterSpacing: "0.02em",
      transition: "all 0.2s", boxShadow: disabled || loading ? "none" : `0 4px 20px rgba(173,92,47,0.35)`,
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
      padding: "10px 14px", borderRadius: 10, background: "rgba(173,92,47,0.1)",
      border: `1px solid ${PRIMARY_BORDER}`, color: ACCENT, fontSize: 13, marginBottom: 16,
    }}>
       {msg}
    </div>
  );
}

function StepDots({ step }: { step: Step }) {
  const steps: Step[] = ["consent", "entry", "auth", "wallet", "profile", "splash"];
  const idx = steps.indexOf(step);
  return (
    <div style={{ display: "flex", gap: 6, justifyContent: "center", marginBottom: 28 }}>
      {steps.slice(0, 5).map((_, i) => (
        <div key={i} style={{
          width: i === idx ? 20 : 6, height: 6, borderRadius: 3,
          background: i <= idx ? RED : "rgba(255,255,255,0.12)", transition: "all 0.3s",
        }} />
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 0.5 � Entry Path Selection
// ---------------------------------------------------------------------------
function EntryStep({
  onChoice,
  onClose
}: {
  onChoice: (choice: "wallet" | "email") => void;
  onClose?: () => void;
}) {
  const [launching, setLaunching] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);

  const playOffline = async () => {
    setLaunchError(null);
    setLaunching(true);
    try {
      const kp = web3.Keypair.generate();
      const pubkey = kp.publicKey.toBase58();
      sessionStorage.setItem("xfchess_session_key", JSON.stringify(Array.from(kp.secretKey)));
      await apiPost("/api/game/launch", { pubkey, hot: true, username: "LocalPlayer" });
    } catch (e: any) {
      setLaunchError(e.message || String(e));
      setLaunching(false);
    }
  };

  return (
    <Card showClose={true} onClose={onClose}>
      <StepDots step="entry" />
      <div style={{ textAlign: "center" as const, marginBottom: 28 }}>
        <LogoMark size={44} />
        <h2 style={{ fontSize: 24, fontWeight: 900, marginTop: 16, fontFamily: "'Space Grotesk', sans-serif", color: TEXT }}>
           Choose your identity path
        </h2>
        <p style={{ fontSize: 13, color: TEXT_DIM, marginTop: 6 }}>
          Choose your identity path for XFChess
        </p>
      </div>

      {launchError && <ErrorMsg msg={launchError} />}

      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        <button
          style={{ ...pathBtn, borderColor: ACCENT, background: "rgba(244,187,68,0.08)" }}
          onClick={playOffline}
          disabled={launching}
          onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = "rgba(244,187,68,0.15)"; }}
          onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = "rgba(244,187,68,0.08)"; }}
        >
          <div style={{ ...iconCircle, background: "rgba(244,187,68,0.12)" }}></div>
          <div style={{ flex: 1 }}>
            <div style={{ fontWeight: 800, fontSize: 15, color: ACCENT }}>Play Now (Offline)</div>
            <div style={{ fontSize: 12, color: TEXT_MUTED }}>Local play � no wallet or account needed</div>
          </div>
          {launching && <div style={{ width: 16, height: 16, border: `2px solid rgba(244,187,68,0.3)`, borderTop: `2px solid ${ACCENT}`, borderRadius: "50%", animation: "spin 0.7s linear infinite" }} />}
        </button>

        <div style={{ margin: "4px 0", height: 1, background: "rgba(255,255,255,0.05)" }} />

        <button
          style={pathBtn}
          onClick={() => onChoice("wallet")}
          onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.borderColor = PRIMARY; (e.currentTarget as HTMLButtonElement).style.background = PRIMARY_DIM; }}
          onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.borderColor = BORDER; (e.currentTarget as HTMLButtonElement).style.background = "rgba(255,255,255,0.03)"; }}
        >
          <div style={{ ...iconCircle, background: "rgba(173,92,47,0.1)" }}>🔐</div>
          <div style={{ flex: 1 }}>
            <div style={{ fontWeight: 800, fontSize: 15 }}>Login with Wallet</div>
            <div style={{ fontSize: 12, color: TEXT_MUTED }}>Phantom / Solflare — for existing users</div>
          </div>
        </button>

        <button
          style={pathBtn}
          onClick={() => onChoice("email")}
          onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.borderColor = PRIMARY; (e.currentTarget as HTMLButtonElement).style.background = PRIMARY_DIM; }}
          onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.borderColor = BORDER; (e.currentTarget as HTMLButtonElement).style.background = "rgba(255,255,255,0.03)"; }}
        >
          <div style={{ ...iconCircle, background: "rgba(244,187,68,0.1)" }}>✉</div>
          <div style={{ flex: 1 }}>
            <div style={{ fontWeight: 800, fontSize: 15 }}>Email + Password</div>
            <div style={{ fontSize: 12, color: TEXT_MUTED }}>Classic account — bring your own wallet</div>
          </div>
        </button>

      </div>
    </Card>
  );
}

const pathBtn: CSSProperties = {
  width: "100%", padding: "16px 20px", borderRadius: 16, border: `1px solid ${BORDER}`,
  background: "rgba(255,255,255,0.03)", color: TEXT, textAlign: "left" as const,
  display: "flex", alignItems: "center", gap: 16, cursor: "pointer", transition: "all 0.2s",
};

const iconCircle: CSSProperties = {
  width: 44, height: 44, borderRadius: "50%", display: "flex", alignItems: "center",
  justifyContent: "center", fontSize: 20,
};

// ---------------------------------------------------------------------------
// Step 0 � Legal / GDPR Consent
// ---------------------------------------------------------------------------
function ConsentStep({ onAccept, onClose }: { onAccept: () => void; onClose?: () => void }) {
  const [checkedTos, setTos] = useState(false);
  const [checkedGdpr, setGdpr] = useState(false);
  const [checkedAge, setAge] = useState(false);
  const canContinue = checkedTos && checkedGdpr && checkedAge;

  return (
    <Card showClose={true} onClose={onClose} style={{ maxWidth: 360, padding: "20px 24px" }}>
      <div style={{ textAlign: "center" as const, marginBottom: 24 }}>
        <LogoMark size={44} />
        <p style={{ fontSize: 12, color: TEXT_MUTED, marginTop: 6, letterSpacing: "0.12em", textTransform: "uppercase" as const }}>
          Legal &amp; Privacy
        </p>
      </div>

      <div style={{
        height: 280, overflowY: "auto" as const, marginBottom: 20, paddingRight: 8,
        fontSize: 13, lineHeight: 1.7, color: TEXT_DIM,
      }}>
        <p style={{ fontWeight: 700, color: TEXT, fontSize: 14, marginBottom: 6 }}> Terms of Service</p>
        <p style={{ marginBottom: 12 }}>
          XFChess is a decentralised chess platform operating on the Solana blockchain. By using
          this application you acknowledge that wagered games involve real cryptocurrency. All wagers
          are final and governed solely by on-chain smart contract logic. XForceSolutions Ltd accepts no
          liability for smart contract bugs, network outages, or losses arising from gameplay. You
          must be 18+ to participate in wagered games.
        </p>

        <p style={{ fontWeight: 700, color: TEXT, fontSize: 14, marginBottom: 6 }}> Privacy &amp; GDPR Notice</p>
        <p style={{ marginBottom: 8 }}>We collect and store the following data securely:</p>
        <ul style={{ paddingLeft: 18, marginBottom: 12 }}>
          <li>Account credentials (email + bcrypt-hashed password � plaintext never stored)</li>
          <li>Solana wallet public key (public by nature on-chain)</li>
          <li>Game history &amp; move records (used for anti-cheat and tournament verification)</li>
          <li>Session tokens (short-lived JWTs, stored only in memory)</li>
        </ul>
        <p style={{ marginBottom: 12 }}>
          We do <strong>not</strong> sell your data to third parties. Identity/tax data (collected
          only if you opt into wagering under CARF 2026 compliance) is stored in a zero-knowledge
          encrypted vault and used exclusively for regulatory reporting. You may request deletion at
          any time by emailing <a href="mailto:privacy@xfchess.com">privacy@xfchess.com</a>.
        </p>
        <p style={{ marginBottom: 12 }}>
          Your rights under GDPR include: access, rectification, erasure, restriction of processing,
          data portability, and objection. The data controller is XForceSolutions Ltd. For complaints,
          contact the ICO (UK) or your local supervisory authority.
        </p>

        <p style={{ fontWeight: 700, color: TEXT, fontSize: 14, marginBottom: 6 }}> CARF 2026 Compliance</p>
        <p style={{ marginBottom: 8 }}>
          XFChess is a Reporting Crypto-Asset Service Provider (RCASP) under CARF 2026. If you
          wager cryptocurrency assets, we are legally required to collect, verify, and in some
          jurisdictions report identity information (name, address, tax ID) to local tax authorities.
          This applies only to wagered play. Free &amp; casual games are unaffected.
        </p>
      </div>

      <div style={{ marginBottom: 20 }}>
        {[
          { checked: checkedTos, set: setTos, label: "I have read and accept the Terms of Service" },
          { checked: checkedGdpr, set: setGdpr, label: "I consent to data collection as described in the Privacy Notice" },
          { checked: checkedAge, set: setAge, label: "I confirm I am 18 years of age or older" },
        ].map(({ checked, set, label }) => (
          <label key={label} style={{
            display: "flex", gap: 10, alignItems: "flex-start", marginBottom: 12,
            cursor: "pointer", fontSize: 13, color: TEXT_DIM,
          }}>
            <div onClick={() => set(!checked)} style={{
              width: 18, height: 18, minWidth: 18, borderRadius: 5,
              border: `2px solid ${checked ? RED : BORDER}`, background: checked ? RED_DIM : "transparent",
              display: "flex", alignItems: "center", justifyContent: "center", marginTop: 1, transition: "all 0.15s",
            }}>
              {checked && <span style={{ color: RED, fontSize: 11, fontWeight: 800 }}></span>}
            </div>
            <span onClick={() => set(!checked)}>{label}</span>
          </label>
        ))}
      </div>

      <PrimaryBtn onClick={onAccept} disabled={!canContinue}>Continue ?</PrimaryBtn>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// Step 1 � Login / Register
// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Step 1 � Login / Register (Email Path)
// ---------------------------------------------------------------------------
function AuthStep({ onAuth, onBack, onClose }: { onAuth: (token: string, username: string) => void; onBack: () => void; onClose?: () => void }) {
  const [mode, setMode] = useState<"login" | "register">("login");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const submit = async () => {
    setError(null);
    if (!email || !password) { setError("Email and password are required"); return; }
    setLoading(true);
    try {
      let res: AuthResponse;
      if (mode === "login") {
        res = await apiPost<AuthResponse>("/api/auth/login-email", { email, password });
      } else {
        // Registration now uses a default username (email prefix)
        // Sol Name is finalized in the profile step later
        res = await apiPost<AuthResponse>("/api/auth/register-email", { 
          email, password, username: email.split('@')[0] 
        });
      }
      onAuth(res.token, res.username);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg.includes("404") || msg.includes("Invalid") ? "Invalid email or password" : msg);
    } finally {
      setLoading(false);
    }
  };

  const handleKey = (e: React.KeyboardEvent) => { if (e.key === "Enter") submit(); };

  return (
    <Card showClose={true} onClose={onClose}>
      <StepDots step="auth" />
      <div style={{ textAlign: "center" as const, marginBottom: 28 }}>
        <h2 style={{ fontSize: 22, fontWeight: 800, fontFamily: "'Space Grotesk', sans-serif", color: TEXT }}>
          {mode === "login" ? "Account Login" : "Email Registration"}
        </h2>
        <p style={{ fontSize: 13, color: TEXT_DIM, marginTop: 4 }}>
          {mode === "login"
            ? "Sign in to your XFChess account"
            : "Quick account setup � no handle needed yet"}
        </p>
      </div>

      {error && <ErrorMsg msg={error} />}

      <div onKeyDown={handleKey}>
        <InputField label="Email Address" value={email} onChange={setEmail} type="email" placeholder="you@example.com" />
        <InputField label="Password" value={password} onChange={setPassword} type="password" placeholder="��������" />
      </div>

      <div style={{ marginTop: 20, marginBottom: 20 }}>
        <PrimaryBtn onClick={submit} loading={loading}>
          {mode === "login" ? "Sign In" : "Register Account"}
        </PrimaryBtn>
      </div>

      <div style={{ textAlign: "center" as const, display: "flex", flexDirection: "column", gap: 12 }}>
        <button onClick={() => { setMode(mode === "login" ? "register" : "login"); setError(null); }}
          style={{ background: "none", border: "none", color: TEXT_DIM, fontSize: 13 }}>
          {mode === "login"
            ? <>No account? <span style={{ color: RED, fontWeight: 600 }}>Register</span></>
            : <>Already have one? <span style={{ color: RED, fontWeight: 600 }}>Sign in</span></>}
        </button>
        <button onClick={onBack} style={{ background: "none", border: "none", color: TEXT_MUTED, fontSize: 12, textDecoration: "underline" }}>
          Go back to paths
        </button>
      </div>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// Step 2 � Wallet Connection (direct, no adapter library)
// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Step 2 � Wallet Connection (Tauri Embedded)
// ---------------------------------------------------------------------------
import * as web3 from "@solana/web3.js";

function WalletStep({
  mode, onContinue, onAuth, onBack, onClose
}: {
  mode: "login" | "link";
  onContinue: (pubkey: string) => void;
  onAuth: (token: string, user: string, pubkey?: string) => void;
  onBack: () => void;
  onClose?: () => void;
}) {
  const [error, setError] = useState<string | null>(null);
  const [connecting, setConnecting] = useState<"phantom" | "solflare" | null>(null);

  const WALLET_META = {
    phantom: { label: "Phantom", icon: "", installUrl: "https://phantom.app/", provider: () => (window as any).phantom?.solana },
    solflare: { label: "Solflare", icon: "?", installUrl: "https://solflare.com/", provider: () => (window as any).solflare },
  };

  const handleConnect = async (walletName: "phantom" | "solflare" | "hot") => {
    setError(null);
    setConnecting(walletName === "hot" ? null : walletName);
    try {
      let pubkey: string;
      let provider: any;

      if (walletName === "hot") {
        // Generate or load a local session key
        const kp = web3.Keypair.generate();
        pubkey = kp.publicKey.toBase58();
        const secretArr = Array.from(kp.secretKey);
        sessionStorage.setItem("xfchess_session_key", JSON.stringify(secretArr));
        localStorage.setItem("xfchess_wallet", pubkey);
      } else {
        provider = WALLET_META[walletName].provider();
        if (!provider) {
          throw new Error(`${WALLET_META[walletName].label} extension not detected.`);
        }
        const resp = await provider.connect();
        // Phantom: publicKey is on the response object
        // Solflare: publicKey is on the provider after connect, not on resp
        pubkey = resp?.publicKey?.toBase58?.()
          ?? resp?.publicKey?.toString?.()
          ?? provider.publicKey?.toBase58?.()
          ?? provider.publicKey?.toString?.();
      }

      if (!pubkey) throw new Error("No public key returned from wallet");
      localStorage.setItem("xfchess_wallet", pubkey);
      const _walletUsername = localStorage.getItem("xfchess_username") ?? "";
      await apiPost("/wallet", { pubkey, username: _walletUsername });

      if (walletName === "hot") {
        // Hot wallet is device-only � no backend auth needed for local play
        onAuth("offline", "LocalPlayer", pubkey);
      } else {
        // Signs raw bytes � no "utf8" arg to avoid Phantom>=0.16 off-chain prefix.
        const signRaw = async (msg: string): Promise<string> => {
          const bytes = new TextEncoder().encode(msg);
          const { signature: sig } = await provider.signMessage(bytes);
          return bs58.encode(sig);
        };

        // Check registration status first � avoids redundant signing requests.
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
        onAuth(auth.token, auth.username, pubkey);
      }

      onContinue(pubkey);
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
        <h2 style={{ fontSize: 22, fontWeight: 800, fontFamily: "'Space Grotesk', sans-serif", color: TEXT }}>
          {mode === "login" ? "Wallet Sign-In" : "Link Your Wallet"}
        </h2>
        <p style={{ fontSize: 13, color: TEXT_DIM, marginTop: 4 }}>
          {mode === "login" ? "Verify ownership to access your account" : "Connect to enable on-chain gameplay"}
        </p>
      </div>

      {error && <ErrorMsg msg={error} />}

      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        {/* Hot Wallet Option � Primary for Tauri */}
        {isTauri && (
          <button
            style={{ ...walletBtnStyle, borderColor: PRIMARY_BORDER, background: PRIMARY_DIM }}
            onClick={() => handleConnect("hot")}
          >
            <span style={{ fontSize: 20 }}></span>
            <span style={{ flex: 1 }}>Software Wallet (Hot Wallet)</span>
            <span style={{ fontSize: 11, color: PRIMARY, fontWeight: 800 }}>RECOMMENDED</span>
          </button>
        )}

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
                <span style={{ flex: 1, color: TEXT_DIM }}>{meta.label} � not installed</span>
                <span style={{ fontSize: 11, color: PRIMARY, fontWeight: 700 }}>Install ?</span>
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

      <button onClick={onBack} style={{ width: "100%", marginTop: 20, background: "none", border: "none", color: TEXT_MUTED, fontSize: 12, textDecoration: "underline" }}>
         Back
      </button>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// Step 3 � Entering Splash
// ---------------------------------------------------------------------------
function SplashStep({ username, onComplete }: { username: string; onComplete: () => void }) {
  return (
    <div style={{ textAlign: "center" as const, position: "relative" as const, zIndex: 1, animation: "fadeUp 0.5s ease" }}>
      <div style={{
        fontSize: 72, marginBottom: 16,
        animation: "glow 2s ease-in-out infinite, wave 3s ease-in-out infinite", display: "inline-block",
      }}></div>

      <div style={{ marginBottom: 8 }}>
        <div style={{
          fontSize: 32, fontWeight: 900, fontFamily: "'Space Grotesk', sans-serif",
          background: `linear-gradient(135deg, ${PRIMARY}, ${ACCENT}, ${PRIMARY})`, backgroundSize: "200% auto",
          WebkitBackgroundClip: "text", WebkitTextFillColor: "transparent",
          animation: "shimmer 2s linear infinite",
        }}>XFChess</div>
      </div>

      <p style={{ fontSize: 14, color: TEXT_DIM, marginBottom: 24 }}>
        Welcome, <span style={{ color: TEXT, fontWeight: 600 }}>{username}</span>
      </p>

      <button
        onClick={onComplete}
        style={{
          padding: "14px 32px", borderRadius: 10, border: "none",
          background: `linear-gradient(135deg, ${PRIMARY}, #8c4a26)`,
          color: "#fff", fontSize: 15, fontWeight: 700, letterSpacing: "0.02em",
          cursor: "pointer", boxShadow: `0 4px 20px rgba(173,92,47,0.35)`,
          transition: "all 0.2s",
        }}
      >
        View Profile Hub ?
      </button>
    </div>
  );
}


// ---------------------------------------------------------------------------
// Background Transaction Signer
// ---------------------------------------------------------------------------
function TransactionSigner({ pubkey }: { pubkey: string }) {
  const [pendingTx, setPendingTx] = useState<string | null>(null);
  const [signing, setSigning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const resolveAndHide = async (signedB64: string) => {
    await fetch("http://localhost:7454/resolved", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ signed: signedB64 }),
    });
    setPendingTx(null);
    if (isTauri) {
      await fetch("http://localhost:7454/hide", { method: "POST" });
    } else {
      window.close();
    }
  };

  const signTxBytes = async (txB64: string, kp: web3.Keypair): Promise<string> => {
    const txBytes = Buffer.from(txB64, "base64");
    try {
      const tx = web3.VersionedTransaction.deserialize(txBytes);
      tx.sign([kp]);
      return Buffer.from(tx.serialize()).toString("base64");
    } catch {
      const tx = web3.Transaction.from(txBytes);
      tx.partialSign(kp);
      return tx.serialize().toString("base64");
    }
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

  useEffect(() => {
    const poll = async () => {
      try {
        const resp = await fetch("http://localhost:7454/pending");
        const data = await resp.json();
        if (data.tx && data.tx !== pendingTx) {
          setPendingTx(data.tx);
          const secret = sessionStorage.getItem("xfchess_session_key");
          if (secret) {
            handleAutoSign(data.tx, secret);
          }
        } else if (!data.tx) {
          setPendingTx(null);
        }
      } catch (e) {
        console.warn("[SIGNER] Poll failed", e);
      }
    };

    const interval = setInterval(poll, 1000);
    return () => clearInterval(interval);
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
        <span style={{ fontWeight: 800, fontSize: 13, color: TEXT }}>PENDING TRANSACTION</span>
      </div>
      <p style={{ fontSize: 12, color: TEXT_DIM, marginBottom: 16 }}>
        {signing ? "Signing…" : "Awaiting signature."}
      </p>
      {error && <ErrorMsg msg={error} />}
      {/* Fallback: extension signing when no session key stored */}
      {!signing && !sessionStorage.getItem("xfchess_session_key") && (
        <PrimaryBtn onClick={async () => {
          const provider = (window as any).phantom?.solana || (window as any).solflare;
          if (!provider) return;
          const txBytes = Buffer.from(pendingTx, "base64");
          const tx = web3.VersionedTransaction.deserialize(txBytes);
          const signed = await provider.signTransaction(tx);
          await resolveAndHide(Buffer.from(signed.serialize()).toString("base64"));
        }}>Sign with Extension</PrimaryBtn>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Root orchestrator
// ---------------------------------------------------------------------------
function ProfileStep({
  onComplete,
  pubkey,
  isHotWallet = false,
  onClose,
}: {
  onComplete: (handle: string) => void;
  pubkey?: string | null;
  isHotWallet?: boolean;
  onClose?: () => void;
}) {
  const [handle, setHandle] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [synced, setSynced] = useState<string | null>(null);

  // On mount: try sync-profile (pulls on-chain canonical username into DB).
  // If the user already has an on-chain profile we skip the form entirely.
  useEffect(() => {
    const trySync = async () => {
      const token = localStorage.getItem("xfchess_token");
      if (!token || isHotWallet) { setLoading(false); return; }
      try {
        const r = await fetch(`${API_BASE}/api/auth/sync-profile`, {
          method: "POST",
          headers: { Authorization: `Bearer ${token}` },
        });
        if (r.ok) {
          const { username } = await r.json();
          localStorage.setItem("xfchess_username", username);
          setSynced(username);
          setLoading(false);
          return;
        }
      } catch { /* no on-chain profile yet � show form */ }
      setLoading(false);
    };
    trySync();
  }, [isHotWallet]);

  const submit = async () => {
    if (!handle) return;
    setSaving(true);
    setError(null);
    try {
      const token = localStorage.getItem("xfchess_token");
      if (token) {
        const r = await fetch(`${API_BASE}/api/auth/username`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
          body: JSON.stringify({ username: handle }),
        });
        if (!r.ok) {
          const msg = await r.text().catch(() => "Failed to save username");
          throw new Error(msg);
        }
      }
      localStorage.setItem("xfchess_username", handle);
      onComplete(handle);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <Card showClose={true} onClose={onClose}>
        <div style={{ textAlign: "center", padding: "40px 0" }}>
          <div style={{ width: 24, height: 24, border: `2px solid ${RED_BORDER}`, borderTop: `2px solid ${RED}`, borderRadius: "50%", animation: "spin 0.8s linear infinite", margin: "0 auto 12px" }} />
          <p style={{ color: TEXT_DIM, fontSize: 13 }}>Loading profile�</p>
        </div>
      </Card>
    );
  }

  // On-chain username found � confirm and proceed
  if (synced) {
    return (
      <Card showClose={true} onClose={onClose}>
        <StepDots step="profile" />
        <div style={{ textAlign: "center" as const, marginBottom: 24 }}>
          <div style={{ fontSize: 40, marginBottom: 8 }}></div>
          <h2 style={{ fontSize: 22, fontWeight: 800, color: TEXT }}>Profile Found</h2>
          <p style={{ fontSize: 14, color: TEXT_DIM, marginTop: 8 }}>
            On-chain username: <strong style={{ color: PRIMARY }}>{synced}</strong>
          </p>
        </div>
        <PrimaryBtn onClick={() => onComplete(synced)}>
          Enter Arena ?
        </PrimaryBtn>
      </Card>
    );
  }

  return (
    <Card showClose={true} onClose={onClose}>
      <StepDots step="profile" />
      <div style={{ textAlign: "center" as const, marginBottom: 28 }}>
        <h2 style={{ fontSize: 22, fontWeight: 800, fontFamily: "'Space Grotesk', sans-serif", color: TEXT }}>
          Choose Your Handle
        </h2>
        <p style={{ fontSize: 13, color: TEXT_DIM, marginTop: 4 }}>
          Pick a display name for the arena (3�20 chars)
        </p>
      </div>
      {error && <ErrorMsg msg={error} />}
      <InputField label="Chess Handle" value={handle} onChange={setHandle} placeholder="e.g. DragonKnight99" />
      <p style={{ fontSize: 11, color: TEXT_MUTED, textAlign: "center", marginBottom: 16 }}>
        Create a full on-chain profile at{" "}
        <a href="http://localhost:5173/profile" target="_blank" rel="noreferrer" style={{ color: PRIMARY }}>
          xfchess.io/profile
        </a>
        {" "}to lock your username globally.
      </p>
      <PrimaryBtn onClick={submit} loading={saving} disabled={!handle || handle.length < 3}>
        Finalise &amp; Enter Arena
      </PrimaryBtn>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// Root orchestrator
// ---------------------------------------------------------------------------
function Onboarding() {
  const [step, setStep] = useState<Step>(() => {
    const params = new URLSearchParams(window.location.search);
    const s = params.get("step");
    if (s === "connect_wallet") return "wallet";
    return "consent";
  });
  const [username, setUsername] = useState("Player");
  const [ready, setReady] = useState(false);
  const [pubkey, setPubkey] = useState<string | null>(null);
  const [path, setPath] = useState<"wallet" | "email" | "hot" | null>(null);

  // Force exact window size � Chrome ignores --window-size when already running
  useEffect(() => {
    window.resizeTo(420, 500);
  }, []);

  useEffect(() => {
    // Always start disconnected — wallet must be connected each session.
    apiGet<ConsentRecord | null>("/api/consent").then(record => {
      if (record && record.version >= CONSENT_VERSION) {
        setStep("entry");
      }
    }).catch(() => {}).finally(() => setReady(true));
  }, []);

  const handleConsent = async () => {
    try { await apiPost("/api/consent", { version: CONSENT_VERSION }); } catch { /* non-critical */ }
    setStep("entry");
  };

  const onChoice = (choice: "wallet" | "email") => {
    setPath(choice);
    if (choice === "wallet") {
      setStep("wallet");
    } else {
      setStep("auth");
    }
  };

  const handleAuth = (token: string, user: string, nextPubkey?: string) => {
    localStorage.setItem("xfchess_token", token);
    localStorage.setItem("xfchess_username", user);
    setUsername(user);
    if (nextPubkey) {
      localStorage.setItem("xfchess_wallet_pubkey", nextPubkey);
      setPubkey(nextPubkey);
    }
    if (path === "wallet" && nextPubkey) {
      setStep("profile");
      return;
    }
    // After email auth, we MUST connect a wallet to link them
    setStep("wallet");
  };

  const handleWalletContinue = (pk: string) => {
    localStorage.setItem("xfchess_wallet", pk);
    setPubkey(pk);
    setStep("profile");
  };

  const handleProfileComplete = (handle: string) => {
    setUsername(handle);
    setStep("splash");
    handleGameLaunch(pubkey || "dummy", path === "hot", handle);
  };

  const handleGameLaunch = async (pk: string, hot: boolean, user: string) => {
    const token = localStorage.getItem("xfchess_token");
    try { 
      await apiPost("/api/game/launch", { pubkey: pk, hot, username: user, token }); 
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
      {step === "consent" && <ConsentStep onAccept={handleConsent} onClose={() => window.close()} />}
      {step === "entry"   && <EntryStep onChoice={onChoice} onClose={() => window.close()} />}

      {step === "auth"    && <AuthStep
        onAuth={handleAuth}
        onBack={() => setStep("entry")}
        onClose={() => window.close()}
      />}

      {step === "wallet"  && <WalletStep
        mode={path === "wallet" ? "login" : "link"}
        onContinue={handleWalletContinue}
        onAuth={handleAuth}
        onBack={() => setStep("entry")}
        onClose={() => window.close()}
      />}

      {step === "profile" && (
        <ProfileStep
          onComplete={handleProfileComplete}
          pubkey={pubkey}
          isHotWallet={path === "hot"}
          onClose={() => window.close()}
        />
      )}

      {step === "splash"  && <SplashStep username={username} onComplete={() => console.log("Done")} />}

      {pubkey && <TransactionSigner pubkey={pubkey} />}
    </div>
  );
}

// ---------------------------------------------------------------------------
// App root (no wallet adapter library � direct connections only)
// ---------------------------------------------------------------------------
export default function App() {
  return (
    <>
      <style>{KEYFRAMES}</style>
      <Onboarding />
    </>
  );
}

