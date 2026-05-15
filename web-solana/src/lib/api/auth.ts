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

/** Submit a legacy email-based signup. */
export function submitSignup(body: SignupRequest): Promise<{ ok: boolean }> {
  return request('/api/signup', { method: 'POST', body: JSON.stringify(body) });
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

/** Sync the on-chain PlayerProfile username back into the backend DB. */
export function syncProfile(token: string): Promise<{ username: string }> {
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
