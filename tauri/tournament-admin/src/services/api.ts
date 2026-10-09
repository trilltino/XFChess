/**
 * API client for XFChess Tournament Admin
 * Provides centralized API communication with authentication, error handling, and response formatting
 */

// Rust-routed fetch, NOT the browser's. This webview is served over
// http://localhost:7454 (embedded axum server — `frontendDist` is claimed by
// wallet-ui), so a browser fetch to the backend on :8090/:8091 is cross-origin
// and dies on CORS: the request succeeds, returns 200, and the browser discards
// the response for want of an Access-Control-Allow-Origin header. It surfaces as
// an indistinguishable "failed to fetch", which is what made a working tunnel
// look like a dead one for hours.
//
// tauri-plugin-http issues the request from Rust, where there is no origin, so
// neither CORS nor this app's CSP connect-src applies. The signature is
// drop-in compatible with window.fetch. Scope: capabilities/admin-http.json.
// See docs/plans/tournament-admin-connection-rearchitecture.md §3 Phase 1.
import { fetch } from "@tauri-apps/plugin-http";
import type { SwissResultRequest, CancellationResponse, TournamentOperation } from "./adminFlows";

export interface ApiError {
  message: string;
  status?: number;
  code?: string;
}

export interface ApiResponse<T = any> {
  data?: T;
  error?: ApiError;
  ok: boolean;
}

class ApiClient {
  private baseUrl: string;
  private token: string | null = null;

  constructor(baseUrl: string = "http://127.0.0.1:8090") {
    this.baseUrl = baseUrl;
    this.loadCredentials();
  }

  private loadCredentials() {
    if (typeof localStorage === "undefined") {
      return;
    }
    this.token = localStorage.getItem("admin_token");
  }

  setCredentials(token: string, baseUrl: string) {
    this.token = token;
    this.baseUrl = baseUrl;
    if (typeof localStorage === "undefined") {
      return;
    }
    localStorage.setItem("admin_token", token);
    localStorage.setItem("backend_url", baseUrl);
  }

  clearCredentials() {
    this.token = null;
    if (typeof localStorage === "undefined") {
      return;
    }
    localStorage.removeItem("admin_token");
    localStorage.removeItem("backend_url");
  }

  private async request<T>(
    endpoint: string,
    options: RequestInit = {}
  ): Promise<ApiResponse<T>> {
    const url = `${this.baseUrl}${endpoint}`;
    
    const headers = new Headers({
      "Content-Type": "application/json",
    });

    // Add any additional headers from options
    if (options.headers) {
      Object.entries(options.headers).forEach(([key, value]) => {
        headers.set(key, value as string);
      });
    }

    if (this.token) {
      headers.set("X-API-Key", this.token);
    }

    try {
      console.log(` Fetching: ${endpoint}...`);
      const response = await fetch(url, {
        ...options,
        headers,
      });

      let data: any;
      const contentType = response.headers.get("content-type");
      
      if (contentType && contentType.includes("application/json")) {
        data = await response.json();
      } else {
        data = await response.text();
      }

      if (response.ok) {
        console.log(` Success: ${endpoint} (HTTP ${response.status})`);
        return {
          data,
          ok: true,
        };
      } else {
        const errorMsg = data?.message || data || `HTTP ${response.status}`;
        console.error(` Failed: ${endpoint} - ${errorMsg}`);
        return {
          error: {
            message: errorMsg,
            status: response.status,
            code: data?.code,
          },
          ok: false,
        };
      }
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : "Network error";
      console.error(` Network Error: ${endpoint} - ${errorMsg}`);
      return {
        error: {
          message: errorMsg,
          code: "NETWORK_ERROR",
        },
        ok: false,
      };
    }
  }

  // Tournament endpoints
  async getTournamentOperations(id: number) {
    return this.request<{ operations: TournamentOperation[] }>(`/admin/tournament/${id}/operations`);
  }
  async recordSwissResult(id: number, result: SwissResultRequest) {
    return this.request<{ ok: boolean }>(`/admin/tournament/${id}/result`, { method: "POST", body: JSON.stringify(result) });
  }

