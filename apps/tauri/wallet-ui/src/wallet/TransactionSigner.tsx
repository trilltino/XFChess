import { useState, useEffect, useRef } from "react";
import { Buffer } from "buffer";
import * as web3 from "@solana/web3.js";
import { usePrivy } from "@privy-io/react-auth";
import { useWallets, useSignTransaction, useSignMessage } from "@privy-io/react-auth/solana";
import { PRIVY_ENABLED, SOLANA_CHAIN } from "../privy/config";
import { API_BASE, SESSION_ID, apiGet, apiPost, withTimeout, logLifecycle, getConnectedProvider, ensureDevnet, isNetworkMismatchError, NETWORK_MISMATCH_MESSAGE } from "./bridge";
import { assertWalletAddress, validateSignedTransaction, validateMessageSignature, deserializeTransaction, serializeWalletTransaction } from "./transactions";
import { PrimaryBtn, ErrorMsg } from "./components";
import { CARD_BG, PRIMARY_BORDER, PRIMARY, TEXT, TEXT_DIM } from "./theme";

export function TransactionSigner({ pubkey }: { pubkey: string }) {
  const [pendingTx, setPendingTx] = useState<string | null>(null);
  const [pendingLabel, setPendingLabel] = useState<string | null>(null);
  const [currentRequestId, setCurrentRequestId] = useState<string | null>(null);
  const [signing, setSigning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Privy hooks return empty state safely when the provider wrapper is a passthrough.
  const { ready: privyReady, authenticated: privyAuthenticated } = usePrivy();
  const { wallets: privyWallets } = useWallets();
  const { signTransaction: privySignTransaction } = useSignTransaction();
  const { signMessage: privySignMessage } = useSignMessage();
  const hasPrivyWallet = PRIVY_ENABLED && !!privyWallets[0]?.address;
  const [bridgeProvider, setBridgeProvider] = useState<string | null>(null);
  const [bridgeStatusLoaded, setBridgeStatusLoaded] = useState(false);
  const isPrivySession = bridgeProvider === "privy";
  // Attempt each pending transaction once to avoid duplicate wallet prompts during polling.
  const autoAttempted = useRef<string | null>(null);
  const activeRequest = useRef<string | null>(null);
  const inFlight = useRef(false);
  const requestedBytes = useRef<string | null>(null);
  const requestKind = useRef<"message" | "transaction">("transaction");
  const txB64For = (id: string) => {
    if (activeRequest.current !== id || !requestedBytes.current) throw new Error("Signing request ended");
    return requestedBytes.current;
  };
  useEffect(() => () => { activeRequest.current = null; }, []);

  useEffect(() => {
    const cancelOnClose = () => {
      if (!activeRequest.current) return;
      navigator.sendBeacon(API_BASE + "/cancel?sid=" + encodeURIComponent(SESSION_ID),
        new Blob([JSON.stringify({ request_id: activeRequest.current, reason: "Wallet window closed" })], { type: "application/json" }));
    };
    window.addEventListener("pagehide", cancelOnClose);
    return () => window.removeEventListener("pagehide", cancelOnClose);
  }, []);

  // Read provider identity from the bridge; stale localStorage could route signing to the wrong wallet.
  useEffect(() => {
    let cancelled = false;
    setBridgeStatusLoaded(false);
    void withTimeout(apiGet<{ pubkey?: string; provider?: string | null }>("/status"), 5000, "Wallet session lookup")
      .then((status) => {
        if (cancelled) return;
        if (status.pubkey && status.pubkey !== pubkey) {
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
  }, [pubkey]);

  const cancelRequest = async (requestId: string, reason: string) => {
    const response = await fetch(`${API_BASE}/cancel`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Session-Id": SESSION_ID },
      body: JSON.stringify({ request_id: requestId, reason }),
      signal: AbortSignal.timeout(5000),
    });
    if (!response.ok && response.status !== 409) {
      throw new Error(`Wallet bridge rejected cancellation (${response.status})`);
    }
  };

  const resolveAndHide = async (requestId: string, signedB64: string) => {
    if (activeRequest.current !== requestId) throw new Error("This signing request has ended");
    if (requestKind.current === "message") validateMessageSignature(txB64For(requestId), signedB64, pubkey);
    else validateSignedTransaction(txB64For(requestId), signedB64, pubkey);
    const response = await fetch(API_BASE + "/resolved", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Session-Id": SESSION_ID },
      body: JSON.stringify({ request_id: requestId, signed: signedB64 }),
      signal: AbortSignal.timeout(5000),
    });
    if (!response.ok) throw new Error(`Wallet bridge rejected signing response (${response.status})`);
    if (activeRequest.current && activeRequest.current !== requestId) return;
    sessionStorage.removeItem("xfchess_auto_attempted_tx");
    setPendingTx(null);
    setPendingLabel(null);
    setCurrentRequestId(null);
    setError(null);
    await apiPost("/hide");
  };


  // Privy signs raw serialized bytes. Skip extension cluster switching because
  // embedded wallets have no user-selected network.
  const signWithPrivy = async (requestId: string, txB64: string) => {
    if (inFlight.current || activeRequest.current !== requestId) return;
    inFlight.current = true;
    setSigning(true);
    setError(null);
    try {
      const wallet = privyWallets[0];
      if (!wallet?.address) throw new Error("No Privy wallet available to sign with");
      assertWalletAddress(wallet.address, pubkey);

      if (requestKind.current === "message") {
        const { signature } = await withTimeout(privySignMessage({ message: Buffer.from(txB64, "base64"), wallet }), 60000, "Message signature");
        await resolveAndHide(requestId, Buffer.from(signature).toString("base64"));
        return;
      }

      const tx = deserializeTransaction(Buffer.from(txB64, "base64"));

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
      inFlight.current = false;
      setSigning(false);
    }
  };

  // Automatically prompt the connected extension for a pending transaction;
  // keep the button for retry after rejection or late connection.
  const signWithExtension = async (requestId: string, txB64: string) => {
    if (inFlight.current || activeRequest.current !== requestId) return;
    inFlight.current = true;
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
      assertWalletAddress(provider.publicKey?.toBase58?.() ?? provider.publicKey?.toString?.(), pubkey);
      if (requestKind.current === "message") {
        const { signature } = await withTimeout<{ signature: Uint8Array }>(provider.signMessage(Buffer.from(txB64, "base64")), 60000, "Message signature");
        await resolveAndHide(requestId, Buffer.from(signature).toString("base64"));
        return;
      }
      const txBytes = Buffer.from(txB64, "base64");
      const tx = deserializeTransaction(txBytes);
      await ensureDevnet(provider, bridgeProvider);
      logLifecycle("SIGN_REQUEST_START");
      const signed = await withTimeout<web3.VersionedTransaction | web3.Transaction>(
        provider.signTransaction(tx),
        60000,
        "Wallet signature",
      );
      logLifecycle("SIGNATURE_RECEIVED");
      await resolveAndHide(requestId, Buffer.from(serializeWalletTransaction(signed)).toString("base64"));
      logLifecycle("TX_COMPLETE");
    } catch (e: any) {
      logLifecycle("FAILED", { message: e?.message || String(e) });
      await cancelRequest(requestId, e?.message || String(e)).catch(() => {});
      setError(isNetworkMismatchError(e) ? NETWORK_MISMATCH_MESSAGE : e.message || String(e));
    } finally {
      inFlight.current = false;
      setSigning(false);
    }
  };

  // SSE pushes pending state immediately and on changes; EventSource reconnects natively.
  useEffect(() => {
    const handleUpdate = (data: { tx?: string | null; label?: string | null; request_id?: string | null; kind?: string }) => {
      if (data.tx && data.request_id !== activeRequest.current) {
        if (!data.request_id) {
          setError("Wallet bridge sent a signing request without a request ID.");
          return;
        }
        activeRequest.current = data.request_id;
        requestKind.current = data.kind === "message" ? "message" : "transaction";
        requestedBytes.current = data.tx;
        setPendingTx(data.tx);
        setPendingLabel(typeof data.label === "string" && data.label ? data.label : null);
        setCurrentRequestId(data.request_id);
        logLifecycle("SIGN_REQUEST_RECEIVED", {
          request_id: data.request_id,
          label: data.label ?? null,
        });
      } else if (!data.tx) {
        if (activeRequest.current) setError("This signing request ended. Return to the game to try again.");
        activeRequest.current = null;
        requestedBytes.current = null;
        sessionStorage.removeItem("xfchess_auto_attempted_tx");
        setPendingTx(null);
        setPendingLabel(null);
        setCurrentRequestId(null);
      }
    };

    const source = new EventSource(API_BASE + "/pending/stream?sid=" + encodeURIComponent(SESSION_ID));
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
  }, []);

  useEffect(() => {
    if (!pendingTx || !currentRequestId || !bridgeStatusLoaded || signing ||
      autoAttempted.current === currentRequestId ||
      sessionStorage.getItem("xfchess_auto_attempted_tx") === currentRequestId) return;
    if (isPrivySession ? !hasPrivyWallet : !getConnectedProvider(bridgeProvider)) return;
    autoAttempted.current = currentRequestId;
    sessionStorage.setItem("xfchess_auto_attempted_tx", currentRequestId);
    if (isPrivySession) void signWithPrivy(currentRequestId, pendingTx);
    else void signWithExtension(currentRequestId, pendingTx);
  }, [pendingTx, currentRequestId, bridgeStatusLoaded, signing, hasPrivyWallet, isPrivySession, bridgeProvider]);

  if (!pendingTx) return error ? <div role="alert"><ErrorMsg msg={error} /></div> : null;

  return (
    <div style={{
      width: "100%", marginTop: 20, padding: 20, background: CARD_BG, border: `1px solid ${PRIMARY_BORDER}`,
      borderRadius: 16, backdropFilter: "blur(20px)", animation: "fadeUp 0.3s ease",
    }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 12 }}>
        <div style={{ width: 10, height: 10, borderRadius: "50%", background: PRIMARY, animation: "pulse 1s infinite" }} />
        <span style={{ fontWeight: 800, fontSize: 13, color: TEXT }}>
          {pendingLabel ? pendingLabel.toUpperCase() : requestKind.current === "message" ? "SIGN MESSAGE" : "SIGN TRANSACTION"}
        </span>
      </div>
      <p style={{ fontSize: 11, color: TEXT_DIM, marginBottom: 12 }}>
        Wallet: {pubkey.slice(0, 6)}…{pubkey.slice(-6)}
      </p>
      <p style={{ fontSize: 12, color: TEXT_DIM, marginBottom: 16 }}>
        {signing
          ? "Signing..."
          : pendingLabel
            ? `You're signing: ${pendingLabel}`
            : "Awaiting signature."}
      </p>
      {requestKind.current === "message" && <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere", fontSize: 11, marginBottom: 12 }}>
        {Buffer.from(pendingTx, "base64").toString("utf8")}
      </pre>}
      {error && <ErrorMsg msg={error} />}
      {!signing && (
        hasPrivyWallet && isPrivySession ? (
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

