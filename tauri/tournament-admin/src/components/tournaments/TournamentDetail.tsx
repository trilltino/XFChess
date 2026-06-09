import { useState, useEffect, useCallback, useRef } from "react";
import { apiClient, type TournamentDetail } from "../../services/api";

/** Countdown clock that ticks every second until `deadlineAt` Unix timestamp. */
function RoundCountdown({ deadlineAt }: { deadlineAt: number }) {
  const [remaining, setRemaining] = useState(deadlineAt - Math.floor(Date.now() / 1000));
  useEffect(() => {
    const id = setInterval(() => setRemaining(deadlineAt - Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(id);
  }, [deadlineAt]);
  if (remaining <= 0) return <span style={{ color: "#f87171", fontWeight: 700, fontSize: "13px" }}>⏰ ROUND EXPIRED</span>;
  const h = Math.floor(remaining / 3600);
  const m = Math.floor((remaining % 3600) / 60);
  const s = remaining % 60;
  const fmt = (n: number) => String(n).padStart(2, "0");
  const color = remaining < 300 ? "#f87171" : remaining < 900 ? "#fbbf24" : "#4ade80";
  return (
    <span style={{ fontFamily: "monospace", fontSize: "14px", fontWeight: 700, color }}>
      ⏱ {h > 0 ? `${fmt(h)}:` : ""}{fmt(m)}:{fmt(s)}
    </span>
  );
}

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
  rounds: { round: number; pairings: { white: string; black: string; board: number }[]; byes: string[] }[];
}

type ResultChoice = "white" | "black" | "draw" | null;

const TEMPLATES_KEY = "tournament_templates";
function loadTemplates(): Record<string, any> {
  try { return JSON.parse(localStorage.getItem(TEMPLATES_KEY) || "{}"); } catch { return {}; }
}
function saveTemplate(name: string, data: any) {
  const t = loadTemplates(); t[name] = data;
  localStorage.setItem(TEMPLATES_KEY, JSON.stringify(t));
}

export default function TournamentDetail({ tournamentId, onBack, onEdit }: TournamentDetailProps) {
  const [tournament, setTournament] = useState<TournamentDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState("overview");
  const [blinkCopied, setBlinkCopied] = useState(false);

  const [bracket, setBracket] = useState<BracketData | null>(null);
  const [bracketLoading, setBracketLoading] = useState(false);
  const [swissData, setSwissData] = useState<SwissData | null>(null);
  const [startingSwiss, setStartingSwiss] = useState(false);
  const [startSwissMsg, setStartSwissMsg] = useState<string | null>(null);
  const [advancingRound, setAdvancingRound] = useState(false);
  const [advanceMsg, setAdvanceMsg] = useState<string | null>(null);
  const [autoAdvanceStatus, setAutoAdvanceStatus] = useState<string | null>(null);
  const prevRoundRef = useRef<number>(0);

  const [resultChoices, setResultChoices] = useState<Record<number, ResultChoice>>({});
  const [submittingMatch, setSubmittingMatch] = useState<number | null>(null);
  const [resultMessages, setResultMessages] = useState<Record<number, string>>({});
  const [gameIdInputs, setGameIdInputs] = useState<Record<number, string>>({});
  const [settingGameId, setSettingGameId] = useState<number | null>(null);

  const [reseedMode, setReseedMode] = useState(false);
  const [reseedOrder, setReseedOrder] = useState<string[]>([]);
  const [reseedMsg, setReseedMsg] = useState<string | null>(null);
  const [roundDeadlineAt, setRoundDeadlineAt] = useState<number | null>(null);
  const [deadlineMinutes, setDeadlineMinutes] = useState("60");
  const [settingDeadline, setSettingDeadline] = useState(false);
  const [bulkRegisterText, setBulkRegisterText] = useState("");
  const [bulkRegisterResults, setBulkRegisterResults] = useState<{ wallet: string; ok: boolean; msg: string }[]>([]);
  const [bulkRegistering, setBulkRegistering] = useState(false);

  const [escrowBalance, setEscrowBalance] = useState<{ balance_sol: number } | null>(null);
  const [templateName, setTemplateName] = useState("");
  const [savedTemplates] = useState<Record<string, any>>(loadTemplates);

  useEffect(() => { loadTournament(); }, [tournamentId]);
  useEffect(() => {
    if (activeTab === "matches") loadBracket();
    if (activeTab === "overview") loadEscrowBalance();
  }, [activeTab, tournamentId]);

  useEffect(() => {
    if (!swissData) return;
    const cur = swissData.current_round;
    if (autoAdvanceStatus && cur > prevRoundRef.current) {
      setAutoAdvanceStatus(`Round ${cur} pairings ready.`);
      setTimeout(() => setAutoAdvanceStatus(null), 4000);
    }
    prevRoundRef.current = cur;
  }, [swissData?.current_round]);

  const loadTournament = async () => {
    try {
      setLoading(true);
      const r = await apiClient.getTournament(tournamentId);
      if (r.ok && r.data) {
        setTournament(r.data);
        if (r.data.swiss_data) setSwissData(r.data.swiss_data);
        if (r.data.round_deadline_at != null) setRoundDeadlineAt(r.data.round_deadline_at as number);
      }
    } finally { setLoading(false); }
  };

  const handleSetDeadline = async () => {
    const mins = parseInt(deadlineMinutes, 10);
    if (isNaN(mins) || mins <= 0) return;
    setSettingDeadline(true);
    const deadlineAt = Math.floor(Date.now() / 1000) + mins * 60;
    const r = await fetch(`${apiClient.getBaseUrl()}/admin/tournament/${tournamentId}/set-round-deadline`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "Authorization": `Bearer ${localStorage.getItem("admin_token") ?? ""}` },
      body: JSON.stringify({ deadline_at: deadlineAt }),
    });
    if (r.ok) setRoundDeadlineAt(deadlineAt);
    setSettingDeadline(false);
  };

  const loadBracket = useCallback(async () => {
    setBracketLoading(true);
    try {
      const r = await apiClient.getTournamentBracket(tournamentId);
      if (r.ok && r.data) setBracket(r.data);
    } finally { setBracketLoading(false); }
  }, [tournamentId]);

  const loadEscrowBalance = async () => {
    const r = await apiClient.getEscrowBalance(tournamentId);
    if (r.ok) setEscrowBalance(r.data);
  };

  const handleStartSwiss = async () => {
    setStartingSwiss(true); setStartSwissMsg(null);
    const r = await apiClient.initializeSwiss(tournamentId);
    if (r.ok) {
      setStartSwissMsg(`Round 1 started — ${r.data?.players ?? "?"} players, ${r.data?.rounds ?? "?"} rounds.`);
      await loadTournament(); await loadBracket();
    } else {
      setStartSwissMsg(`Error: ${r.error?.status === 409 ? "Not enough players." : r.error?.message || "Failed."}`);
    }
    setStartingSwiss(false);
  };

  const handleAdvanceRound = async () => {
    setAdvancingRound(true); setAdvanceMsg(null);
    const r = await apiClient.advanceRound(tournamentId);
    if (r.ok) {
      setAdvanceMsg(`Advanced to round ${r.data?.new_round ?? "?"}.`);
      setAutoAdvanceStatus("Waiting for new pairings…");
      await loadTournament(); await loadBracket();
    } else {
      setAdvanceMsg(`Error: ${r.error?.message || "Not all matches complete?"}`);
    }
    setAdvancingRound(false);
  };

  const handleRecordResult = async (matchIndex: number, white: string, black: string, forfeit = false) => {
    const choice = forfeit ? "white" : resultChoices[matchIndex];
    if (!choice) return;
    const winner = choice === "white" ? white : choice === "black" ? black : null;
    const loser  = choice === "white" ? black  : choice === "black" ? white : null;
    setSubmittingMatch(matchIndex);
    const r = await apiClient.recordResult(tournamentId, matchIndex, winner ?? white, loser ?? black);
    if (r.ok) {
      setResultMessages(prev => ({ ...prev, [matchIndex]: forfeit ? "Forfeit recorded." : "Result recorded." }));
      setResultChoices(prev => { const n = { ...prev }; delete n[matchIndex]; return n; });
      await loadBracket(); await loadTournament();
    } else {
      setResultMessages(prev => ({ ...prev, [matchIndex]: r.error?.message || "Failed." }));
    }
    setSubmittingMatch(null);
  };

  const handleSetGameId = async (matchIndex: number) => {
    const gameId = parseInt(gameIdInputs[matchIndex]);
    if (isNaN(gameId)) return;
    setSettingGameId(matchIndex);
    const r = await apiClient.setMatchGameId(tournamentId, matchIndex, gameId);
    setResultMessages(prev => ({ ...prev, [matchIndex]: r.ok ? `Game #${gameId} linked.` : "Failed to set." }));
    if (r.ok) await loadBracket();
    setSettingGameId(null);
  };

  const handleReseed = async () => {
    const r = await apiClient.reseedPlayers(tournamentId, reseedOrder);
    setReseedMsg(r.ok ? "Players reseeded." : `Error: ${r.error?.message}`);
    if (r.ok) { setReseedMode(false); await loadTournament(); }
  };

  const handleBulkRegister = async () => {
    const wallets = bulkRegisterText.split(/[\n,]+/).map(s => s.trim()).filter(Boolean);
    if (!wallets.length) return;
    setBulkRegistering(true);
    const results: { wallet: string; ok: boolean; msg: string }[] = [];
    for (const wallet of wallets) {
      const r = await fetch(`${apiClient.getBaseUrl()}/tournament/${tournamentId}/join`, {
        method: "POST", headers: { "Content-Type": "application/json", "Authorization": `Bearer ${localStorage.getItem("admin_token")}` },
        body: JSON.stringify({ player: wallet }),
      });
      results.push({ wallet, ok: r.ok, msg: r.ok ? "Joined" : await r.text() });
    }
    setBulkRegisterResults(results); setBulkRegistering(false);
    await loadTournament();
  };

  const copyBlinkUrl = async () => {
    const url = `https://dial.to/?action=solana-action:${apiClient.getBaseUrl().replace("http://", "https://")}/api/actions/tournament/${tournament?.tournament_id}`;
    await navigator.clipboard.writeText(url).catch(() => {});
    setBlinkCopied(true); setTimeout(() => setBlinkCopied(false), 2000);
  };

  const fmt = (l: number) => (l / 1e9).toFixed(4) + " SOL";
  const fmtTs = (t: number) => new Date(t * 1000).toLocaleString();
  const statusColor = (s: string) => ({ active: "var(--primary)", completed: "#3b82f6", scheduled: "var(--accent)", registration: "#4ade80" }[s.toLowerCase()] ?? "var(--text-dim)");
  const shorten = (k: string) => k.length > 16 ? `${k.slice(0, 8)}…${k.slice(-4)}` : k;

  // ── Overview ──────────────────────────────────────────────────────────────────
  const renderOverview = () => {
    if (!tournament) return null;
    return (
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(300px, 1fr))", gap: "1.5rem" }}>
        <InfoCard title="TOURNAMENT CORE">
          <Row label="ID" value={`#${tournament.tournament_id}`} />
          <Row label="NAME" value={tournament.name} />
          <Row label="STATUS" value={tournament.status.toUpperCase()} color={statusColor(tournament.status)} />
          <Row label="FORMAT" value={tournament.format.toUpperCase()} />
          {tournament.format === "Swiss" && <Row label="ROUNDS" value={tournament.total_rounds || "—"} />}
          {tournament.scheduled_at && <Row label="STARTS" value={fmtTs(tournament.scheduled_at)} />}
        </InfoCard>

        <InfoCard title="PLAYER LOAD">
          <Row label="CAPACITY" value={`${tournament.players.length} / ${tournament.max_players}`} />
          <Row label="ENTRY FEE" value={fmt(tournament.entry_fee_lamports)} />
          {tournament.elo_min && <Row label="ELO RANGE" value={`${tournament.elo_min} – ${tournament.elo_max || "∞"}`} />}
          <Row label="KYC" value={tournament.kyc_required ? "REQUIRED" : "OPTIONAL"} color={tournament.kyc_required ? "var(--accent)" : "var(--text-dim)"} />
        </InfoCard>

        {tournament.entry_fee_lamports > 0 && (
          <InfoCard title="ECONOMICS">
            <Row label="PLATFORM CUT" value={fmt(tournament.platform_fee_lamports || 0)} color="var(--text-dim)" />
            <Row label="PRIZE POOL" value={fmt(tournament.prize_pool || 0)} color="var(--accent)" />
            {escrowBalance != null && <Row label="ESCROW LIVE" value={`${escrowBalance.balance_sol.toFixed(4)} SOL`} color="#4ade80" />}
            {tournament.prize_shares && (
              <div style={{ marginTop: "1rem" }}>
                <div style={{ fontSize: "10px", color: "var(--text-dim)", marginBottom: "8px", fontWeight: "800" }}>DISTRIBUTION</div>
                {[1, 2, 3, 4].map(p => {
                  const share = tournament.prize_shares![p - 1];
                  if (!share) return null;
                  return <div key={p} style={{ display: "flex", justifyContent: "space-between", fontSize: "12px", marginBottom: "4px" }}>
                    <span style={{ color: "var(--text-dim)" }}>#{p}</span>
                    <span style={{ color: "#fff", fontWeight: "700" }}>{fmt((tournament.prize_pool || 0) * share / 10000)}</span>
                  </div>;
                })}
              </div>
            )}
          </InfoCard>
        )}

        <InfoCard title="BLINKS">
          <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: "0 0 12px" }}>Share on Twitter/Discord for instant wallet registration.</p>
          <div style={{ background: "rgba(0,0,0,0.3)", padding: "10px", borderRadius: "8px", fontFamily: "monospace", fontSize: "10px", color: "var(--primary)", wordBreak: "break-all", marginBottom: "12px" }}>
            dial.to/?action=solana-action:{apiClient.getBaseUrl().replace("http://", "https://")}/api/actions/tournament/{tournament.tournament_id}
          </div>
          <button onClick={copyBlinkUrl} style={{ width: "100%", padding: "0.75rem", borderRadius: "100px", backgroundColor: blinkCopied ? "#4ade80" : "var(--glass)", color: blinkCopied ? "#000" : "#fff", border: blinkCopied ? "none" : "1px solid var(--border)", fontWeight: "bold", fontSize: "12px", cursor: "pointer" }}>
            {blinkCopied ? "COPIED" : "COPY BLINK URL"}
          </button>
        </InfoCard>

        <InfoCard title="TEMPLATE">
          <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: "0 0 12px" }}>Save this config as a named preset for reuse.</p>
          <div style={{ display: "flex", gap: "8px" }}>
            <input value={templateName} onChange={e => setTemplateName(e.target.value)} placeholder="Template name…"
              style={{ flex: 1, background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 12px", fontSize: "12px" }} />
            <button onClick={() => { if (templateName && tournament) { saveTemplate(templateName, tournament); setTemplateName(""); alert(`Saved "${templateName}"`); }}}
              style={{ padding: "8px 16px", borderRadius: "8px", backgroundColor: "var(--primary)", color: "#000", border: "none", fontWeight: "700", fontSize: "12px", cursor: "pointer" }}>SAVE</button>
          </div>
          {Object.keys(savedTemplates).length > 0 && (
            <div style={{ marginTop: "10px", fontSize: "11px", color: "var(--text-dim)" }}>Saved: {Object.keys(savedTemplates).join(", ")}</div>
          )}
        </InfoCard>
      </div>
    );
  };

  // ── Players tab ───────────────────────────────────────────────────────────────
  const renderPlayers = () => {
    if (!tournament) return null;
    const list = reseedMode ? reseedOrder : tournament.players;

    return (
      <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
        <div style={{ backgroundColor: "var(--surface)", borderRadius: "24px", overflow: "hidden", border: "1px solid var(--border)" }}>
          <div style={{ padding: "1rem 1.5rem", backgroundColor: "rgba(255,255,255,0.05)", borderBottom: "1px solid var(--border)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontWeight: "800", fontSize: "12px", letterSpacing: "1.5px", color: "var(--primary)" }}>
              {tournament.players.length} PLAYERS
            </span>
            <div style={{ display: "flex", gap: "8px" }}>
              {!reseedMode && tournament.status === "Registration" && (
                <button onClick={() => { setReseedOrder([...tournament.players]); setReseedMode(true); }}
                  style={{ padding: "6px 14px", borderRadius: "8px", background: "rgba(255,255,255,0.08)", color: "var(--text-dim)", border: "1px solid var(--border)", fontSize: "11px", cursor: "pointer" }}>
                  RESEED ↕
                </button>
              )}
              {reseedMode && <>
                <button onClick={handleReseed} style={{ padding: "6px 14px", borderRadius: "8px", backgroundColor: "var(--primary)", color: "#000", border: "none", fontSize: "11px", fontWeight: "700", cursor: "pointer" }}>APPLY</button>
                <button onClick={() => setReseedMode(false)} style={{ padding: "6px 14px", borderRadius: "8px", background: "transparent", color: "var(--text-dim)", border: "1px solid var(--border)", fontSize: "11px", cursor: "pointer" }}>CANCEL</button>
              </>}
            </div>
          </div>
          {reseedMsg && <div style={{ padding: "8px 1.5rem", fontSize: "11px", color: reseedMsg.startsWith("Error") ? "#f87171" : "#4ade80" }}>{reseedMsg}</div>}
          <div style={{ maxHeight: "400px", overflow: "auto", padding: "0.5rem" }}>
            {list.map((p, i) => (
              <div key={p} draggable={reseedMode}
                onDragOver={e => e.preventDefault()}
                onDrop={() => {
                  const arr = [...reseedOrder];
                  const from = arr.indexOf(p);
                  const [item] = arr.splice(from, 1);
                  arr.splice(i, 0, item);
                  setReseedOrder(arr);
                }}
                style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "0.9rem 1.25rem", borderRadius: "10px", marginBottom: "2px", cursor: reseedMode ? "grab" : "default" }}
                onMouseEnter={e => e.currentTarget.style.background = "rgba(255,255,255,0.03)"}
                onMouseLeave={e => e.currentTarget.style.background = "transparent"}>
                <div style={{ color: "#fff", fontSize: "13px", fontFamily: "monospace" }}>
                  {reseedMode && <span style={{ color: "var(--text-dim)", marginRight: "8px" }}>⠿</span>}
                  <span style={{ color: "var(--primary)", marginRight: "1rem", opacity: 0.5 }}>{String(i + 1).padStart(2, "0")}</span>
                  {p}
                </div>
                {tournament.player_elos?.[i] && (
                  <span style={{ color: "var(--accent)", fontSize: "11px", fontWeight: "bold", background: "rgba(244,187,68,0.1)", padding: "2px 8px", borderRadius: "4px" }}>ELO {tournament.player_elos[i]}</span>
                )}
              </div>
            ))}
          </div>
        </div>

        <InfoCard title="BULK REGISTER">
          <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: "0 0 12px" }}>Paste wallet addresses (one per line or comma-separated).</p>
          <textarea value={bulkRegisterText} onChange={e => setBulkRegisterText(e.target.value)}
            placeholder={"wallet1\nwallet2\nwallet3"}
            style={{ width: "100%", height: "90px", background: "rgba(255,255,255,0.04)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 12px", fontSize: "12px", fontFamily: "monospace", resize: "vertical", boxSizing: "border-box" }} />
          <div style={{ display: "flex", gap: "8px", marginTop: "8px", flexWrap: "wrap" }}>
            <button onClick={handleBulkRegister} disabled={bulkRegistering || !bulkRegisterText.trim()}
              style={{ padding: "8px 20px", borderRadius: "8px", backgroundColor: "var(--primary)", color: "#000", border: "none", fontWeight: "700", fontSize: "12px", cursor: "pointer", opacity: bulkRegistering ? 0.6 : 1 }}>
              {bulkRegistering ? "REGISTERING…" : "BULK REGISTER"}
            </button>
            <label style={{ padding: "8px 16px", borderRadius: "8px", backgroundColor: "rgba(100,180,100,0.15)", color: "#86efac", border: "1px solid rgba(100,180,100,0.3)", fontSize: "12px", fontWeight: "700", cursor: "pointer" }}>
              CSV IMPORT
              <input type="file" accept=".csv,.txt" style={{ display: "none" }} onChange={async (e) => {
                const file = e.target.files?.[0]; if (!file) return;
                const text = await file.text();
                setBulkRegistering(true);
                const r = await fetch(`${apiClient.getBaseUrl()}/admin/tournament/${tournamentId}/import-players-csv`, {
                  method: "POST",
                  headers: { "Content-Type": "text/plain", "Authorization": `Bearer ${localStorage.getItem("admin_token") ?? ""}` },
                  body: text,
                });
                const json = r.ok ? await r.json() : null;
                if (json?.results) {
                  setBulkRegisterResults(json.results.map((row: any) => ({ wallet: row.player, ok: row.status === "added", msg: row.status === "added" ? "Added" : "Already registered" })));
                }
                setBulkRegistering(false);
                await loadTournament();
                e.target.value = "";
              }} />
            </label>
          </div>
          {bulkRegisterResults.length > 0 && (
            <div style={{ marginTop: "12px", maxHeight: "130px", overflowY: "auto" }}>
              {bulkRegisterResults.map((r, i) => (
                <div key={i} style={{ fontSize: "11px", fontFamily: "monospace", color: r.ok ? "#4ade80" : "#f87171", marginBottom: "2px" }}>
                  {r.ok ? "✓" : "✗"} {r.wallet.slice(0, 14)}… — {r.msg}
                </div>
              ))}
            </div>
          )}
        </InfoCard>
      </div>
    );
  };

  // ── Match card ────────────────────────────────────────────────────────────────
  const renderMatchCard = (match: MatchRecord) => {
    const white = match.player_white ?? "TBD";
    const black = match.player_black ?? "TBD";
    const done = match.status === "Completed";
    const choice = resultChoices[match.match_index] ?? null;
    const msg = resultMessages[match.match_index];

    return (
      <div key={match.match_index} style={{ backgroundColor: "rgba(255,255,255,0.03)", border: `1px solid ${done ? "rgba(59,130,246,0.3)" : "var(--border)"}`, borderRadius: "16px", padding: "1.25rem 1.5rem", marginBottom: "10px" }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.75rem" }}>
          <span style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: "800", letterSpacing: "1px" }}>
            {match.board != null ? `BOARD ${match.board}` : `MATCH ${match.match_index}`}{match.round > 0 && ` · R${match.round}`}
          </span>
          <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
            {match.game_id != null && <span style={{ fontSize: "10px", color: "var(--text-dim)", fontFamily: "monospace" }}>#{match.game_id}</span>}
            {!done && match.player_white && match.player_black && match.game_id && (
              <button onClick={() => navigator.clipboard.writeText(`xfchess://spectate/${match.game_id}`).catch(() => {})}
                style={{ fontSize: "10px", padding: "2px 8px", borderRadius: "4px", backgroundColor: "rgba(59,130,246,0.15)", color: "#60a5fa", border: "1px solid rgba(59,130,246,0.3)", cursor: "pointer" }}>
                SPECTATE ↗
              </button>
            )}
            <StatusPill status={match.status} />
          </div>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: "0.75rem", marginBottom: "1rem" }}>
          <PlayerTag color="white" name={white} isWinner={done && match.winner === white} />
          <span style={{ color: "var(--text-dim)", fontSize: "11px" }}>vs</span>
          <PlayerTag color="black" name={black} isWinner={done && match.winner === black} />
        </div>

        {!done && match.player_white && match.player_black && (
          <div>
            <div style={{ display: "flex", gap: "6px", marginBottom: "6px" }}>
              {(["white", "black", "draw"] as ResultChoice[]).map(opt => (
                <button key={opt!} onClick={() => setResultChoices(p => ({ ...p, [match.match_index]: opt === choice ? null : opt }))}
                  style={{ flex: 1, padding: "0.45rem", borderRadius: "8px", fontSize: "11px", fontWeight: "700", cursor: "pointer",
                    backgroundColor: choice === opt ? (opt === "white" ? "rgba(255,255,255,0.15)" : opt === "black" ? "rgba(0,0,0,0.5)" : "rgba(100,100,100,0.3)") : "rgba(255,255,255,0.04)",
                    border: choice === opt ? "1px solid rgba(255,255,255,0.3)" : "1px solid var(--border)", color: choice === opt ? "#fff" : "var(--text-dim)" }}>
                  {opt === "white" ? "WHITE" : opt === "black" ? "BLACK" : "DRAW"}
                </button>
              ))}
            </div>
            <div style={{ display: "flex", gap: "6px", marginBottom: "6px" }}>
              <button disabled={!choice || submittingMatch === match.match_index}
                onClick={() => handleRecordResult(match.match_index, white, black)}
                style={{ flex: 2, padding: "0.55rem", borderRadius: "8px", fontSize: "12px", fontWeight: "800",
                  backgroundColor: choice ? "var(--primary)" : "rgba(255,255,255,0.05)", color: choice ? "#000" : "var(--text-dim)", border: "none", cursor: choice ? "pointer" : "not-allowed" }}>
                {submittingMatch === match.match_index ? "SUBMITTING…" : "CONFIRM"}
              </button>
              <button onClick={() => handleRecordResult(match.match_index, white, black, true)} disabled={submittingMatch === match.match_index}
                style={{ flex: 1, padding: "0.55rem", borderRadius: "8px", fontSize: "11px", fontWeight: "700",
                  backgroundColor: "rgba(239,68,68,0.12)", color: "#f87171", border: "1px solid rgba(239,68,68,0.3)", cursor: "pointer" }}>
                FORFEIT
              </button>
            </div>
            <div style={{ display: "flex", gap: "6px" }}>
              <input value={gameIdInputs[match.match_index] ?? ""} onChange={e => setGameIdInputs(p => ({ ...p, [match.match_index]: e.target.value }))}
                placeholder="Set game ID…" type="number"
                style={{ flex: 1, background: "rgba(255,255,255,0.05)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "5px 10px", fontSize: "11px" }} />
              <button onClick={() => handleSetGameId(match.match_index)} disabled={settingGameId === match.match_index}
                style={{ padding: "5px 14px", borderRadius: "8px", background: "rgba(255,255,255,0.08)", color: "var(--text-dim)", border: "1px solid var(--border)", fontSize: "11px", cursor: "pointer" }}>
                {settingGameId === match.match_index ? "…" : "LINK"}
              </button>
            </div>
            {msg && <div style={{ marginTop: "6px", fontSize: "11px", color: msg.startsWith("Error") || msg.startsWith("Failed") ? "#f87171" : "#4ade80", textAlign: "center" }}>{msg}</div>}
          </div>
        )}

        {done && match.winner && (
          <div style={{ fontSize: "11px", color: "#4ade80", fontWeight: "700" }}>WINNER: <span style={{ fontFamily: "monospace" }}>{shorten(match.winner)}</span></div>
        )}
      </div>
    );
  };

  // ── Matches tab ───────────────────────────────────────────────────────────────
  const renderMatches = () => {
    if (!tournament) return null;
    const isSwiss = tournament.format === "Swiss";
    const canStart = isSwiss && (tournament.status === "Registration" || tournament.status === "Active") && !swissData?.current_round;
    const curRound = swissData?.current_round ?? 0;

    const byRound: Record<number, MatchRecord[]> = {};
    for (const m of bracket?.matches ?? []) {
      if (!m) continue;
      const r = m.round ?? 0;
      (byRound[r] ??= []).push(m);
    }
    const rounds = Object.keys(byRound).map(Number).sort((a, b) => a - b);

    const curMatches = byRound[curRound] ?? [];
    const canAdvance = isSwiss && curRound > 0 && curMatches.length > 0 && curMatches.every(m => m.status === "Completed") && curRound < (swissData?.total_rounds ?? 0);

    return (
      <div>
        {isSwiss && (
          <div style={{ display: "flex", alignItems: "center", gap: "1rem", marginBottom: "1.5rem", padding: "1rem 1.5rem", backgroundColor: "rgba(255,255,255,0.03)", borderRadius: "16px", border: "1px solid var(--border)" }}>
            <div style={{ flex: 1 }}>
              {swissData
                ? <div style={{ fontSize: "13px", color: "#fff", fontWeight: "700" }}>Swiss Round <span style={{ color: "var(--primary)" }}>{swissData.current_round}</span> / {swissData.total_rounds}</div>
                : <div style={{ fontSize: "13px", color: "var(--text-dim)" }}>Swiss not yet started</div>
              }
              {startSwissMsg && <div style={{ fontSize: "11px", marginTop: "4px", color: startSwissMsg.startsWith("Error") ? "#f87171" : "#4ade80" }}>{startSwissMsg}</div>}
              {advanceMsg && <div style={{ fontSize: "11px", marginTop: "4px", color: advanceMsg.startsWith("Error") ? "#f87171" : "#4ade80" }}>{advanceMsg}</div>}
              {autoAdvanceStatus && (
                <div style={{ fontSize: "11px", marginTop: "4px", color: "var(--accent)", display: "flex", alignItems: "center", gap: "6px" }}>
                  <span style={{ display: "inline-block", width: "7px", height: "7px", borderRadius: "50%", backgroundColor: "var(--accent)" }} />
                  {autoAdvanceStatus}
                </div>
              )}
            </div>
            {swissData && swissData.current_round > 0 && (
              <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                {roundDeadlineAt && roundDeadlineAt > Math.floor(Date.now() / 1000)
                  ? <RoundCountdown deadlineAt={roundDeadlineAt} />
                  : null}
                <input
                  type="number" value={deadlineMinutes} min={1} max={480}
                  onChange={e => setDeadlineMinutes(e.target.value)}
                  style={{ width: "56px", padding: "4px 6px", borderRadius: "6px", border: "1px solid var(--border)", backgroundColor: "var(--surface)", color: "white", fontSize: "12px" }}
                />
                <span style={{ fontSize: "11px", color: "var(--text-dim)" }}>min</span>
                <button onClick={handleSetDeadline} disabled={settingDeadline}
                  style={{ padding: "4px 10px", borderRadius: "8px", backgroundColor: "rgba(100,100,220,0.2)", color: "#a0a0ff", fontSize: "11px", border: "1px solid rgba(100,100,220,0.4)", cursor: "pointer" }}>
                  SET DEADLINE
                </button>
              </div>
            )}
            {canStart && (
              <button onClick={handleStartSwiss} disabled={startingSwiss}
                style={{ padding: "0.6rem 1.5rem", borderRadius: "100px", backgroundColor: "var(--primary)", color: "#000", fontWeight: "800", fontSize: "12px", border: "none", cursor: "pointer", opacity: startingSwiss ? 0.6 : 1 }}>
                {startingSwiss ? "STARTING…" : "START SWISS"}
              </button>
            )}
            {canAdvance && (
              <button onClick={handleAdvanceRound} disabled={advancingRound}
                style={{ padding: "0.6rem 1.5rem", borderRadius: "100px", backgroundColor: "rgba(244,187,68,0.15)", color: "var(--accent)", fontWeight: "800", fontSize: "12px", border: "1px solid rgba(244,187,68,0.35)", cursor: "pointer", opacity: advancingRound ? 0.6 : 1 }}>
                {advancingRound ? "ADVANCING…" : "ADVANCE ROUND ▶"}
              </button>
            )}
          </div>
        )}

        {bracketLoading && <div style={{ textAlign: "center", padding: "3rem", color: "var(--text-dim)" }}>LOADING PAIRINGS…</div>}
        {!bracketLoading && rounds.length === 0 && (
          <div style={{ textAlign: "center", padding: "4rem", color: "var(--text-dim)", border: "1px dashed var(--border)", borderRadius: "24px" }}>
            {isSwiss ? "No pairings yet. Start Swiss to generate Round 1." : "Bracket not yet generated."}
          </div>
        )}

        {rounds.map(r => {
          const rMatches = byRound[r];
          const allDone = rMatches.every(m => m.status === "Completed");
          const byes = swissData?.rounds.find(sr => sr.round === r + 1)?.byes ?? [];
          return (
            <div key={r} style={{ marginBottom: "2rem" }}>
              <div style={{ display: "flex", alignItems: "center", gap: "0.75rem", marginBottom: "1rem" }}>
                <div style={{ fontSize: "11px", fontWeight: "800", letterSpacing: "2px", color: "var(--primary)" }}>ROUND {r + 1}</div>
                {allDone && <span style={{ fontSize: "10px", backgroundColor: "rgba(74,222,128,0.1)", color: "#4ade80", padding: "2px 8px", borderRadius: "4px", fontWeight: "700" }}>COMPLETE</span>}
                {byes.length > 0 && <span style={{ fontSize: "10px", color: "var(--text-dim)", fontStyle: "italic" }}>BYE: {byes.map(shorten).join(", ")} (+1 pt)</span>}
              </div>
              {rMatches.map(renderMatchCard)}
            </div>
          );
        })}

        {isSwiss && swissData?.standings?.length ? (
          <InfoCard title="STANDINGS">
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "12px" }}>
              <thead>
                <tr>{["#", "PLAYER", "PTS", "BUC", "SB", "ELO"].map(h => (
                  <th key={h} style={{ textAlign: "left", padding: "6px 10px", color: "var(--text-dim)", fontWeight: "800", fontSize: "10px", borderBottom: "1px solid var(--border)" }}>{h}</th>
                ))}</tr>
              </thead>
              <tbody>
                {swissData.standings.map(s => (
                  <tr key={s.player_id} style={{ borderBottom: "1px solid rgba(255,255,255,0.04)" }}>
                    <td style={{ padding: "8px 10px", color: s.rank <= 3 ? "var(--accent)" : "var(--text-dim)", fontWeight: "800" }}>{s.rank}</td>
                    <td style={{ padding: "8px 10px", fontFamily: "monospace", color: "#fff" }}>{shorten(s.player_id)}</td>
                    <td style={{ padding: "8px 10px", color: "var(--primary)", fontWeight: "800" }}>{s.score}</td>
                    <td style={{ padding: "8px 10px", color: "var(--text-dim)" }}>{s.buchholz.toFixed(1)}</td>
                    <td style={{ padding: "8px 10px", color: "var(--text-dim)" }}>{s.sonneborn.toFixed(1)}</td>
                    <td style={{ padding: "8px 10px", color: "var(--text-dim)" }}>{s.rating}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </InfoCard>
        ) : null}
      </div>
    );
  };

  if (loading) return <div style={{ textAlign: "center", padding: "4rem", color: "var(--text-dim)" }}>DECRYPTING DATA...</div>;
  if (!tournament) return <div style={{ textAlign: "center", padding: "4rem", color: "var(--text-dim)" }}>Tournament not found.</div>;

  return (
    <div style={{ width: "100%" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "2.5rem" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "1.5rem" }}>
          <button onClick={onBack} style={{ padding: "0.6rem 1.25rem", backgroundColor: "var(--glass)", color: "var(--text-dim)", border: "1px solid var(--border)", borderRadius: "100px", cursor: "pointer", fontSize: "12px", fontWeight: "700" }}>← RETURN</button>
          <h2 style={{ color: "#fff", margin: 0, fontSize: "28px", fontWeight: "900" }}>{tournament.name}</h2>
        </div>
        <button onClick={() => onEdit(tournament!.tournament_id)} className="primary" style={{ padding: "0.75rem 2rem", borderRadius: "100px" }}>MODIFY CONFIG</button>
      </div>

      <div style={{ marginBottom: "2rem" }}>
        <div style={{ display: "flex", gap: "8px", borderBottom: "1px solid var(--border)", paddingBottom: "1px" }}>
          {["overview", "players", "matches"].map(tab => (
            <button key={tab} onClick={() => setActiveTab(tab)}
              style={{ padding: "1rem 2rem", backgroundColor: "transparent", border: "none", borderBottom: activeTab === tab ? "3px solid var(--primary)" : "3px solid transparent", color: activeTab === tab ? "var(--primary)" : "var(--text-dim)", cursor: "pointer", fontSize: "11px", fontWeight: "800", letterSpacing: "1.5px" }}>
              {tab.toUpperCase()}
            </button>
          ))}
        </div>
      </div>

      <div style={{ animation: "fadeIn 0.4s ease" }}>
        {activeTab === "overview" && renderOverview()}
        {activeTab === "players" && renderPlayers()}
        {activeTab === "matches" && renderMatches()}
      </div>
    </div>
  );
}

