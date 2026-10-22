import { fetchProfileStatus, resolveExistingUsername, getAuthToken, setAuthToken, authenticateWallet } from "./wallet/auth";
import { API_BASE, SESSION_ID, apiGet, apiPost, closePopup, logLifecycle, withTimeout, getConnectedProvider, ensureDevnet, isNetworkMismatchError, NETWORK_MISMATCH_MESSAGE, refreshBlockhash, isStaleBlockhashError } from "./wallet/bridge";
import { PRIMARY, PRIMARY_DIM, PRIMARY_BORDER, BG, CARD_BG, BORDER, TEXT, TEXT_DIM, TEXT_MUTED, INPUT_BG, RED, RED_DIM, RED_BORDER } from "./wallet/theme";
import { PrimaryBtn, ErrorMsg } from "./wallet/components";
import { TransactionSigner } from "./wallet/TransactionSigner";
import { validateSignedTransaction, assertWalletAddress, serializeWalletTransaction } from "./wallet/transactions";

import { useState, useEffect, useRef, type CSSProperties } from "react";
import { usePrivy, useLogin, useLogout } from "@privy-io/react-auth";
import { useWallets, useSignMessage, useSignTransaction, useCreateWallet } from "@privy-io/react-auth/solana";
import type { WalletSource } from "./wallet/types";
import { connectExtension } from "./wallet/extension";
import { privyWalletSource } from "./wallet/privy";
import { PRIVY_ENABLED, SOLANA_CHAIN } from "./privy/config";

