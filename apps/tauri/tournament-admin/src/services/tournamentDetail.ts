export interface RawTournamentDetail {
  [key: string]: any;
}

export interface TournamentDetail {
  tournament_id: number;
  name: string;
  status: string;
  max_players: number;
  entry_fee_lamports: number;
  platform_fee_lamports?: number;
  players: string[];
  player_elos?: number[];
  prize_pool?: number;
  prize_shares: [number, number, number, number, number, number, number, number, number, number];
  winner?: string;
  second_place?: string;
  third_place?: string;
  fourth_place?: string;
  kyc_required?: boolean;
  scheduled_at?: number;
  elo_min?: number;
  elo_max?: number;
  format: "SingleElimination" | "Swiss";
  current_round?: number;
  total_rounds?: number;
  round_deadline_at?: number | null;
  min_players?: number | null;
  prizes_distributed?: boolean;
  prize_release_approved?: boolean;
  /** A join password exists; the hash itself is never served. */
  is_private?: boolean;
  swiss_data?: {
    current_round: number;
    total_rounds: number;
    standings: {
      player_id: string;
      score: number;
      buchholz: number;
      sonneborn: number;
      rating: number;
      rank: number;
    }[];
    withdrawn_players?: string[];
    absent_players?: string[];
    forbidden_pairs?: [string, string][];
    manual_pairings_next_round?: { white: string; black: string }[];
    round_deadline_at?: number | null;
    rounds: {
      round: number;
      pairings: { white: string; black: string; board: number }[];
      byes: string[];
    }[];
  } | null;
}

export function normalizeTournamentDetail(raw: RawTournamentDetail): TournamentDetail {
  const rawFormat = raw.format;
  const format = typeof rawFormat === "string"
    ? rawFormat.toLowerCase().includes("swiss") ? "Swiss" : "SingleElimination"
    : rawFormat && typeof rawFormat === "object" && "Swiss" in rawFormat
      ? "Swiss"
      : "SingleElimination";
  const swissData = raw.swiss_data ?? null;

  return {
    ...raw,
    format,
    swiss_data: swissData,
    current_round: raw.current_round ?? swissData?.current_round,
    total_rounds: raw.total_rounds ?? swissData?.total_rounds,
    // The store keeps the Swiss round deadline inside swiss_data.
    round_deadline_at: raw.round_deadline_at ?? swissData?.round_deadline_at ?? null,
  } as TournamentDetail;
}
