import { useMemo, useCallback } from "react";
import {
  ConnectionProvider,
  WalletProvider,
  useWallet,
} from "@solana/wallet-adapter-react";
import {
  WalletModalProvider,
  WalletMultiButton,
} from "@solana/wallet-adapter-react-ui";
import { PhantomWalletAdapter, SolflareWalletAdapter } from "@solana/wallet-adapter-wallets";
import { clusterApiUrl } from "@solana/web3.js";
import "@solana/wallet-adapter-react-ui/styles.css";

declare global {
  interface Window {
    __TAURI__?: {
      core: {
        invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
      };
    };
  }
}

function WalletStatus() {
  const { publicKey, connected, disconnect } = useWallet();

  const notifyTauri = useCallback(async (pubkey: string | null) => {
    if (!window.__TAURI__) return;
    try {
      if (pubkey) {
        await window.__TAURI__.core.invoke("set_wallet_pubkey", { pubkey });
      } else {
        await window.__TAURI__.core.invoke("wallet_disconnect");
      }
    } catch (e) {
      console.error("Tauri IPC error:", e);
    }
  }, []);

  // Notify Tauri when wallet connects
  if (connected && publicKey) {
    notifyTauri(publicKey.toBase58());
  }

  return (
    <div style={{ padding: "20px", textAlign: "center" }}>
      <h1 style={{ fontSize: "24px", marginBottom: "8px", color: "#fff" }}>
        XFChess Wallet
      </h1>
      <p style={{ fontSize: "13px", color: "#888", marginBottom: "24px" }}>
        Connect your Solana wallet to play competitive matches
      </p>

      <WalletMultiButton
        style={{
          width: "100%",
          justifyContent: "center",
          borderRadius: "8px",
          fontSize: "16px",
          padding: "14px 0",
          marginBottom: "20px",
        }}
      />

      {connected && publicKey && (
        <div
          style={{
            background: "#16213e",
            borderRadius: "8px",
            padding: "16px",
            marginTop: "16px",
          }}
        >
          <p style={{ fontSize: "12px", color: "#64ffda", marginBottom: "8px" }}>
            Connected
          </p>
          <p
            style={{
              fontSize: "14px",
              fontFamily: "monospace",
              color: "#e0e0e0",
              wordBreak: "break-all",
            }}
          >
            {publicKey.toBase58()}
          </p>
          <button
            onClick={() => {
              disconnect();
              notifyTauri(null);
            }}
            style={{
              marginTop: "12px",
              padding: "8px 24px",
              borderRadius: "6px",
              border: "1px solid #ff5252",
              background: "transparent",
              color: "#ff5252",
              cursor: "pointer",
              fontSize: "13px",
            }}
          >
            Disconnect
          </button>
        </div>
      )}

      {!connected && (
        <div style={{ marginTop: "24px", color: "#666", fontSize: "12px" }}>
          <p>Supported wallets:</p>
          <p style={{ marginTop: "4px" }}>Phantom, Solflare</p>
        </div>
      )}
    </div>
  );
}

export default function App() {
  const endpoint = useMemo(() => clusterApiUrl("devnet"), []);
  const wallets = useMemo(
    () => [new PhantomWalletAdapter(), new SolflareWalletAdapter()],
    []
  );

  return (
    <ConnectionProvider endpoint={endpoint}>
      <WalletProvider wallets={wallets} autoConnect>
        <WalletModalProvider>
          <div
            style={{
              minHeight: "100vh",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              background: "#1a1a2e",
            }}
          >
            <WalletStatus />
          </div>
        </WalletModalProvider>
      </WalletProvider>
    </ConnectionProvider>
  );
}
