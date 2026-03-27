import {
  useMemo,
  useCallback,
  useEffect,
  useState,
  useRef,
  type CSSProperties,
} from "react";
import {
  ConnectionProvider,
  WalletProvider,
  useWallet,
  useConnection,
} from "@solana/wallet-adapter-react";
import {
  WalletModalProvider,
  WalletMultiButton,
} from "@solana/wallet-adapter-react-ui";
import {
  PhantomWalletAdapter,
  SolflareWalletAdapter,
} from "@solana/wallet-adapter-wallets";
import { clusterApiUrl, LAMPORTS_PER_SOL, VersionedTransaction } from "@solana/web3.js";
import "@solana/wallet-adapter-react-ui/styles.css";

/* ------------------------------------------------------------------ */
/*  Constants                                                         */
/* ------------------------------------------------------------------ */

const BRIDGE = window.location.origin;
const POLL_MS = 600;

type TxEntry = {
  id: number;
  status: "signed" | "failed" | "rejected";
  ts: number;
};

/* ------------------------------------------------------------------ */
/*  Inline styles                                                     */
/* ------------------------------------------------------------------ */

/* ------------------------------------------------------------------ */
/*  Helpers                                                           */
/* ------------------------------------------------------------------ */

function relativeTime(ts: number): string {
  const delta = Math.floor((Date.now() - ts) / 1000);
  if (delta < 5) return "just now";
  if (delta < 60) return `${delta}s ago`;
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  return new Date(ts).toLocaleTimeString();
}

/* ------------------------------------------------------------------ */
/*  Styles                                                            */
/* ------------------------------------------------------------------ */

