/**
 * API client for XFChess Tournament Admin
 * Provides centralized API communication with authentication, error handling, and response formatting
 */

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
    this.token = localStorage.getItem("admin_token");
  }

  setCredentials(token: string, baseUrl: string) {
    this.token = token;
    this.baseUrl = baseUrl;
    localStorage.setItem("admin_token", token);
    localStorage.setItem("backend_url", baseUrl);
  }

  clearCredentials() {
    this.token = null;
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
  async getTournaments() {
    return this.request<any[]>("/tournaments");
  }

  async getTournament(id: number) {
    return this.request<any>(`/tournament/${id}`);
  }

  async createTournament(data: any) {
    return this.request<any>("/admin/tournament/create", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async getTournamentBracket(id: number) {
    return this.request<any>(`/tournament/${id}/bracket`);
  }

  async recordResult(tournamentId: number, matchIndex: number, winner: string, loser: string) {
    return this.request<any>(`/admin/tournament/${tournamentId}/record-result`, {
      method: "POST",
      body: JSON.stringify({
        match_index: matchIndex,
        winner,
        loser,
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

  async getGameEval(gameId: number) {
    return this.request<any>(`/admin/anti-cheat/game/${gameId}/eval`);
  }

  // Audit + logs
  async getAuditLog(limit = 100) {
    return this.request<any>(`/admin/audit-log?limit=${limit}`);
  }

  async getLogsStream() {
    return this.request<any>("/admin/logs/stream");
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

  // Moderation
  async ipBan(ip: string, reason: string) {
    return this.request<any>("/admin/moderation/ip-ban", { method: "POST", body: JSON.stringify({ ip, reason }) });
  }
  async getIpBans() { return this.request<any>("/admin/moderation/ip-bans"); }
  async whitelistPlayer(wallet: string) {
    return this.request<any>("/admin/moderation/whitelist", { method: "POST", body: JSON.stringify({ wallet }) });
  }
  async assignDispute(gameId: number, reviewer: string) {
    return this.request<any>(`/admin/disputes/${gameId}/assign`, { method: "POST", body: JSON.stringify({ reviewer }) });
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
 
  async getKycStatus(wallet: string) {
    return this.request<any>(`/admin/kyc/status/${wallet}`);
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

