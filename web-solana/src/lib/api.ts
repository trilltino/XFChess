// Thin HTTP helpers for the XFChess backend.
// Uses VITE_BACKEND_URL when set, falls back to localhost.

const BACKEND_URL: string =
  (import.meta.env.VITE_BACKEND_URL as string | undefined) ||
  ''; // Empty string for relative paths (proxied via Tauri in prod)

export interface SignupRequest {
  email: string;
  wallet_pubkey?: string | null;
  username?: string | null;
}

export interface KycSubmission {
  wallet_pubkey: string;
  country: string;
  full_name: string;
  dob: string; // YYYY-MM-DD
  residence: string;
  tax_id: string;
}

export interface UserStatus {
  has_profile: boolean;
  has_email: boolean;
  has_kyc: boolean;
  can_wager: boolean;
}

async function request<T>(
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const res = await fetch(`${BACKEND_URL}${path}`, {
    headers: { 'Content-Type': 'application/json', ...(init.headers || {}) },
    ...init,
  });
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(text || `Request failed: ${res.status}`);
  }
  return (await res.json()) as T;
}

export function submitSignup(body: SignupRequest): Promise<{ ok: boolean }> {
  return request('/api/signup', { method: 'POST', body: JSON.stringify(body) });
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

export function registerWithWallet(body: RegisterRequest): Promise<AuthResponse> {
  return request('/api/auth/register', { method: 'POST', body: JSON.stringify(body) });
}

export function checkUsernameAvailable(username: string): Promise<{ taken: boolean }> {
  return request(`/api/auth/check-username/${encodeURIComponent(username)}`, { method: 'GET' });
}

export function submitKyc(body: KycSubmission): Promise<{ ok: boolean }> {
  return request('/api/kyc/submit', {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

export function getUserStatus(pubkey: string): Promise<UserStatus> {
  return request(`/api/user/status/${pubkey}`, { method: 'GET' });
}

export interface GameHistoryRecord {
  id: string;
  player_white: string | null;
  player_black: string | null;
  white_username: string | null;
  black_username: string | null;
  stake_amount: number;
  start_time: number;
  end_time: number | null;
  winner: string | null;
  status: string;
}

export function getGameHistory(wallet: string): Promise<{ games: GameHistoryRecord[] }> {
  return request(`/api/games/history/${wallet}`, { method: 'GET' });
}

export interface NotifyDisputeRequest {
  game_id: number;
  challenger_wallet: string;
  reason: string;
  tx_signature: string;
}

export function notifyDispute(body: NotifyDisputeRequest): Promise<{ ok: boolean; case_id: string }> {
  return request('/api/dispute/notify', { method: 'POST', body: JSON.stringify(body) });
}

export interface DisputeStatus {
  game_id: number;
  status: string;
  decision: string | null;
  resolution_text: string | null;
  tx_sig: string | null;
  notified_at: number;
  resolved_at: number | null;
}

export function getDisputeStatus(gameId: number): Promise<DisputeStatus> {
  return request(`/api/dispute/${gameId}`, { method: 'GET' });
}