const s: Record<string, CSSProperties> = {
  page: {
    minHeight: "100vh",
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    background: "#08080c",
    fontFamily: "'Inter', -apple-system, sans-serif",
    color: "#e0e0e0",
    padding: "20px 16px",
    overflow: "auto",
  },
  gridBg: {
    position: "fixed",
    inset: 0,
    backgroundImage:
      "linear-gradient(rgba(230,57,70,0.025) 1px, transparent 1px)," +
      "linear-gradient(90deg, rgba(230,57,70,0.025) 1px, transparent 1px)",
    backgroundSize: "40px 40px",
    zIndex: 0,
    pointerEvents: "none",
  },
  card: {
    position: "relative",
    zIndex: 1,
    width: "100%",
    maxWidth: "370px",
    padding: "24px 22px",
    background: "linear-gradient(145deg, rgba(255,255,255,0.035), rgba(255,255,255,0.015))",
    border: "1px solid rgba(255,255,255,0.07)",
    borderRadius: "16px",
    marginBottom: "12px",
    backdropFilter: "blur(12px)",
  },
  /* header */
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    marginBottom: "4px",
  },
  logo: {
    fontSize: "20px",
    fontWeight: 900,
    letterSpacing: "-0.04em",
    display: "flex",
    alignItems: "center",
    gap: "6px",
  },
  xf: { color: "#e63946" },
  chessIcon: {
    fontSize: "16px",
    opacity: 0.4,
  },
  badge: {
    fontSize: "8px",
    fontWeight: 700,
    letterSpacing: "0.1em",
    textTransform: "uppercase" as const,
    padding: "3px 8px",
    borderRadius: "4px",
    background: "rgba(230,57,70,0.12)",
    color: "#e63946",
    border: "1px solid rgba(230,57,70,0.2)",
  },
  glowLine: {
    height: "1px",
    background: "linear-gradient(90deg, transparent, rgba(230,57,70,0.5), transparent)",
    margin: "12px 0 16px",
    border: "none",
  },
  /* balance */
  balanceRow: {
    display: "flex",
    alignItems: "baseline",
    gap: "8px",
    marginTop: "14px",
    marginBottom: "2px",
  },
  balanceVal: {
    fontSize: "32px",
    fontWeight: 800,
    color: "#fff",
    letterSpacing: "-0.03em",
    animation: "shimmer 3s ease-in-out infinite",
    backgroundImage: "linear-gradient(110deg, #fff 40%, rgba(230,57,70,0.6) 50%, #fff 60%)",
    backgroundSize: "200% 100%",
    WebkitBackgroundClip: "text",
    WebkitTextFillColor: "transparent",
  },
  balanceUnit: {
    fontSize: "14px",
    fontWeight: 600,
    color: "rgba(255,255,255,0.3)",
  },
  /* address pill */
  addressPill: {
    display: "inline-flex",
    alignItems: "center",
    gap: "6px",
    padding: "4px 12px",
    borderRadius: "20px",
    background: "rgba(255,255,255,0.04)",
    border: "1px solid rgba(255,255,255,0.08)",
    fontSize: "11px",
    fontFamily: "monospace",
    color: "rgba(255,255,255,0.5)",
    cursor: "pointer",
    marginTop: "10px",
    transition: "all 0.2s",
  },
  copyIcon: {
    fontSize: "10px",
    opacity: 0.4,
  },
  divider: {
    border: "none",
    borderTop: "1px solid rgba(255,255,255,0.05)",
    margin: "14px 0",
  },
  /* signing prompt */
  signOverlay: {
    position: "relative",
    zIndex: 2,
    width: "100%",
    maxWidth: "370px",
    padding: "20px 24px",
    borderRadius: "16px",
    border: "1px solid rgba(230,57,70,0.5)",
    background: "linear-gradient(145deg, rgba(230,57,70,0.1), rgba(230,57,70,0.04))",
    marginBottom: "12px",
    textAlign: "center" as const,
    boxShadow: "0 0 30px rgba(230,57,70,0.1)",
  },
  signTitle: {
    fontSize: "15px",
    fontWeight: 700,
    color: "#e63946",
    marginBottom: "6px",
    letterSpacing: "0.02em",
  },
  signStep: {
    fontSize: "10px",
    fontWeight: 600,
    color: "rgba(230,57,70,0.6)",
    textTransform: "uppercase" as const,
    letterSpacing: "0.08em",
    marginBottom: "8px",
  },
  signSub: {
    fontSize: "11px",
    color: "rgba(255,255,255,0.45)",
    marginBottom: "14px",
  },
  spinner: {
    display: "inline-block",
    width: "22px",
    height: "22px",
    border: "2px solid rgba(230,57,70,0.2)",
    borderTop: "2px solid #e63946",
    borderRadius: "50%",
    animation: "spin 0.7s linear infinite",
  },
  /* toast */
  toast: {
    position: "relative",
    zIndex: 2,
    width: "100%",
    maxWidth: "370px",
    padding: "10px 16px",
    borderRadius: "10px",
    fontSize: "12px",
    fontWeight: 600,
    marginBottom: "10px",
    textAlign: "center" as const,
    animation: "fadeIn 0.3s ease",
  },
  toastOk: {
    background: "rgba(34,197,94,0.1)",
    border: "1px solid rgba(34,197,94,0.25)",
    color: "#22c55e",
    boxShadow: "0 0 20px rgba(34,197,94,0.08)",
  },
  toastFail: {
    background: "rgba(239,68,68,0.1)",
    border: "1px solid rgba(239,68,68,0.25)",
    color: "#ef4444",
    boxShadow: "0 0 20px rgba(239,68,68,0.08)",
  },
  /* history */
  historyCard: {
    position: "relative",
    zIndex: 1,
    width: "100%",
    maxWidth: "370px",
    padding: "16px 18px",
    background: "rgba(255,255,255,0.018)",
    border: "1px solid rgba(255,255,255,0.04)",
    borderRadius: "16px",
  },
  historyTitle: {
    fontSize: "9px",
    fontWeight: 700,
    letterSpacing: "0.12em",
    textTransform: "uppercase" as const,
    color: "rgba(255,255,255,0.25)",
    marginBottom: "10px",
  },
  historyRow: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "7px 0",
    borderBottom: "1px solid rgba(255,255,255,0.03)",
    fontSize: "11px",
  },
  historyStatus: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
  },
  historyDot: {
    width: "5px",
    height: "5px",
    borderRadius: "50%",
    flexShrink: 0,
  },
  historyEmpty: {
    fontSize: "11px",
    color: "rgba(255,255,255,0.15)",
    fontStyle: "italic" as const,
    textAlign: "center" as const,
    padding: "8px 0",
  },
  disconnectLink: {
    background: "none",
    border: "none",
    color: "rgba(255,255,255,0.2)",
    cursor: "pointer",
    fontSize: "10px",
    letterSpacing: "0.04em",
    padding: "4px 0",
    transition: "color 0.2s",
    textAlign: "center" as const,
    width: "100%",
    marginTop: "4px",
  },
  hint: {
    marginTop: "16px",
    color: "rgba(255,255,255,0.12)",
    fontSize: "11px",
    letterSpacing: "0.04em",
    textAlign: "center" as const,
  },
  idle: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    padding: "10px 14px",
    borderRadius: "10px",
    background: "rgba(34,197,94,0.04)",
    border: "1px solid rgba(34,197,94,0.1)",
    fontSize: "11px",
    color: "rgba(255,255,255,0.35)",
  },
  idleDot: {
    width: "6px",
    height: "6px",
    borderRadius: "50%",
    background: "#22c55e",
    flexShrink: 0,
    animation: "pulse-dot 2s ease infinite",
  },
  footer: {
    marginTop: "16px",
    fontSize: "9px",
    color: "rgba(255,255,255,0.1)",
    letterSpacing: "0.06em",
    textAlign: "center" as const,
  },
};

