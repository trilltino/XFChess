/**
 * Authentication and account-linking endpoints.
 *
 * Covers initial signup, wallet-signed registration/login, username
 * availability checks, and JWT-based profile sync / email attach flows.
 */

import { request } from './client';

export interface SignupRequest {
  email: string;
  wallet_pubkey?: string | null;
  username?: string | null;
}

export interface RegisterRequest {
  wallet: string;
  signature: string;
  timestamp: number;
  username: string;
  email?: string | null;
}

export interface LoginRequest {
  wallet: string;
  signature: string;
  timestamp: number;
}

export interface AuthResponse {
  token: string;
  username: string;
  wallet: string;
}

/** Record a signup; the confirmation email is queued for durable delivery. */
export function submitSignup(body: SignupRequest): Promise<{ ok: boolean; queued: boolean }> {
  return request('/api/signup', { method: 'POST', body: JSON.stringify(body) });
}

/** Create an account proving wallet ownership with a signed message. */
export function registerWithWallet(body: RegisterRequest): Promise<AuthResponse> {
  return request('/api/auth/register', { method: 'POST', body: JSON.stringify(body) });
}

/** Re-establish a session for an already-registered wallet with a signed message. */
export function loginWithWallet(body: LoginRequest): Promise<AuthResponse> {
  return request('/api/auth/login', { method: 'POST', body: JSON.stringify(body) });
}

/**
 * Returns a JWT for the connected wallet, minting a fresh one via a signed
 * login message if `xfchess_token` isn't already cached in localStorage.
 *
 * `xfchess_token` is otherwise only ever set once, during first-time profile
 * creation (`ProfileViewer.tsx`'s `handleCreateProfile`) — viewing this page
 * on a return visit (wallet reconnected, on-chain profile loads fine, no
 * token in this browser/session) never re-establishes it. That silently
 * broke every JWT-gated action on the page (add-email, Lichess linking) for
 * any returning player without a cached token — invisible until
 * `/auth/lichess/init` started requiring a JWT at all (it previously took
 * none), which is what actually surfaced this gap. A wallet signature here
 * is cheap and this is the same message shape `login`/`register` already
 * use elsewhere, so this is a real re-auth, not a bypass.
 */
export async function ensureAuthToken(wallet: {
  publicKey: { toBase58(): string } | null;
  signMessage?: (msg: Uint8Array) => Promise<Uint8Array>;
}): Promise<string> {
  const cached = localStorage.getItem('xfchess_token');
  if (cached) return cached;

  if (!wallet.publicKey || !wallet.signMessage) {
    throw new Error('Wallet not connected or does not support signing messages.');
  }
  const bs58 = await import('bs58');
  const timestamp = Math.floor(Date.now() / 1000);
  const message = new TextEncoder().encode(`xfchess:login:${timestamp}`);
  const signatureBytes = await wallet.signMessage(message);
  const signature = bs58.default.encode(signatureBytes);

  const auth = await loginWithWallet({
    wallet: wallet.publicKey.toBase58(),
    signature,
    timestamp,
  });
  localStorage.setItem('xfchess_token', auth.token);
  localStorage.setItem('xfchess_username', auth.username);
  return auth.token;
}

/** Check whether a username is already taken. */
export function checkUsernameAvailable(username: string): Promise<{ taken: boolean }> {
  return request(`/api/auth/check-username/${encodeURIComponent(username)}`, { method: 'GET' });
}

/** Sync the on-chain PlayerProfile status back into the backend DB. */
export function syncProfile(token: string): Promise<{
  has_profile: boolean;
  username_set: boolean;
  is_verified: boolean;
  username: string | null;
}> {
  return request('/api/auth/sync-profile', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
  });
}

/** Attach an email to an existing wallet account (requires JWT). */
export function addEmail(email: string, token: string): Promise<{ ok: boolean }> {
  return request('/api/auth/add-email', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ email }),
  });
}

export interface InitProfileTxRequest {
  username: string;
  country: string;
  /** Unix timestamp (seconds). Must be >= 18 years before now. */
  date_of_birth: number;
}

export interface InitProfileTxResponse {
  /** Base64 bincode-serialized Transaction, already partially signed by the
   * backend as fee payer. The player still needs to sign before broadcasting. */
  tx_b64: string;
  profile_pda: string;
}

/**
 * Build a backend-sponsored `init_profile` transaction — XFChess pays the
 * on-chain rent for the player's first profile. Requires KYC to already be
 * submitted (see submitKyc in kyc.ts) and only works once per account.
 */
export function initProfileSponsoredTx(
  body: InitProfileTxRequest,
  token: string,
): Promise<InitProfileTxResponse> {
  return request('/api/auth/init-profile-sponsored-tx', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify(body),
  });
}

/** Broadcast a fully-signed transaction (base64 bincode) built by one of the
 * `*Tx` helpers above, once the player's wallet has added its signature. */
export function broadcastTx(txB64: string): Promise<{ signature: string }> {
  return request('/api/auth/broadcast-tx', {
    method: 'POST',
    body: JSON.stringify({ tx_b64: txB64 }),
  });
}
