/** Backend tournament-list API helpers. */

import { request } from './client';

/**
 * Row shape of `GET /api/tournaments` — the backend's `TournamentSummary`
 * (backend/src/signing/routes/tournament.rs). `scheduled_at` is unix
 * seconds and null for an unscheduled event.
 */
export interface TournamentSummaryResponse {
  tournament_id: number;
  name: string;
  entry_fee_lamports: number;
  prize_pool: number;
  max_players: number;
  registered: number;
  status: string;
  is_private: boolean;
  is_tournament: boolean;
  usdc_mint: string | null;
  min_elo: number;
  max_elo: number;
  format: string;
  scheduled_at: number | null;
}

/** List every tournament the backend knows about. */
export function listTournaments(): Promise<TournamentSummaryResponse[]> {
  // The frontend SPA owns `/tournaments`; the API is mounted under `/api`.
  return request('/api/tournaments', { method: 'GET' });
}
