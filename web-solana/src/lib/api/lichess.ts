/**
 * Lichess OAuth 2.0 + PKCE helpers for XFChess frontend.
 *
 * Uses the Web Crypto API (crypto.subtle) for SHA-256 hashing,
 * which is available in all modern browsers and Tauri/WebView2.
 */

import { request } from './client';

export interface LichessInitResponse {
  auth_url: string;
  state: string;
  code_challenge: string;
}

export interface LichessExchangeResponse {
  tx_signature: string;
  lichess_username: string;
  blitz_rating: number;
  rapid_rating: number;
  bullet_rating: number;
  seeded_elo: number;
}

/**
 * Generates a PKCE code_verifier: 128 random bytes → base64url (no padding).
 */
function generateCodeVerifier(): string {
  const array = new Uint8Array(128);
  crypto.getRandomValues(array);
  return base64url(array);
}

/**
 * Encodes a Uint8Array to base64url (RFC 4648 §5) without padding.
 */
function base64url(buf: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < buf.byteLength; i++) {
    binary += String.fromCharCode(buf[i]);
  }
  return btoa(binary)
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}

/**
 * Starts the Lichess OAuth flow.
 *
 * 1. Generates PKCE code_verifier + code_challenge
 * 2. Calls backend /api/auth/lichess/init to bind the flow to the wallet
 * 3. Returns the Lichess authorize URL for the popup
 */
export async function initLichessLink(walletPubkey: string): Promise<{
  authUrl: string;
  codeVerifier: string;
  state: string;
}> {
  const codeVerifier = generateCodeVerifier();

  const init = await request<LichessInitResponse>(
    `/api/auth/lichess/init?wallet_pubkey=${encodeURIComponent(walletPubkey)}`,
  );

  // Store code_verifier locally so the callback page can access it
  sessionStorage.setItem('lichess_pkce_verifier', codeVerifier);
  sessionStorage.setItem('lichess_pkce_state', init.state);
  sessionStorage.setItem('lichess_wallet_pubkey', walletPubkey);

  return {
    authUrl: init.auth_url,
    codeVerifier,
    state: init.state,
  };
}

/**
 * Completes the Lichess OAuth flow after the user is redirected back.
 *
 * Reads code_verifier from sessionStorage (set by initLichessLink),
 * POSTs to backend /api/auth/lichess/exchange, which:
 * - validates PKCE
 * - exchanges code for Lichess access token
 * - fetches user profile
 * - submits link_external_elo on-chain
 */
export async function completeLichessLink(
  code: string,
  state: string,
): Promise<LichessExchangeResponse> {
  const codeVerifier = sessionStorage.getItem('lichess_pkce_verifier');
  const storedState = sessionStorage.getItem('lichess_pkce_state');
  const walletPubkey = sessionStorage.getItem('lichess_wallet_pubkey');

  if (!codeVerifier || !storedState || !walletPubkey) {
    throw new Error('Missing PKCE session data. Did you call initLichessLink first?');
  }

  if (state !== storedState) {
    throw new Error('State mismatch — possible CSRF attack');
  }

  const result = await request<LichessExchangeResponse>('/api/auth/lichess/exchange', {
    method: 'POST',
    body: JSON.stringify({
      code,
      state,
      code_verifier: codeVerifier,
      wallet_pubkey: walletPubkey,
    }),
  });

  // Clean up session storage
  sessionStorage.removeItem('lichess_pkce_verifier');
  sessionStorage.removeItem('lichess_pkce_state');
  sessionStorage.removeItem('lichess_wallet_pubkey');

  return result;
}
