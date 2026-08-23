/**
 * Tournament list endpoint.
 *
 * The Swiss round/pairings/standings/result helpers that used to live here
 * went with the pages that called them (tournament_detail, _standings,
 * _play). Only the calendar remains.
 */

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
  // NOT the bare `/tournaments` — that path is also a page on this site,
  // and nginx resolves it to the SPA catch-all, answering 200 text/html.
  // Verified against production: `/tournaments` -> text/html,
  // `/api/tournaments` -> application/json.
  return request('/api/tournaments', { method: 'GET' });
}