const CSS_KEYFRAMES = `
@keyframes spin { to { transform: rotate(360deg); } }
@keyframes shimmer { 0%,100% { background-position: 200% 0; } 50% { background-position: -200% 0; } }
@keyframes fadeIn { from { opacity: 0; transform: translateY(-4px); } to { opacity: 1; transform: translateY(0); } }
@keyframes pulse-border {
  0%, 100% { border-color: rgba(230,57,70,0.4); box-shadow: 0 0 20px rgba(230,57,70,0.05); }
  50% { border-color: rgba(230,57,70,0.8); box-shadow: 0 0 30px rgba(230,57,70,0.15); }
}
@keyframes pulse-dot {
  0%, 100% { opacity: 1; } 50% { opacity: 0.4; }
}
`;

/* ------------------------------------------------------------------ */
/*  Balance hook                                                      */
/* ------------------------------------------------------------------ */

function useBalance() {
  const { connection } = useConnection();
  const { publicKey, connected } = useWallet();
  const [sol, setSol] = useState<number | null>(null);

  useEffect(() => {
    if (!connected || !publicKey) {
      setSol(null);
      return;
    }
    let active = true;
    const refresh = async () => {
      try {
        const lamports = await connection.getBalance(publicKey);
        if (active) setSol(lamports / LAMPORTS_PER_SOL);
      } catch {
        if (active) setSol(null);
      }
    };
    refresh();
    const id = setInterval(refresh, 15_000);
    return () => { active = false; clearInterval(id); };
  }, [connected, publicKey, connection]);

  return sol;
}

/* ------------------------------------------------------------------ */
/*  Main wallet component                                             */
/* ------------------------------------------------------------------ */

