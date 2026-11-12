// Wallet sign-in: ask the backend for a challenge, have Rust sign it with a keypair file
// (the secret key never reaches the webview), and exchange the signature for a session token.
// See services/backend/src/infrastructure/admin_session.rs.

import { fetch } from "@tauri-apps/plugin-http";
import { invoke } from "@tauri-apps/api/core";

const KEYPAIR_PATH_KEY = "admin_wallet_keypair_path";

/** The keypair file last used to sign in, so the next launch can sign in without any input. */
export function savedKeypairPath(): string {
  try {
    return localStorage.getItem(KEYPAIR_PATH_KEY) ?? "";
  } catch {
    return "";
  }
}

export function rememberKeypairPath(path: string) {
  try {
    localStorage.setItem(KEYPAIR_PATH_KEY, path);
  } catch {
    // Remembering is a convenience only.
  }
}

export function forgetKeypairPath() {
  try {
    localStorage.removeItem(KEYPAIR_PATH_KEY);
  } catch {
    // Nothing to forget.
  }
}

/** A plain-language reason for a refused sign-in step. */
export function signInProblem(status: number, body: string, wallet?: string): string {
  switch (status) {
    case 503:
      return "Wallet sign-in is not set up on this backend. Add your wallet's address to ADMIN_WALLETS and restart it.";
    case 403:
      return wallet
        ? `Wallet ${wallet} is not on this backend's admin list. Add  name:${wallet}  to ADMIN_WALLETS in the backend's .env and restart it.`
        : "That wallet is not on this backend's admin list (ADMIN_WALLETS).";
    case 429:
      return "Too many sign-ins are waiting. Try again in a minute.";
    case 401:
      return "The backend did not accept the signature. Try again.";
    default:
      return body.trim() || `The backend refused the sign-in (HTTP ${status}).`;
  }
}

async function post<T>(baseUrl: string, path: string, body: unknown, wallet?: string): Promise<T> {
  const response = await fetch(`${baseUrl}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  const text = await response.text();
  if (!response.ok) throw new Error(signInProblem(response.status, text, wallet));
  return JSON.parse(text) as T;
}

/**
 * Signs in with the wallet in `keypairPath` and returns the session token. It is sent exactly
 * like an API key (the X-API-Key header), so the rest of the panel needs no other change.
 */
export async function walletSessionToken(baseUrl: string, keypairPath: string): Promise<string> {
  const path = keypairPath.trim();
  if (!path) throw new Error("Choose the keypair file of an admin wallet.");
  const wallet = await invoke<string>("admin_wallet_pubkey", { path });
  const challenge = await post<{ nonce: string; message: string }>(baseUrl, "/admin/auth/challenge", { wallet }, wallet);
  const signed = await invoke<{ signature: string }>("admin_wallet_sign", { path, message: challenge.message });
  const session = await post<{ token: string }>(baseUrl, "/admin/auth/login", {
    wallet,
    nonce: challenge.nonce,
    signature: signed.signature,
  });
  return session.token;
}

/**
 * Signs in with the wallet connected in the game's wallet window (Phantom, Solflare or an embedded
 * wallet): the same signing the game uses. The wallet window opens to connect and to approve the
 * one-time challenge; nothing secret passes through the panel.
 */
export async function walletPopupSessionToken(baseUrl: string): Promise<string> {
  const wallet = await invoke<string>("admin_wallet_connect");
  const challenge = await post<{ nonce: string; message: string }>(baseUrl, "/admin/auth/challenge", { wallet }, wallet);
  const signature = await invoke<string>("admin_wallet_popup_sign", { message: challenge.message });
  const session = await post<{ token: string }>(
    baseUrl,
    "/admin/auth/login",
    { wallet, nonce: challenge.nonce, signature },
    wallet,
  );
  return session.token;
}
