// Tournament types for XFChess admin interface

export interface CreateTournamentReq {
  tournament_id: number;
  name: string;
  entry_fee_lamports: number;
  max_players: 8 | 16 | 32 | 64 | 128 | 256;
  format: "SingleElimination" | "Swiss";
  swiss_rounds?: number; // Required for Swiss format
  elo_min?: number;
  elo_max?: number;
  min_players?: number;
  prize_shares?: [number, number, number, number, number, number, number, number, number, number, number];
  winner_takes_all?: boolean;
  scheduled_at?: number; // Unix timestamp
  kyc_required?: boolean;
}

export interface TournamentSummary {
  tournament_id: number;
  name: string;
  entry_fee_lamports: number;
  prize_pool: number;
  max_players: number;
  registered: number;
  status: string;
}

export interface RecordResultReq {
  match_index: number;
  winner: string;
  loser: string;
}

export interface SetMatchGameIdReq {
  match_index: number;
  game_id: number;
}

export interface TournamentRecord {
  tournament_id: number;
  name: string;
  entry_fee_lamports: number;
  max_players: number;
  registered: string[];
  status: string;
  format: string;
  current_round?: number;
  total_rounds?: number;
}

export interface MatchInfo {
  match_index: number;
  player1: string;
  player2: string;
  game_id?: number;
  winner?: string;
  status: string;
}

export interface AdminAuthState {
  token: string | null;
  authenticated: boolean;
  backend_url: string;
}
