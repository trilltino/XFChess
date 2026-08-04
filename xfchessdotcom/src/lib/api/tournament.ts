/**
 * Swiss-format tournament endpoints.
 *
 * Round state (`getSwissCurrentRound`), pairings, standings, match result
 * recording, and the current player's pairing lookup. These are the
 * endpoints the tournament detail / play / standings pages poll while a
 * Swiss event is live.
 */

import { request } from './client';

export interface SwissCurrentRoundResponse {
  round: number;
  total_rounds: number;
  is_active: boolean;
}

export interface SwissPairing {
  white: string;
  black: string;
  board: number;
}

export interface SwissRound {
  round: number;
  pairings: SwissPairing[];
  byes: string[];
  float_downs?: string[];
  float_ups?: string[];
}

export interface SwissStandingEntry {
  player_id: string;
  score: number;
  buchholz: number;
  sonneborn: number;
  rating: number;
  rank: number;
}

export interface SwissRecordResultRequest {
  round: number;
  board: number;
  result: '1-0' | '0-1' | '0.5-0.5' | '1/2-1/2' | 'draw' | 'bye';
}

export interface TournamentMatchResponse {
  found: boolean;
  match_index?: number;
  round?: number | null;
  board?: number | null;
  game_id?: number | null;
  opponent_pubkey?: string;
  opponent_node_id?: string | null;
  your_color?: string;
  status?: string;
  is_bye?: boolean;
}

/** Get the current round state for a Swiss tournament. */
export function getSwissCurrentRound(
  tournamentId: string | number,
): Promise<SwissCurrentRoundResponse> {
  return request(`/tournament/${tournamentId}/current-round`, { method: 'GET' });
}

/** Fetch the pairings for a specific round. */
export function getSwissPairings(
  tournamentId: string | number,
  round: number,
): Promise<SwissRound> {
  return request(`/tournament/${tournamentId}/pairings/${round}`, { method: 'GET' });
}

/** Fetch the current standings (sorted by score then tiebreaks). */
export function getSwissStandings(
  tournamentId: string | number,
): Promise<SwissStandingEntry[]> {
  return request(`/tournament/${tournamentId}/standings`, { method: 'GET' });
}

/** Record a match result and return the freshly computed standings. */
export function recordSwissResult(
  tournamentId: string | number,
  body: SwissRecordResultRequest,
): Promise<SwissStandingEntry[]> {
  return request(`/tournament/${tournamentId}/result`, {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

/** Look up the current player's pairing or bye for this round. */
export function getTournamentMatch(
  tournamentId: string | number,
  player: string,
): Promise<TournamentMatchResponse> {
  const params = new URLSearchParams({ player });
  return request(
    `/tournament/${tournamentId}/my-match?${params.toString()}`,
    { method: 'GET' },
  );
}
