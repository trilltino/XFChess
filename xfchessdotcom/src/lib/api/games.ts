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

/** Fetch recent games for a wallet. */
export function getGameHistory(wallet: string): Promise<{ games: GameHistoryRecord[] }> {
  return request(`/api/games/history/${wallet}`, { method: 'GET' });
}

/** Notify the backend of a contested game. */
export function notifyDispute(
  body: NotifyDisputeRequest,
): Promise<{ ok: boolean; case_id: string }> {
  return request('/api/dispute/notify', { method: 'POST', body: JSON.stringify(body) });
}

/** Poll the resolution status of a dispute. */
export function getDisputeStatus(gameId: number): Promise<DisputeStatus> {
  return request(`/api/dispute/${gameId}`, { method: 'GET' });
}