const InfoCard = ({ title, children }: { title: string; children: React.ReactNode }) => (
  <div style={{ backgroundColor: "var(--surface)", padding: "2rem", borderRadius: "24px", border: "1px solid var(--border)", backdropFilter: "blur(20px)", boxShadow: "0 10px 40px rgba(0,0,0,0.3)" }}>
    <h4 style={{ color: "var(--primary)", fontSize: "11px", fontWeight: "800", letterSpacing: "2px", margin: "0 0 1.5rem 0" }}>{title}</h4>
    {children}
  </div>
);

const Row = ({ label, value, color }: { label: string; value: any; color?: string }) => (
  <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "12px", alignItems: "baseline" }}>
    <span style={{ color: "var(--text-dim)", fontSize: "11px", fontWeight: "700" }}>{label}</span>
    <span style={{ color: color || "#fff", fontSize: "14px", fontWeight: "800" }}>{value}</span>
  </div>
);

const StatusPill = ({ status }: { status: string }) => {
  const c = ({ Completed: { bg: "rgba(59,130,246,0.12)", text: "#60a5fa" }, Active: { bg: "rgba(74,222,128,0.12)", text: "#4ade80" } } as any)[status] ?? { bg: "rgba(255,255,255,0.06)", text: "var(--text-dim)" };
  return <span style={{ fontSize: "10px", fontWeight: "800", padding: "2px 8px", borderRadius: "4px", backgroundColor: c.bg, color: c.text }}>{status.toUpperCase()}</span>;
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
