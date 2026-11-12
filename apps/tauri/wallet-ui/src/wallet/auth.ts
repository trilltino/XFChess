import type { WalletSource } from "./types";
import { API_BASE, SESSION_ID, apiGet, apiPost, logLifecycle } from "./bridge";

export interface ProfileStatus {
  has_profile: boolean;
  username_set: boolean;
  is_verified: boolean;
  username: string | null;
}

export async function fetchProfileStatus(token: string): Promise<ProfileStatus> {
  const resp = await fetch(`${API_BASE}/api/auth/sync-profile`, {
    method: "POST",
    headers: { Authorization: `Bearer ${token}`, "X-Session-Id": SESSION_ID },
    signal: AbortSignal.timeout(15000),
  });
  if (!resp.ok) throw new Error(`sync-profile failed: ${resp.status}`);
  return resp.json();
}

async function fetchMe(token: string): Promise<{ username: string }> {
  const resp = await fetch(`${API_BASE}/api/auth/me`, {
    headers: { Authorization: `Bearer ${token}`, "X-Session-Id": SESSION_ID },
    signal: AbortSignal.timeout(15000),
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


let authToken: string | null = null;

export function setAuthToken(token: string): void { authToken = token; }

export async function getAuthToken(): Promise<string | null> {
  if (!authToken) authToken = (await apiGet<{ token: string | null }>("/token")).token;
  return authToken;
}

export interface AuthResponse {
  token: string;
  username: string;
  wallet?: string;
}

export async function authenticateWallet(src: WalletSource): Promise<AuthResponse> {
  const { pubkey, signRaw, kind } = src;
  if (!pubkey) throw new Error("No public key returned from wallet");
  localStorage.setItem("xfchess_wallet", pubkey);
  localStorage.setItem("xfchess_wallet_provider", kind);

  // Check registration status first — avoids redundant signing requests.
  logLifecycle("BACKEND_VERIFY", { path: "check-wallet" });
  const checkResp = await fetch(`${API_BASE}/api/auth/check-wallet/${pubkey}`, {
    headers: { "X-Session-Id": SESSION_ID },
    signal: AbortSignal.timeout(15000),
  });
  if (!checkResp.ok && checkResp.status !== 404) throw new Error("Could not check your wallet. Try again when the backend is available.");
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
  if (auth.wallet && auth.wallet !== pubkey) throw new Error("The backend authenticated a different wallet");
  await apiPost("/token", { token: auth.token });
  await apiPost("/wallet", { pubkey, username: auth.username, provider: kind });

  return auth;
}
