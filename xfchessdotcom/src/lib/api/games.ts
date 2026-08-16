/**
 * Game history and dispute endpoints.
 *
 * `getGameHistory` lists recent games for a wallet; the dispute helpers
 * notify the backend of a contested game and poll its resolution status.
 */

import { request } from './client';

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

export interface NotifyDisputeRequest {
  game_id: number;
  challenger_wallet: string;
  reason: string;
  tx_signature: string;
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

// `/games/*` and `/dispute/*` are bare-mounted on the backend (see
// `backend/src/infrastructure/router.rs` — `history_router`/`dispute_router`
// are `.merge()`d with no `/api` nest, unlike `mail_router`/`kyc_router`/
// `casual_games_router`, which each bake `/api/` directly into their own
// route strings) and nginx proxies them bare too (`ops/nginx/nginx.conf`'s
// `location /games/` and `location /dispute/` blocks — see that file's own
// comment on why bare-root backend routes need their own location block or
// they silently fall through to the SPA catch-all). An `/api/` prefix here
// reaches the backend as a literal `/api/games/...`/`/api/dispute/...` path,
// which the backend has never registered — a 404, not a permissions issue.
// Confirmed live: this was breaking every game-history lookup on the
// profile page (visible in devtools as a 404 on `/api/games/history/...`).

/** Fetch recent games for a wallet. */
export function getGameHistory(wallet: string): Promise<{ games: GameHistoryRecord[] }> {
  return request(`/games/history/${wallet}`, { method: 'GET' });
}

/** Notify the backend of a contested game. */
export function notifyDispute(
  body: NotifyDisputeRequest,
): Promise<{ ok: boolean; case_id: string }> {
  return request('/dispute/notify', { method: 'POST', body: JSON.stringify(body) });
}

/** Poll the resolution status of a dispute. */
export function getDisputeStatus(gameId: number): Promise<DisputeStatus> {
  return request(`/dispute/${gameId}`, { method: 'GET' });
}
