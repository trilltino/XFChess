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

export interface AuthResponse {
  token: string;
  username: string;
  wallet: string;
}

export interface LoginRequest {
  email: string;
  password: string;
}

export interface LoginResponse {
  token: string;
  username: string;
}

/** Record a signup; the confirmation email is queued for durable delivery. */
export function submitSignup(body: SignupRequest): Promise<{ ok: boolean; queued: boolean }> {
  return request('/api/signup', { method: 'POST', body: JSON.stringify(body) });
}

/** Join the waitlist; the acknowledgement email is queued for durable delivery. */
export function submitWaitlist(email: string, referral?: string): Promise<{ ok: boolean; queued: boolean }> {
  return request('/api/waitlist', { method: 'POST', body: JSON.stringify({ email, referral }) });
}

/** Create an account proving wallet ownership with a signed message. */
export function registerWithWallet(body: RegisterRequest): Promise<AuthResponse> {
  return request('/api/auth/register', { method: 'POST', body: JSON.stringify(body) });
}

/** Check whether a username is already taken. */
export function checkUsernameAvailable(username: string): Promise<{ taken: boolean }> {
  return request(`/api/auth/check-username/${encodeURIComponent(username)}`, { method: 'GET' });
}

/** Log in an email/password account. */
export function loginWithEmail(body: LoginRequest): Promise<LoginResponse> {
  return request('/api/auth/login', { method: 'POST', body: JSON.stringify(body) });
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