  async approvePrizeRelease(id: number) {
    return this.request<{ ok: boolean }>(`/admin/tournament/${id}/approve-prize-release`, { method: "POST", body: JSON.stringify({}) });
  }

  async getTournaments() {
    const response = await this.request<any>("/api/tournaments");
    if (!response.ok) return response as ApiResponse<TournamentSummary[]>;
    return {
      ...response,
      data: normalizeTournamentList(response.data),
    };
  }

  async getTournament(id: number) {
    return this.request<any>(`/api/tournament/${id}`);
  }

  async createTournament(data: any) {
    return this.request<any>("/admin/tournament/create", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async predictTournamentCost(data: {
    max_players: number;
    format: string;
    average_moves?: number;
  }) {
    return this.request<any>("/admin/tournament/predict-cost", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async getOperatorAffordability() {
    return this.request<any>("/admin/operator-affordability");
  }

  async getTournamentBracket(id: number) {
    return this.request<any>(`/api/tournament/${id}/bracket`);
  }

  async recordResult(tournamentId: number, matchIndex: number, winner: string, loser: string, reason?: string) {
    return this.request<any>(`/admin/tournament/${tournamentId}/record-result`, {
      method: "POST",
      body: JSON.stringify({
        match_index: matchIndex,
        winner,
        loser,
        ...(reason ? { reason } : {}),
      }),
    });
  }

  async setMatchGameId(tournamentId: number, matchIndex: number, gameId: number) {
    return this.request<any>(`/admin/tournament/${tournamentId}/set-match-game-id`, {
      method: "POST",
      body: JSON.stringify({
        match_index: matchIndex,
        game_id: gameId,
      }),
    });
  }

  async initializeSwiss(tournamentId: number) {
    return this.request<any>(`/admin/tournament/${tournamentId}/initialize-swiss`, {
      method: "POST",
      body: JSON.stringify({}),
    });
  }

  async advanceRound(tournamentId: number) {
    return this.request<any>(`/admin/tournament/${tournamentId}/advance-round`, { method: "POST", body: JSON.stringify({}) });
  }

  async reseedPlayers(tournamentId: number, players: string[]) {
    return this.request<any>(`/admin/tournament/${tournamentId}/reseed`, { method: "POST", body: JSON.stringify({ players }) });
  }

  async getEscrowBalance(tournamentId: number) {
    return this.request<any>(`/admin/tournament/${tournamentId}/escrow-balance`);
  }

  async getRegistrationReconciliation(tournamentId: number) {
    return this.request<any>(`/admin/tournament/${tournamentId}/registration-reconciliation`);
  }

  async getTournamentTransactions(tournamentId: number) {
    return this.request<any>(`/admin/tournament/${tournamentId}/transactions`);
  }

  // Offline/local events persisted by the backend. These are admin-only for
  // writes, but the backend also exposes published events at /offline-tournaments
  // for future public surfaces.
  async listOfflineTournaments() {
    return this.request<OfflineTournamentSummary[]>("/admin/offline-tournaments");
  }

  async createOfflineTournament(data: CreateOfflineTournamentRequest) {
    return this.request<OfflineTournamentRecord>("/admin/offline-tournaments", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async getOfflineTournament(tournamentId: string) {
    return this.request<OfflineTournamentRecord>(
      `/admin/offline-tournaments/${encodeURIComponent(tournamentId)}`
    );
  }

  async updateOfflineTournament(tournamentId: string, data: UpdateOfflineTournamentRequest) {
    return this.request<OfflineTournamentRecord>(
      `/admin/offline-tournaments/${encodeURIComponent(tournamentId)}`,
      {
        method: "POST",
        body: JSON.stringify(data),
      }
    );
  }

  // Tournament templates — backend-persisted (SQLite), shared across
  // machines and audit-logged. Replaces the old localStorage-only store.
  async listTemplates() {
    return this.request<{ templates: { name: string; data: any; created_at: number; updated_at: number }[] }>(
      "/admin/tournament-templates"
    );
  }

  async saveTemplate(name: string, data: any) {
    return this.request<{ ok: boolean; name: string }>("/admin/tournament-templates", {
      method: "POST",
      body: JSON.stringify({ name, data }),
    });
  }

  async deleteTemplate(name: string) {
    return this.request<{ ok: boolean; name: string }>(`/admin/tournament-templates/${encodeURIComponent(name)}`, {
      method: "DELETE",
    });
  }

  async setRoundDeadline(tournamentId: number, deadlineAt: number | null) {
    return this.request<any>(`/admin/tournament/${tournamentId}/set-round-deadline`, {
      method: "POST",
      body: JSON.stringify({ deadline_at: deadlineAt }),
    });
  }

  // Locks the guaranteed SOL prize for a tournament in its escrow PDA. Must be
  // called after creation but BEFORE the first registration — paid
  // tournaments (entry_fee_lamports > 0) reject registration on-chain until
  // this has run at least once.
  async fundTournamentPrize(tournamentId: number, amountLamports: number) {
    return this.request<{ ok: boolean; tournament_id: number; amount_lamports: number; signature: string }>(
      `/admin/tournament/${tournamentId}/fund-prize`,
      { method: "POST", body: JSON.stringify({ amount_lamports: amountLamports }) }
    );
  }

  // Cancels the tournament on-chain (refunds entry fees + returns the
  // guaranteed prize to the operator) and marks it Cancelled in the store.
  async cancelTournament(tournamentId: number) {
    return this.request<CancellationResponse>(
      `/admin/tournament/${tournamentId}/cancel`,
      { method: "POST", body: JSON.stringify({}) }
    );
  }

  // Removes a Cancelled/Completed tournament from this list — local
  // housekeeping only, does not touch on-chain state (backend rejects this
  // for any tournament not already in one of those terminal states).
  async deleteTournament(tournamentId: number) {
    return this.request<{ ok: boolean; tournament_id: number }>(
      `/admin/tournament/${tournamentId}`,
      { method: "DELETE" }
    );
  }

  // Player management
  async getPlayerHistory(wallet: string) {
    return this.request<any>(`/admin/players/${wallet}/history`);
  }

  async banPlayer(wallet: string, reason: string, durationDays?: number) {
    return this.request<any>(`/admin/players/${wallet}/ban`, { method: "POST", body: JSON.stringify({ reason, duration_days: durationDays ?? null }) });
  }

  async eloOverride(wallet: string, newElo: number, reason: string) {
    return this.request<any>(`/admin/players/${wallet}/elo-override`, { method: "POST", body: JSON.stringify({ new_elo: newElo, reason }) });
  }

  // Game admin
  async forceResign(gameId: number, winner: string) {
    return this.request<any>(`/admin/games/${gameId}/force-resign`, { method: "POST", body: JSON.stringify({ winner }) });
  }

  async flagGame(gameId: number, reason: string) {
    return this.request<any>(`/admin/games/${gameId}/flag`, { method: "POST", body: JSON.stringify({ reason }) });
  }

  // Real Stockfish-derived verdict from anticheat_verdicts (aggregate per
  // game — no move-by-move eval curve exists yet). `analysed: false` means
  // the anti-cheat worker hasn't processed this game.
  async getGameEval(gameId: number) {
    return this.request<{
      game_id: number; analysed: boolean; analysed_at?: number;
      white?: { pubkey: string; verdict: string; score: number; signals: Record<string, number> };
      black?: { pubkey: string; verdict: string; score: number; signals: Record<string, number> };
    }>(`/admin/anti-cheat/game/${gameId}/eval`);
  }

  // Audit + logs
  async getAuditLog(limit = 100) {
    return this.request<any>(`/admin/audit-log?limit=${limit}`);
  }

  async getTournamentAuditLog(tournamentId: number, limit = 100) {
    return this.request<any>(`/admin/audit-log?limit=${limit}&tournament_id=${tournamentId}`);
  }

  async getLogsStream() {
    return this.request<any>("/admin/logs/stream");
  }

  async debugTransaction(signature: string) {
    return this.request<any>(`/admin/debug/tx/${encodeURIComponent(signature)}`);
  }

  async getDebugCapabilities() {
    return this.request<any>("/admin/debug/capabilities");
  }

  async searchDebugGames(query: string, limit = 25) {
    return this.request<any>(
      `/admin/debug/games?query=${encodeURIComponent(query)}&limit=${limit}`
    );
  }

  async getGameDebugBundle(gameId: string | number) {
    return this.request<any>(`/admin/debug/games/${encodeURIComponent(String(gameId))}`);
  }

  async getGameDebugTransactions(gameId: string | number) {
    return this.request<any>(
      `/admin/debug/games/${encodeURIComponent(String(gameId))}/transactions`
    );
  }

  async retryMoneyAction(id: string) {
    return this.request<any>(
      `/admin/money-actions/${encodeURIComponent(id)}/retry-reconcile`,
      { method: "POST", body: JSON.stringify({}) }
    );
  }

  async markMoneyActionReview(id: string) {
    return this.request<any>(
      `/admin/money-actions/${encodeURIComponent(id)}/mark-review`,
      { method: "POST", body: JSON.stringify({}) }
    );
  }

  // Treasury
  async getTreasuryPayouts() { return this.request<any>("/admin/treasury/payouts"); }
  async getFeeReport(period = "week") { return this.request<any>(`/admin/treasury/fee-report?period=${period}`); }
  async manualRefund(wallet: string, lamports: number, reason: string, adminToken: string) {
    return this.request<any>("/admin/treasury/refund", { method: "POST", body: JSON.stringify({ wallet, lamports, reason, admin_token: adminToken }) });
  }

  // Infrastructure
  async getTasksStatus() { return this.request<any>("/admin/tasks/status"); }
  async getDbStats() { return this.request<any>("/admin/db/stats"); }
  async getTlsExpiry() { return this.request<any>("/admin/tls/expiry"); }
  async rotateToken() { return this.request<any>("/admin/auth/rotate-token", { method: "POST", body: JSON.stringify({}) }); }

  // Real KPI numbers (presence, confirmed txs, ER staleness/subscription,
  // fee payer balance) in one call — replaces regexing raw /metrics text.
  async getMetricsSummary() {
    return this.request<{
      online_count: number;
      games_in_progress: number;
      transactions_confirmed_solana: number;
      er_stale_delegated_count: number;
      er_subscription_connected: boolean;
      er_last_push_age_seconds: number | null;
      feepayer_balance_lamports: number;
      // Worker detail. Optional so an older backend that predates these
      // groups still type-checks and simply renders the panels empty rather
      // than crashing the dashboard.
      settlement?: Record<string, number | null>;
      schedulers?: Record<string, number | null>;
      anticheat?: Record<string, number | null>;
      prizes?: Record<string, number | null>;
      guards?: Record<string, number | null>;
    }>("/admin/metrics/summary");
  }

  // Moderation
  async ipBan(ip: string, reason: string) {
    return this.request<any>("/admin/moderation/ip-ban", { method: "POST", body: JSON.stringify({ ip, reason }) });
  }
  async getIpBans() { return this.request<any>("/admin/moderation/ip-bans"); }
  async assignDispute(gameId: number, reviewer: string) {
    return this.request<any>(`/admin/disputes/${gameId}/assign`, { method: "POST", body: JSON.stringify({ reviewer }) });
  }

  // Resolves a disputed game on-chain (WHITE_WINS/BLACK_WINS/DRAW/DISMISS).
  // admin_token is a second factor (ADMIN_TOKEN env var), distinct from the
  // X-API-Key header this client already sends on every request.
  async resolveDispute(
    gameId: number,
    decision: "WHITE_WINS" | "BLACK_WINS" | "DRAW" | "DISMISS",
    resolutionText: string,
    whiteWallet: string,
    blackWallet: string,
    adminToken: string
  ) {
    return this.request<{ ok: boolean; tx_sig: string }>("/admin/dispute/resolve", {
      method: "POST",
      body: JSON.stringify({
        game_id: gameId,
        decision,
        resolution_text: resolutionText,
        admin_token: adminToken,
        white_wallet: whiteWallet,
        black_wallet: blackWallet,
      }),
    });
  }

  // Game history endpoints
  async getGameHistory(wallet: string) {
    return this.request<any>(`/games/history/${wallet}`);
  }
 
  async getGameHistoryByUsername(username: string) {
    return this.request<any>(`/games/history/username/${username}`);
  }
 
  async getGameMoves(gameId: string) {
    return this.request<any>(`/games/moves/${gameId}`);
  }

  async getGamePgn(gameId: string) {
    return this.request<{ pgn: string }>(`/games/${gameId}/pgn`);
  }
 
  async getArchiveStats() {
    return this.request<any>("/admin/archive/stats");
  }
 
  // Download an archive via an authenticated fetch (X-API-Key header) and save
  // it through a blob URL. The token is never placed in the URL â€” the download
  // route is behind require_api_key, which only reads the header, so a plain
  // window.open() navigation (which can't set headers) would 401 anyway.
  async downloadArchive(type: "games" | "wallets"): Promise<void> {
    const res = await fetch(`${this.baseUrl}/admin/archive/download/${type}`, {
      headers: this.token ? { "X-API-Key": this.token } : {},
    });
    if (!res.ok) {
      throw new Error(`Archive download failed: HTTP ${res.status}`);
    }
    const blob = await res.blob();
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = type === "games" ? "games.xfg" : "wallets.idx";
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  }
 
  async getPlayers(limit: number = 50) {
    return this.request<any>(`/admin/players?limit=${limit}`);
  }
 
  async getActiveSessions() {
    return this.request<any>("/admin/active-sessions");
  }
 
  async getFeepayerBalance() {
    return this.request<any>("/admin/feepayer-balance");
  }

  async getWalletBalances() {
    return this.request<any>("/admin/wallet-balances");
  }

  async getAntiCheatReports() {
    return this.request<any>("/admin/anti-cheat/reports");
  }
 
  // KYC verification is fully automatic at player self-registration
  // (POST /identity/register submits the on-chain verify_profile_ix as part
  // of that flow) — there is no separate backend concept of manual admin
  // approval, so this is a read-only status lookup.
  async getKycStatus(wallet: string) {
    return this.request<{ verified: boolean; verified_at: number | null; country: string | null; requires_kyc: boolean }>(
      `/identity/status/${wallet}`
    );
  }

  // Health check
  async healthCheck() {
    return this.request<any>("/health");
  }

  // Exchange rates
  async getExchangeRates() {
    return this.request<any>("/api/rates/all");
  }

  // â”€â”€ Puzzles (admin) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
  async listPuzzles(q: {
    eloMin?: number; eloMax?: number; name?: string; theme?: string;
    limit?: number; offset?: number;
  } = {}) {
    const p = new URLSearchParams();
    if (q.eloMin != null) p.set("elo_min", String(q.eloMin));
    if (q.eloMax != null) p.set("elo_max", String(q.eloMax));
    if (q.name) p.set("name", q.name);
    if (q.theme) p.set("theme", q.theme);
    if (q.limit != null) p.set("limit", String(q.limit));
    if (q.offset != null) p.set("offset", String(q.offset));
    return this.request<any>(`/admin/puzzles?${p.toString()}`);
  }

  async getPuzzle(id: string) {
    return this.request<any>(`/admin/puzzles/${id}`);
  }

  async namePuzzle(id: string, name: string) {
    return this.request<any>(`/admin/puzzles/${id}/name`, {
      method: "POST",
      body: JSON.stringify({ name }),
    });
  }

  async featurePuzzle(id: string, featured: boolean) {
    return this.request<any>(`/admin/puzzles/${id}/feature`, {
      method: "POST",
      body: JSON.stringify({ featured }),
    });
  }

  async enablePuzzle(id: string, enabled: boolean) {
    return this.request<any>(`/admin/puzzles/${id}/enable`, {
      method: "POST",
      body: JSON.stringify({ enabled }),
    });
  }

  async fundPuzzle(body: {
    scope: "puzzle" | "band" | "daily";
    puzzle_id?: string; band_lo?: number; band_hi?: number;
    reward_lamports: number; budget_lamports: number; max_per_wallet?: number;
  }) {
    return this.request<any>("/admin/puzzles/fund", {
      method: "POST",
      body: JSON.stringify(body),
    });
  }

  async getPuzzleBounties() {
    return this.request<any>("/admin/puzzles/bounties");
  }

  async closePuzzleBounty(id: number) {
    return this.request<any>(`/admin/puzzles/bounties/${id}/close`, {
      method: "POST",
      body: JSON.stringify({}),
    });
  }

  // Helper to get base URL for Blinks
  getBaseUrl() {
    return this.baseUrl;
  }
}

// Create singleton instance
export const apiClient = new ApiClient();

export function normalizeTournamentList(data: any): TournamentSummary[] {
  if (Array.isArray(data)) return data;
  const wrapped = data?.value ?? data?.Value ?? data?.tournaments;
  return Array.isArray(wrapped) ? wrapped : [];
}

// Export types for tournament data
export interface TournamentSummary {
  tournament_id: number;
  name: string;
  entry_fee_lamports: number;
  platform_fee_lamports?: number;
  prize_pool: number;
  max_players: number;
  registered: number;
  status: string;
  is_private?: boolean;
  is_tournament?: boolean;
  source?: string;
  usdc_mint?: string | null;
  min_elo?: number;
  max_elo?: number;
  format?: string;
  scheduled_at?: number | null;
}

export interface OfflineTournamentSummary {
  tournament_id: string;
  name: string;
  format: "single_elimination" | "swiss" | string;
  status: "draft" | "published" | "active" | "completed" | "cancelled" | string;
  revision: number;
  updated_at: number;
  participants: string[];
}

export interface OfflineTournamentRecord extends OfflineTournamentSummary {
  state: any;
  created_at: number;
}

export interface CreateOfflineTournamentRequest {
  tournament_id: string;
  name: string;
  format: "single_elimination" | "swiss";
  state: any;
}

export interface UpdateOfflineTournamentRequest {
  status: "draft" | "published" | "active" | "completed" | "cancelled" | string;
  state: any;
  revision: number;
}

export interface SwissStandingsEntry {
  player_id: string;
  score: number;
  buchholz: number;
  sonneborn: number;
  rating: number;
  rank: number;
}

export interface SwissDataDetail {
  current_round: number;
  total_rounds: number;
  standings: SwissStandingsEntry[];
  rounds: {
    round: number;
    pairings: { white: string; black: string; board: number }[];
    byes: string[];
  }[];
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
  format: string;
  current_round?: number;
  total_rounds?: number;
  swiss_data?: SwissDataDetail;
}

export interface CreateTournamentRequest {
  tournament_id: number;
  name: string;
  entry_fee_lamports: number;
  platform_fee_lamports: number;
  max_players: 2 | 4 | 8 | 16 | 32 | 64 | 128 | 256;
  format: "SingleElimination" | "Swiss";
  swiss_rounds?: number;
  elo_min?: number;
  elo_max?: number;
  min_players?: number;
  prize_shares?: [number, number, number, number, number, number, number, number, number, number];
  winner_takes_all?: boolean;
  scheduled_at?: number;
  kyc_required?: boolean;
  /** Set to make the tournament private — players must supply it to register. */
  password?: string;
}

export interface MatchInfo {
  match_index: number;
  player1: string;
  player2: string;
  game_id?: number;
  winner?: string;
  status: string;
  round?: number;
}