type Step = "wallet" | "profile" | "splash" | "sign";



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
  onAuth: (token: string, user: string, pubkey: string) => Promise<void>;
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
    const auth = await authenticateWallet(src);
    const { pubkey } = src;
    onContinue(pubkey, src.provider ?? null);
    await onAuth(auth.token, auth.username, pubkey);
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
      const token = await getAuthToken();
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
          assertWalletAddress(provider.publicKey?.toBase58?.() ?? provider.publicKey?.toString?.(), walletPubkey);
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

        // Retry one stale-blockhash failure with a new hash and signature. Do not
        // retry program errors, insufficient funds, or user rejection.
        const MAX_ATTEMPTS = 2;
        for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
          const tx = web3.Transaction.from(txBytes);
          let signedB64: string;
          try {
            await refreshBlockhash(tx);
            const original = tx.serialize({ requireAllSignatures: false, verifySignatures: false }).toString("base64");
            const simulation = await apiPost<{ error?: { message: string }; result?: { value?: { err: unknown } } }>(
              "/api/simulate-tx", { tx_b64: original }, token);
            if (simulation.error || !simulation.result?.value || simulation.result.value.err) {
              throw new Error(`Transaction simulation failed: ${simulation.error?.message ?? JSON.stringify(simulation.result?.value?.err)}`);
            }
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
              signedB64 = Buffer.from(serializeWalletTransaction(signed)).toString("base64");
            }
            validateSignedTransaction(original, signedB64, pubkey!);
            logLifecycle("SIGNATURE_RECEIVED");
          } catch (e: any) {
            if (isNetworkMismatchError(e)) throw new Error(NETWORK_MISMATCH_MESSAGE);
            throw new Error(e?.message || "Signature rejected — try again to finish on-chain setup.");
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
            signal: AbortSignal.timeout(15000),
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
  const sessionGeneration = useRef(0);
  useEffect(() => () => { sessionGeneration.current++; }, []);
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
  const [pubkey, setPubkey] = useState<string | null>(null);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [walletProvider, setWalletProvider] = useState<any>(null);

  useEffect(() => {
    localStorage.removeItem("xfchess_token");
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

  // Use the bridge identity for this game session.
  // Fall back to bridge wallet state when popup storage is empty so pending
  // transactions can still render their signer.
  useEffect(() => {
    let active = true;
    const generation = sessionGeneration.current;
    apiGet<{ connected: boolean; pubkey: string | null }>("/status")
      .then((s) => {
        if (!active || generation !== sessionGeneration.current) return;
        setPubkey(s.pubkey ?? "");
        if (s.pubkey) {
          localStorage.setItem("xfchess_wallet_pubkey", s.pubkey);
          setPubkey(s.pubkey);
        }
      })
      .catch(() => { /* bridge unreachable — nothing to fall back to */ });
    return () => { active = false; };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const source = new EventSource(API_BASE + "/pending/stream?sid=" + encodeURIComponent(SESSION_ID));
    source.onmessage = event => {
      try {
        const state = JSON.parse(event.data);
        if (state.tx) setStep("sign");
        else if (state.step === "wallet") {
          setStep("wallet");
          void apiGet<{ pubkey: string | null }>("/status").then(status => setPubkey(status.pubkey));
        } else if (state.step === "profile") {
          setRequireOnchain(true);
          setStep("profile");
        }
      } catch { /* Ignore malformed events; the bridge sends the next state. */ }
    };
    return () => source.close();
  }, []);

  // Poll for profile-step requests in every step except sign; a reused popup
  // can be idle on wallet or splash, but an active signature must not be interrupted.
  useEffect(() => {
    if (step === "sign") return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const r = await apiGet<{ needs_profile: boolean }>("/api/needs-profile-step");
        if (active && r.needs_profile) {
          apiPost("/api/debug-log", {
            msg: `needs-profile-step poll: flag was set (was on step="${step}") — setStep(profile), requireOnchain=true`,
          }).catch(() => {});
          // Profile-step requests require on-chain initialization, not just an off-chain handle.
          setRequireOnchain(true);
          setStep("profile");
        }
      } catch { /* Bridge may not be running. */ }
      finally { if (active) timer = setTimeout(() => { void poll(); }, 1500); }
    };
    timer = setTimeout(() => { void poll(); }, 1500);
    return () => { active = false; clearTimeout(timer); };
  }, [step]);

  // Resolve the current wallet's handle before mounting ProfileStep; shared
  // localStorage may contain another wallet's name, and the form seeds its input once.
  const [handleResolved, setHandleResolved] = useState(!requireOnchain);
  useEffect(() => {
    if (step !== "profile" || !requireOnchain || handleResolved) return;
    const activePubkey = pubkey ?? localStorage.getItem("xfchess_wallet_pubkey") ?? localStorage.getItem("xfchess_wallet");
    if (!activePubkey) { setHandleResolved(true); return; }
    let active = true;
    (async () => {
      try {
        const token = await getAuthToken();
        if (!token) return;
        const status = await fetchProfileStatus(token);
        const existing = await resolveExistingUsername(token, activePubkey, status);
        if (!active) return;
        if (existing) {
          setUsername(existing);
          localStorage.setItem("xfchess_username", existing);
        } else {
          localStorage.removeItem("xfchess_username");
          setUsername("Player");
        }
      } catch { /* backend unreachable — fall back to whatever was cached */ }
      finally { if (active) setHandleResolved(true); }
    })();
    return () => { active = false; };
  }, [step, requireOnchain, handleResolved, pubkey]);

  const handleAuth = async (token: string, user: string, nextPubkey: string) => {
    const generation = ++sessionGeneration.current;
    const current = () => generation === sessionGeneration.current;
    setAuthToken(token);
    localStorage.setItem("xfchess_wallet_pubkey", nextPubkey);
    setPubkey(nextPubkey);

    // An off-chain username does not prove an on-chain PlayerProfile exists.
    // Check on-chain setup even for returning users so failed initialization can be retried.

    // A known off-chain handle can finish normal login, but needs_profile_step
    // requires on-chain setup even in a reused popup with a stale URL.
    let resolvedUser = user;
    let needsProfile = true;
    try {
      const status = await fetchProfileStatus(token);
      if (!current()) return;
      const existing = await resolveExistingUsername(token, nextPubkey, status);
      if (!current()) return;
      if (existing) {
        resolvedUser = existing;
        localStorage.setItem("xfchess_username", resolvedUser);
        setUsername(resolvedUser);
        needsProfile = false;
      }
      if (needsProfile === false && !status.username_set) {
        const flagResp = await apiGet<{ needs_profile: boolean }>("/api/needs-profile-step");
        if (!current()) return;
        if (flagResp.needs_profile) {
          apiPost("/api/debug-log", {
            msg: "handleAuth: off-chain username resolved, but needs-profile-step flag was set — forcing ProfileStep(requireOnchain)",
          }).catch(() => {});
          setRequireOnchain(true);
          needsProfile = true;
        }
      }
    } catch {
      if (!current()) return;
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

    if (!current()) return;
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
    const token = await getAuthToken();
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
              Approve the pending wallet request below to continue.
            </p>
          </div>
          {pubkey && <TransactionSigner pubkey={pubkey} />}
        </Card>
      )}

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

