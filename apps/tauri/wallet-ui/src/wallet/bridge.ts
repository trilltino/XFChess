import * as web3 from "@solana/web3.js";

const BRIDGE_PORT = import.meta.env.DEV
  ? (import.meta.env.VITE_BRIDGE_PORT ?? "7454")
  : (typeof window !== "undefined" && window.location.port || "7454");
// Call the IPv4-only bridge at 127.0.0.1 to avoid failed ::1 attempts.
// The popup page origin stays localhost for wallet and Privy trust.
export const API_BASE = `http://127.0.0.1:${BRIDGE_PORT}`;

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
export const SESSION_ID =
  new URLSearchParams(typeof window !== "undefined" ? window.location.search : "").get("sid") ||
  `local-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;

export function logLifecycle(event: string, detail?: unknown) {
  // eslint-disable-next-line no-console
  console.log(`[Lifecycle sid=${SESSION_ID}] ${event}`, detail ?? "");
}

export async function apiGet<T = unknown>(path: string): Promise<T> {
  const resp = await fetch(`${API_BASE}${path}`, {
    headers: { "X-Session-Id": SESSION_ID },
    signal: AbortSignal.timeout(5000),
  });
  if (!resp.ok) throw new Error(`GET ${path} failed: ${resp.status}`);
  return resp.json() as Promise<T>;
}

// Ask the bridge to close its popup; Chrome may reject window.close for
// windows not opened by script.
export async function closePopup() {
  try {
    const response = await fetch(`${API_BASE}/close`, { method: "POST", headers: { "X-Session-Id": SESSION_ID }, signal: AbortSignal.timeout(5000) });
    if (!response.ok) throw new Error("Wallet window could not be closed");
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

// Unsupported network selection must not hold up the signature request.
export async function ensureDevnet(provider: any, kind: string | null): Promise<void> {
  if (!provider) return;
  if (kind === "phantom" && provider.request) {
    try {
      await withTimeout(provider.request({ method: "switchNetwork", params: { network: "devnet" } }), 5000, "Network selection");
      return;
    } catch { /* ignore — may be unsupported or already on devnet */ }
  }
  if (kind === "solflare") {
    if (provider.setCluster) {
      try {
        await withTimeout(provider.setCluster("devnet"), 5000, "Network selection");
        return;
      } catch { /* ignore */ }
    }
    if (provider.request) {
      try {
        await withTimeout(provider.request({ method: "switchNetwork", params: { network: "devnet" } }), 5000, "Network selection");
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
export async function apiPost<T = unknown>(path: string, body?: unknown, token?: string | null): Promise<T> {
  const headers: Record<string, string> = { "Content-Type": "application/json", "X-Session-Id": SESSION_ID };
  if (token) headers["Authorization"] = `Bearer ${token}`;
  const resp = await fetch(`${API_BASE}${path}`, {
    method: "POST",
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
    signal: AbortSignal.timeout(15000),
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
  return msg.includes("blockhash not found") || msg.includes("blockhashnotfound");
}

/**
 * Refresh the blockhash in place immediately before signing. On fetch
 * failure, leave the transaction untouched for the caller's retry/error path.
 */
export async function refreshBlockhash(tx: web3.Transaction | web3.VersionedTransaction): Promise<boolean> {
  const signatures = tx instanceof web3.VersionedTransaction
    ? tx.signatures ?? []
    : tx.signatures.map(entry => entry.signature);
  if (signatures.some(signature => signature?.some(byte => byte !== 0))) return false;
  try {
    const resp = await fetch(`${API_BASE}/api/fresh-blockhash`, { headers: { "X-Session-Id": SESSION_ID }, signal: AbortSignal.timeout(5000) });
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


if (typeof document !== "undefined") document.title = `XFChess #${BRIDGE_PORT} [${SESSION_ID.slice(0, 8)}]`;