function WalletPanel() {
  const { publicKey, connected, disconnect, signTransaction } = useWallet();
  const balance = useBalance();

  const [phase, setPhase] = useState<"idle" | "pending" | "signing">("idle");
  const [toast, setToast] = useState<{ msg: string; ok: boolean } | null>(null);
  const [history, setHistory] = useState<TxEntry[]>([]);
  const txIdRef = useRef(0);
  const signingRef = useRef(false);

  /* Notify bridge of wallet connect / disconnect */
  useEffect(() => {
    if (connected && publicKey) {
      fetch(`${BRIDGE}/wallet`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ pubkey: publicKey.toBase58() }),
      }).catch(() => {});
    }
  }, [connected, publicKey]);

  /* Poll for pending transactions */
  useEffect(() => {
    if (!connected || !signTransaction) return;

    let active = true;
    const poll = async () => {
      while (active) {
        try {
          const res = await fetch(`${BRIDGE}/pending`);
          const json = await res.json();
          const txB64: string | null = json.tx;

          if (txB64 && !signingRef.current) {
            signingRef.current = true;
            setPhase("pending");

            /* brief delay so user sees the prompt */
            await new Promise((r) => setTimeout(r, 400));
            setPhase("signing");

            try {
              const txBytes = Uint8Array.from(atob(txB64), (c) => c.charCodeAt(0));
              const tx = VersionedTransaction.deserialize(txBytes);
              const signed = await signTransaction(tx as never);
              const signedBytes = (signed as VersionedTransaction).serialize();
              const signedB64 = btoa(String.fromCharCode(...signedBytes));
              await fetch(`${BRIDGE}/resolved`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ signed: signedB64 }),
              });
              addHistory("signed");
              showToast("Transaction signed", true);
            } catch (e: unknown) {
              console.error("[SIGN] Signing failed:", e);
              await fetch(`${BRIDGE}/resolved`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ signed: "" }),
              }).catch(() => {});
              const msg = e instanceof Error && e.message.includes("User rejected")
                ? "rejected"
                : "failed";
              addHistory(msg === "rejected" ? "rejected" : "failed");
              showToast(
                msg === "rejected" ? "Transaction rejected" : "Signing failed",
                false
              );
            } finally {
              signingRef.current = false;
              setPhase("idle");
            }
          }
        } catch {
          /* bridge not ready */
        }
        await new Promise((r) => setTimeout(r, POLL_MS));
      }
    };
    poll();
    return () => { active = false; };
  }, [connected, signTransaction]);

  function addHistory(status: TxEntry["status"]) {
    txIdRef.current += 1;
    setHistory((h) => [{ id: txIdRef.current, status, ts: Date.now() }, ...h].slice(0, 20));
  }

  function showToast(msg: string, ok: boolean) {
    setToast({ msg, ok });
    setTimeout(() => setToast(null), 3500);
  }

  const handleDisconnect = useCallback(() => {
    disconnect();
    fetch(`${BRIDGE}/wallet`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ pubkey: null }),
    }).catch(() => {});
  }, [disconnect]);

  const [copied, setCopied] = useState(false);
  const pk = publicKey?.toBase58() ?? "";
  const shortPk = pk.length > 10 ? `${pk.slice(0, 6)}...${pk.slice(-4)}` : pk;

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(pk);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [pk]);

  return (
    <>
      {/* ---- Toast notification ---- */}
      {toast && (
        <div style={{ ...s.toast, ...(toast.ok ? s.toastOk : s.toastFail) }}>
          {toast.ok ? "\u2713" : "\u2717"} {toast.msg}
        </div>
      )}

      {/* ---- Signing prompt ---- */}
      {phase !== "idle" && (
        <div style={{ ...s.signOverlay, animation: "pulse-border 1.5s ease infinite" }}>
          <p style={s.signStep}>
            {phase === "pending" ? "Step 1/2" : "Step 2/2"}
          </p>
          <p style={s.signTitle}>
            {phase === "pending" ? "Transaction Incoming" : "Awaiting Signature"}
          </p>
          <p style={s.signSub}>
            {phase === "pending"
              ? "Preparing signing request..."
              : "Approve in your wallet extension"}
          </p>
          {phase === "signing" && <div style={s.spinner} />}
        </div>
      )}

      {/* ---- Main card ---- */}
      <div style={s.card}>
        <div style={s.header}>
          <div style={s.logo}>
            <span style={s.chessIcon}>{"\u265A"}</span>
            <span><span style={s.xf}>XF</span>Chess</span>
          </div>
          <span style={s.badge}>Devnet</span>
        </div>
        <hr style={s.glowLine} />

        <WalletMultiButton
          style={{
            width: "100%",
            justifyContent: "center",
            borderRadius: "10px",
            fontSize: "13px",
            fontWeight: 600,
            padding: "11px 0",
            background: "linear-gradient(135deg, #e63946, #c62a36)",
            border: "none",
            boxShadow: "0 4px 16px rgba(230,57,70,0.25)",
            transition: "all 0.2s",
          }}
        />

        {connected && publicKey && (
          <>
            <div
              style={s.addressPill}
              title={copied ? "Copied!" : `Click to copy: ${pk}`}
              onClick={handleCopy}
            >
              <span>{shortPk}</span>
              <span style={s.copyIcon}>{copied ? "\u2713" : "\u2398"}</span>
            </div>

            <div style={s.balanceRow}>
              <span style={s.balanceVal}>
                {balance !== null ? balance.toFixed(4) : "\u2014"}
              </span>
              <span style={s.balanceUnit}>SOL</span>
            </div>

            <hr style={s.divider} />

            {phase === "idle" && (
              <div style={s.idle}>
                <span style={s.idleDot} />
                Listening for transactions from XFChess...
              </div>
            )}

            <hr style={s.divider} />

            <button onClick={handleDisconnect} style={s.disconnectLink}>
              Disconnect wallet
            </button>
          </>
        )}

        {!connected && (
          <p style={s.hint}>Connect Phantom or Solflare to get started</p>
        )}
      </div>

      {/* ---- Transaction history ---- */}
      {connected && (
        <div style={s.historyCard}>
          <p style={s.historyTitle}>Activity</p>
          {history.length === 0 && (
            <p style={s.historyEmpty}>No transactions yet</p>
          )}
          {history.map((tx) => (
            <div key={tx.id} style={s.historyRow}>
              <span style={s.historyStatus}>
                <span style={{
                  ...s.historyDot,
                  background: tx.status === "signed" ? "#22c55e"
                    : tx.status === "rejected" ? "#eab308" : "#ef4444",
                }} />
                <span style={{ color: tx.status === "signed" ? "rgba(34,197,94,0.8)"
                  : tx.status === "rejected" ? "rgba(234,179,8,0.8)" : "rgba(239,68,68,0.8)" }}>
                  {tx.status === "signed" && "Signed"}
                  {tx.status === "failed" && "Failed"}
                  {tx.status === "rejected" && "Rejected"}
                </span>
              </span>
              <span style={{ color: "rgba(255,255,255,0.2)", fontSize: "10px" }}>
                {relativeTime(tx.ts)}
              </span>
            </div>
          ))}
        </div>
      )}

      <p style={s.footer}>Powered by Solana</p>
    </>
  );
}

/* ------------------------------------------------------------------ */
/*  App root                                                          */
/* ------------------------------------------------------------------ */

export default function App() {
  const endpoint = useMemo(() => clusterApiUrl("devnet"), []);
  const wallets = useMemo(
    () => [new PhantomWalletAdapter(), new SolflareWalletAdapter()],
    []
  );

  return (
    <ConnectionProvider endpoint={endpoint}>
      <WalletProvider wallets={wallets} autoConnect={false}>
        <WalletModalProvider>
          <style>{CSS_KEYFRAMES}</style>
          <div style={s.page}>
            <div style={s.gridBg} />
            <WalletPanel />
          </div>
        </WalletModalProvider>
      </WalletProvider>
    </ConnectionProvider>
  );
}
