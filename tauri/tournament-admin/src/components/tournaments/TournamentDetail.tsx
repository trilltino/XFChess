import { useState, useEffect, useCallback } from "react";
import { apiClient, type TournamentDetail } from "../../services/api";

interface TournamentDetailProps {
  tournamentId: number;
  onBack: () => void;
  onEdit: (tournamentId: number) => void;
}

interface MatchRecord {
  match_index: number;
  round: number;
  board?: number;
  player_white: string | null;
  player_black: string | null;
  winner: string | null;
  game_id: number | null;
  status: string;
  next_match_for_winner: number | null;
}

interface BracketData {
  tournament_id: number;
  status: string;
  max_players: number;
  players: string[];
  matches: (MatchRecord | null)[];
  current_round: number;
  winner?: string;
  second_place?: string;
  third_place?: string;
}

interface StandingsEntry {
  player_id: string;
  score: number;
  buchholz: number;
  sonneborn: number;
  rating: number;
  rank: number;
}

interface SwissData {
  current_round: number;
  total_rounds: number;
  standings: StandingsEntry[];
  rounds: {
    round: number;
    pairings: { white: string; black: string; board: number }[];
    byes: string[];
  }[];
}

type ResultChoice = "white" | "black" | "draw" | null;

export default function TournamentDetail({ tournamentId, onBack, onEdit }: TournamentDetailProps) {
  const [tournament, setTournament] = useState<TournamentDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState("overview");
  const [blinkCopied, setBlinkCopied] = useState(false);

  // Matches tab state
  const [bracket, setBracket] = useState<BracketData | null>(null);
  const [bracketLoading, setBracketLoading] = useState(false);
  const [swissData, setSwissData] = useState<SwissData | null>(null);
  const [startingSwiss, setStartingSwiss] = useState(false);
  const [startSwissMsg, setStartSwissMsg] = useState<string | null>(null);

  // Per-match result recording state
  const [resultChoices, setResultChoices] = useState<Record<number, ResultChoice>>({});
  const [submittingMatch, setSubmittingMatch] = useState<number | null>(null);
  const [resultMessages, setResultMessages] = useState<Record<number, string>>({});

  useEffect(() => {
    loadTournament();
  }, [tournamentId]);

  useEffect(() => {
    if (activeTab === "matches") {
      loadBracket();
    }
  }, [activeTab, tournamentId]);

  const loadTournament = async () => {
    try {
      setLoading(true);
      const response = await apiClient.getTournament(tournamentId);
      if (response.ok && response.data) {
        setTournament(response.data);
        if (response.data.swiss_data) {
          setSwissData(response.data.swiss_data);
        }
      }
    } catch (err) {
      console.error("Network error loading tournament", err);
    } finally {
      setLoading(false);
    }
  };

  const loadBracket = useCallback(async () => {
    setBracketLoading(true);
    try {
      const res = await apiClient.getTournamentBracket(tournamentId);
      if (res.ok && res.data) {
        setBracket(res.data);
      }
    } finally {
      setBracketLoading(false);
    }
  }, [tournamentId]);

  const handleStartSwiss = async () => {
    setStartingSwiss(true);
    setStartSwissMsg(null);
    const res = await apiClient.initializeSwiss(tournamentId);
    if (res.ok) {
      setStartSwissMsg(`Round 1 started — ${res.data?.players ?? "?"} players, ${res.data?.rounds ?? "?"} rounds.`);
      await loadTournament();
      await loadBracket();
    } else {
      const msg = res.error?.status === 409
        ? "Not enough players to start."
        : res.error?.message || "Failed to start Swiss.";
      setStartSwissMsg(`Error: ${msg}`);
    }
    setStartingSwiss(false);
  };

  const handleRecordResult = async (matchIndex: number, whitePlayer: string, blackPlayer: string) => {
    const choice = resultChoices[matchIndex];
    if (!choice) return;

    const winner = choice === "white" ? whitePlayer : choice === "black" ? blackPlayer : null;
    const loser  = choice === "white" ? blackPlayer : choice === "black" ? whitePlayer : null;

    // Draw: record once with white as "winner" and black as "loser" — backend treats symmetrically for Swiss
    const effectiveWinner = winner ?? whitePlayer;
    const effectiveLoser  = loser  ?? blackPlayer;

    setSubmittingMatch(matchIndex);
    const res = await apiClient.recordResult(tournamentId, matchIndex, effectiveWinner, effectiveLoser);
    if (res.ok) {
      setResultMessages(prev => ({ ...prev, [matchIndex]: "Result recorded." }));
      setResultChoices(prev => { const n = { ...prev }; delete n[matchIndex]; return n; });
      await loadBracket();
      await loadTournament();
    } else {
      setResultMessages(prev => ({ ...prev, [matchIndex]: res.error?.message || "Failed." }));
    }
    setSubmittingMatch(null);
  };

  const copyBlinkUrl = async () => {
    try {
      const baseUrl = apiClient.getBaseUrl();
      const domain = baseUrl.replace('http://', 'https://');
      const actionUrl = `https://dial.to/?action=solana-action:${domain}/api/actions/tournament/${tournament?.tournament_id}`;
      await navigator.clipboard.writeText(actionUrl);
      setBlinkCopied(true);
      setTimeout(() => setBlinkCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy", err);
    }
  };

  const formatLamports = (lamports: number) => {
    return (lamports / 1_000_000_000).toFixed(4) + " SOL";
  };

  const formatTimestamp = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

  const getStatusColor = (status: string) => {
    switch (status.toLowerCase()) {
      case "active": return "var(--primary)";
      case "completed": return "#3b82f6";
      case "scheduled": return "var(--accent)";
      case "registration": return "#4ade80";
      default: return "var(--text-dim)";
    }
  };

  const shortenKey = (key: string) => key.length > 16 ? `${key.slice(0, 8)}…${key.slice(-4)}` : key;

  // ── Tab renderers ────────────────────────────────────────────────────────────

  const renderOverview = () => {
    if (!tournament) return null;
    return (
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(300px, 1fr))", gap: "1.5rem" }}>
        <InfoCard title="TOURNAMENT CORE">
          <DetailRow label="SEQUENCE" value={`#${tournament.tournament_id}`} />
          <DetailRow label="IDENTIFIER" value={tournament.name} />
          <DetailRow label="STATUS" value={tournament.status.toUpperCase()} color={getStatusColor(tournament.status)} />
          <DetailRow label="PROTOCOL" value={tournament.format.toUpperCase()} />
          {tournament.format === "Swiss" && (
            <DetailRow label="SWISS ROUNDS" value={tournament.total_rounds || "N/A"} />
          )}
          {tournament.scheduled_at && (
            <DetailRow label="DEPLOYS AT" value={formatTimestamp(tournament.scheduled_at)} />
          )}
        </InfoCard>

        <InfoCard title="PLAYER LOAD">
          <DetailRow label="CAPACITY" value={`${tournament.players.length} / ${tournament.max_players}`} />
          <DetailRow label="ENTRY FEE" value={formatLamports(tournament.entry_fee_lamports)} />
          {tournament.elo_min && (
            <DetailRow label="ELO RANGE" value={`${tournament.elo_min} - ${tournament.elo_max || "∞"}`} />
          )}
          <DetailRow label="KYC CLEARANCE" value={tournament.kyc_required ? "REQUIRED" : "OPTIONAL"} color={tournament.kyc_required ? "var(--accent)" : "var(--text-dim)"} />
        </InfoCard>

        {tournament.entry_fee_lamports > 0 && (
          <InfoCard title="ECONOMICS">
            <DetailRow label="PLATFORM CUT" value={formatLamports(tournament.platform_fee_lamports || 0)} color="var(--text-dim)" />
            <DetailRow label="TOTAL POOL" value={formatLamports(tournament.prize_pool || 0)} color="var(--accent)" />
            {tournament.prize_shares && (
              <div style={{ marginTop: "1.5rem" }}>
                <div style={{ fontSize: "10px", color: "var(--text-dim)", marginBottom: "10px", fontWeight: "800" }}>REWARD DISTRIBUTION</div>
                {[1, 2, 3, 4].map(place => {
                  const share = tournament.prize_shares![place - 1];
                  if (share === 0) return null;
                  const amount = ((tournament.prize_pool || 0) * share) / 10000;
                  return (
                    <div key={place} style={{ display: "flex", justifyContent: "space-between", fontSize: "12px", marginBottom: "6px" }}>
                      <span style={{ color: "var(--text-dim)" }}>RANK {place}</span>
                      <span style={{ color: "#fff", fontWeight: "700" }}>{formatLamports(amount)}</span>
                    </div>
                  );
                })}
              </div>
            )}
          </InfoCard>
        )}

        <InfoCard title="MARKETING (BLINKS)">
          <div style={{ marginBottom: "1rem", color: "var(--text-dim)", fontSize: "12px", lineHeight: "1.5" }}>
            Share this Blink URL on Twitter or Discord to allow players to register instantly from their wallet.
          </div>
          <div style={{ backgroundColor: "rgba(0,0,0,0.3)", padding: "0.75rem", borderRadius: "8px", fontFamily: "monospace", fontSize: "10px", color: "var(--primary)", wordBreak: "break-all", marginBottom: "1rem", border: "1px solid rgba(255,255,255,0.05)" }}>
            https://dial.to/?action=solana-action:{apiClient.getBaseUrl().replace('http://', 'https://')}/api/actions/tournament/{tournament.tournament_id}
          </div>
          <button onClick={copyBlinkUrl} style={{ width: "100%", padding: "0.75rem", borderRadius: "100px", backgroundColor: blinkCopied ? "#4ade80" : "var(--glass)", color: blinkCopied ? "#000" : "#fff", border: blinkCopied ? "none" : "1px solid var(--border)", fontWeight: "bold", fontSize: "12px", cursor: "pointer", transition: "all 0.2s" }}>
            {blinkCopied ? "COPIED TO CLIPBOARD" : "COPY BLINK URL"}
          </button>
        </InfoCard>
      </div>
    );
  };

  const renderPlayers = () => {
    if (!tournament) return null;
    return (
      <div style={{ backgroundColor: "var(--surface)", borderRadius: "24px", overflow: "hidden", border: "1px solid var(--border)", backdropFilter: "blur(20px)" }}>
        <div style={{ padding: "1.25rem 1.5rem", backgroundColor: "rgba(255,255,255,0.05)", borderBottom: "1px solid var(--border)", fontWeight: "800", fontSize: "12px", letterSpacing: "1.5px", color: "var(--primary)" }}>
          MANIFEST: {tournament.players.length} CONNECTED ENTITIES
        </div>
        <div style={{ maxHeight: "500px", overflow: "auto", padding: "0.5rem" }}>
          {tournament.players.map((player, index) => (
            <div key={player} style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "1rem 1.25rem", borderRadius: "12px", marginBottom: "4px", transition: "background-color 0.2s ease" }}
              onMouseEnter={(e) => e.currentTarget.style.backgroundColor = "rgba(255,255,255,0.03)"}
              onMouseLeave={(e) => e.currentTarget.style.backgroundColor = "transparent"}>
              <div style={{ color: "#ffffff", fontSize: "13px", fontFamily: "'Fira Code', monospace" }}>
                <span style={{ color: "var(--primary)", marginRight: "1rem", opacity: 0.5 }}>{String(index + 1).padStart(2, '0')}</span>
                {player}
              </div>
              {tournament.player_elos && tournament.player_elos[index] && (
                <div style={{ color: "var(--accent)", fontSize: "11px", fontWeight: "bold", backgroundColor: "rgba(244, 187, 68, 0.1)", padding: "2px 8px", borderRadius: "4px" }}>
                  ELO: {tournament.player_elos[index]}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    );
  };

  const renderMatchCard = (match: MatchRecord) => {
    const white = match.player_white ?? "TBD";
    const black = match.player_black ?? "TBD";
    const isCompleted = match.status === "Completed";
    const choice = resultChoices[match.match_index] ?? null;
    const msg = resultMessages[match.match_index];

    return (
      <div key={match.match_index} style={{ backgroundColor: "rgba(255,255,255,0.03)", border: `1px solid ${isCompleted ? "rgba(59,130,246,0.3)" : "var(--border)"}`, borderRadius: "16px", padding: "1.25rem 1.5rem", marginBottom: "10px" }}>
        {/* Match header */}
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.75rem" }}>
          <div style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: "800", letterSpacing: "1px" }}>
            {match.board != null ? `BOARD ${match.board}` : `MATCH ${match.match_index}`}
            {match.round > 0 && ` · R${match.round}`}
          </div>
          <StatusPill status={match.status} />
        </div>

        {/* Players row */}
        <div style={{ display: "flex", alignItems: "center", gap: "0.75rem", marginBottom: "1rem" }}>
          <PlayerTag color="white" name={white} isWinner={isCompleted && match.winner === white} />
          <span style={{ color: "var(--text-dim)", fontSize: "11px", fontWeight: "700" }}>vs</span>
          <PlayerTag color="black" name={black} isWinner={isCompleted && match.winner === black} />
          {match.game_id != null && (
            <span style={{ marginLeft: "auto", fontSize: "10px", color: "var(--text-dim)", fontFamily: "monospace" }}>
              game #{match.game_id}
            </span>
          )}
        </div>

        {/* Result recording (only for pending/active matches with both players assigned) */}
        {!isCompleted && match.player_white && match.player_black && (
          <div>
            <div style={{ display: "flex", gap: "6px", marginBottom: "8px" }}>
              {(["white", "black", "draw"] as ResultChoice[]).map(opt => (
                <button key={opt!} onClick={() => setResultChoices(prev => ({ ...prev, [match.match_index]: opt === choice ? null : opt }))}
                  style={{ flex: 1, padding: "0.5rem", borderRadius: "8px", fontSize: "11px", fontWeight: "700", cursor: "pointer", transition: "all 0.15s",
                    backgroundColor: choice === opt ? (opt === "white" ? "rgba(255,255,255,0.15)" : opt === "black" ? "rgba(0,0,0,0.5)" : "rgba(100,100,100,0.3)") : "rgba(255,255,255,0.04)",
                    border: choice === opt ? "1px solid rgba(255,255,255,0.3)" : "1px solid var(--border)",
                    color: choice === opt ? "#fff" : "var(--text-dim)" }}>
                  {opt === "white" ? "WHITE WINS" : opt === "black" ? "BLACK WINS" : "DRAW"}
                </button>
              ))}
            </div>
            <button
              disabled={!choice || submittingMatch === match.match_index}
              onClick={() => handleRecordResult(match.match_index, white, black)}
              style={{ width: "100%", padding: "0.6rem", borderRadius: "8px", fontSize: "12px", fontWeight: "800", cursor: choice ? "pointer" : "not-allowed",
                backgroundColor: choice ? "var(--primary)" : "rgba(255,255,255,0.05)",
                color: choice ? "#000" : "var(--text-dim)", border: "none", transition: "all 0.2s" }}>
              {submittingMatch === match.match_index ? "SUBMITTING…" : "CONFIRM RESULT"}
            </button>
            {msg && <div style={{ marginTop: "6px", fontSize: "11px", color: msg.startsWith("Error") ? "#f87171" : "#4ade80", textAlign: "center" }}>{msg}</div>}
          </div>
        )}

        {/* Completed result display */}
        {isCompleted && match.winner && (
          <div style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "11px", color: "#4ade80", fontWeight: "700" }}>
            <span>WINNER:</span>
            <span style={{ fontFamily: "monospace" }}>{shortenKey(match.winner)}</span>
          </div>
        )}
      </div>
    );
  };

  const renderSwissStandings = () => {
    const standings = swissData?.standings ?? [];
    if (standings.length === 0) return null;
    return (
      <InfoCard title="STANDINGS">
        <div style={{ overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "12px" }}>
            <thead>
              <tr>
                {["#", "PLAYER", "PTS", "BUC", "SB", "ELO"].map(h => (
                  <th key={h} style={{ textAlign: "left", padding: "6px 10px", color: "var(--text-dim)", fontWeight: "800", fontSize: "10px", letterSpacing: "1px", borderBottom: "1px solid var(--border)" }}>{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {standings.map((s) => (
                <tr key={s.player_id} style={{ borderBottom: "1px solid rgba(255,255,255,0.04)" }}>
                  <td style={{ padding: "8px 10px", color: s.rank <= 3 ? "var(--accent)" : "var(--text-dim)", fontWeight: "800" }}>{s.rank}</td>
                  <td style={{ padding: "8px 10px", fontFamily: "monospace", color: "#fff" }}>{shortenKey(s.player_id)}</td>
                  <td style={{ padding: "8px 10px", color: "var(--primary)", fontWeight: "800" }}>{s.score}</td>
                  <td style={{ padding: "8px 10px", color: "var(--text-dim)" }}>{s.buchholz.toFixed(1)}</td>
                  <td style={{ padding: "8px 10px", color: "var(--text-dim)" }}>{s.sonneborn.toFixed(1)}</td>
                  <td style={{ padding: "8px 10px", color: "var(--text-dim)" }}>{s.rating}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </InfoCard>
    );
  };

  const renderMatches = () => {
    if (!tournament) return null;

    const isSwiss = tournament.format === "Swiss";
    const canStartSwiss = isSwiss && (tournament.status === "Registration" || tournament.status === "Active") && !swissData?.current_round;

    // Group matches by round for display
    const matchesByRound: Record<number, MatchRecord[]> = {};
    if (bracket?.matches) {
      for (const m of bracket.matches) {
        if (!m) continue;
        const r = m.round ?? 0;
        if (!matchesByRound[r]) matchesByRound[r] = [];
        matchesByRound[r].push(m);
      }
    }

    const rounds = Object.keys(matchesByRound).map(Number).sort((a, b) => a - b);

    return (
      <div>
        {/* Swiss control bar */}
        {isSwiss && (
          <div style={{ display: "flex", alignItems: "center", gap: "1rem", marginBottom: "1.5rem", padding: "1rem 1.5rem", backgroundColor: "rgba(255,255,255,0.03)", borderRadius: "16px", border: "1px solid var(--border)" }}>
            <div style={{ flex: 1 }}>
              {swissData ? (
                <div style={{ fontSize: "13px", color: "#fff", fontWeight: "700" }}>
                  Swiss Round <span style={{ color: "var(--primary)" }}>{swissData.current_round}</span> / {swissData.total_rounds}
                </div>
              ) : (
                <div style={{ fontSize: "13px", color: "var(--text-dim)" }}>Swiss tournament not yet started</div>
              )}
              {startSwissMsg && <div style={{ fontSize: "11px", marginTop: "4px", color: startSwissMsg.startsWith("Error") ? "#f87171" : "#4ade80" }}>{startSwissMsg}</div>}
            </div>
            {canStartSwiss && (
              <button onClick={handleStartSwiss} disabled={startingSwiss}
                style={{ padding: "0.6rem 1.5rem", borderRadius: "100px", backgroundColor: "var(--primary)", color: "#000", fontWeight: "800", fontSize: "12px", border: "none", cursor: "pointer", opacity: startingSwiss ? 0.6 : 1 }}>
                {startingSwiss ? "STARTING…" : "START SWISS"}
              </button>
            )}
          </div>
        )}

        {/* Loading state */}
        {bracketLoading && (
          <div style={{ textAlign: "center", padding: "3rem", color: "var(--text-dim)" }}>LOADING PAIRINGS…</div>
        )}

        {/* No bracket yet */}
        {!bracketLoading && rounds.length === 0 && (
          <div style={{ textAlign: "center", padding: "4rem", color: "var(--text-dim)", border: "1px dashed var(--border)", borderRadius: "24px", fontSize: "13px" }}>
            {isSwiss ? "No pairings generated yet. Start the Swiss to generate Round 1." : "Bracket not yet generated. Tournament must be started first."}
          </div>
        )}

        {/* Rounds */}
        {rounds.map(round => {
          const roundMatches = matchesByRound[round];
          const allDone = roundMatches.every(m => m.status === "Completed");
          return (
            <div key={round} style={{ marginBottom: "2rem" }}>
              <div style={{ display: "flex", alignItems: "center", gap: "0.75rem", marginBottom: "1rem" }}>
                <div style={{ fontSize: "11px", fontWeight: "800", letterSpacing: "2px", color: "var(--primary)" }}>
                  ROUND {round + 1}
                </div>
                {allDone && <span style={{ fontSize: "10px", backgroundColor: "rgba(74,222,128,0.1)", color: "#4ade80", padding: "2px 8px", borderRadius: "4px", fontWeight: "700" }}>COMPLETE</span>}
              </div>
              {roundMatches.map(renderMatchCard)}
            </div>
          );
        })}

        {/* Swiss standings */}
        {isSwiss && renderSwissStandings()}
      </div>
    );
  };

  if (loading) return <div style={{ textAlign: "center", padding: "4rem", color: "var(--text-dim)" }}>DECRYPTING DATA...</div>;
  if (!tournament) return <div style={{ textAlign: "center", padding: "4rem", color: "var(--text-dim)" }}>Tournament not found.</div>;

  return (
    <div style={{ width: "100%" }}>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "2.5rem" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "1.5rem" }}>
          <button onClick={onBack} style={{ padding: "0.6rem 1.25rem", backgroundColor: "var(--glass)", color: "var(--text-dim)", border: "1px solid var(--border)", borderRadius: "100px", cursor: "pointer", fontSize: "12px", fontWeight: "700" }}>
            ← RETURN
          </button>
          <h2 style={{ color: "#fff", margin: 0, fontSize: "28px", fontWeight: "900" }}>{tournament.name}</h2>
        </div>
        <div style={{ display: "flex", gap: "1rem" }}>
          <button onClick={() => onEdit(tournament!.tournament_id)} className="primary" style={{ padding: "0.75rem 2rem", borderRadius: "100px" }}>
            MODIFY CONFIG
          </button>
        </div>
      </div>

      {/* Nav Tabs */}
      <div style={{ marginBottom: "2rem" }}>
        <div style={{ display: "flex", gap: "8px", borderBottom: "1px solid var(--border)", paddingBottom: "1px" }}>
          {["overview", "players", "matches"].map(tab => (
            <button key={tab} onClick={() => setActiveTab(tab)} style={{ padding: "1rem 2rem", backgroundColor: "transparent", border: "none", borderBottom: activeTab === tab ? "3px solid var(--primary)" : "3px solid transparent", color: activeTab === tab ? "var(--primary)" : "var(--text-dim)", cursor: "pointer", fontSize: "11px", fontWeight: "800", letterSpacing: "1.5px", transition: "all 0.2s ease", borderRadius: 0 }}>
              {tab.toUpperCase()}
            </button>
          ))}
        </div>
      </div>

      {/* Tab Content */}
      <div style={{ animation: "fadeIn 0.4s ease" }}>
        {activeTab === "overview" && renderOverview()}
        {activeTab === "players" && renderPlayers()}
        {activeTab === "matches" && renderMatches()}
      </div>
    </div>
  );
}

// ── Utility Components ────────────────────────────────────────────────────────

const InfoCard = ({ title, children }: { title: string; children: React.ReactNode }) => (
  <div style={{ backgroundColor: "var(--surface)", padding: "2rem", borderRadius: "24px", border: "1px solid var(--border)", backdropFilter: "blur(20px)", boxShadow: "0 10px 40px rgba(0,0,0,0.3)" }}>
    <h4 style={{ color: "var(--primary)", fontSize: "11px", fontWeight: "800", letterSpacing: "2px", margin: "0 0 1.5rem 0" }}>{title}</h4>
    {children}
  </div>
);

const DetailRow = ({ label, value, color }: { label: string; value: any; color?: string }) => (
  <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "12px", alignItems: "baseline" }}>
    <span style={{ color: "var(--text-dim)", fontSize: "11px", fontWeight: "700" }}>{label}</span>
    <span style={{ color: color || "#fff", fontSize: "14px", fontWeight: "800" }}>{value}</span>
  </div>
);

const StatusPill = ({ status }: { status: string }) => {
  const colors: Record<string, { bg: string; text: string }> = {
    Pending:   { bg: "rgba(255,255,255,0.06)", text: "var(--text-dim)" },
    Active:    { bg: "rgba(74,222,128,0.12)",  text: "#4ade80" },
    Completed: { bg: "rgba(59,130,246,0.12)",  text: "#60a5fa" },
  };
  const c = colors[status] ?? colors.Pending;
  return (
    <span style={{ fontSize: "10px", fontWeight: "800", padding: "2px 8px", borderRadius: "4px", backgroundColor: c.bg, color: c.text, letterSpacing: "0.5px" }}>
      {status.toUpperCase()}
    </span>
  );
};

const PlayerTag = ({ color, name, isWinner }: { color: "white" | "black"; name: string; isWinner: boolean }) => (
  <div style={{ display: "flex", alignItems: "center", gap: "6px", flex: 1, minWidth: 0 }}>
    <div style={{ width: "10px", height: "10px", borderRadius: "50%", flexShrink: 0, backgroundColor: color === "white" ? "#e2e8f0" : "#1e293b", border: "1.5px solid rgba(255,255,255,0.2)" }} />
    <span style={{ fontSize: "12px", fontFamily: "monospace", color: isWinner ? "#4ade80" : "#cbd5e1", fontWeight: isWinner ? "800" : "400", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
      {name.length > 20 ? `${name.slice(0, 8)}…${name.slice(-4)}` : name}
    </span>
    {isWinner && <span style={{ fontSize: "10px", color: "#4ade80" }}>✓</span>}
  </div>
);
