import { useState, useEffect, useRef } from "react";
import { apiClient } from "../services/api";

interface EvalPoint { move_number: number; eval_cp: number; best_move?: string; played_move?: string; deviation?: number; }

export default function GameExplorer() {
  const [searchQuery, setSearchQuery] = useState("");
  const [searchType, setSearchType] = useState<"username" | "wallet">("username");
  const [games, setGames] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [selectedGame, setSelectedGame] = useState<any | null>(null);
  const [moves, setMoves] = useState<any[]>([]);
  const [scrubberIdx, setScrubberIdx] = useState(0);
  const [archiveStats, setArchiveStats] = useState<any>(null);
  const [evalData, setEvalData] = useState<EvalPoint[]>([]);
  const [evalLoading, setEvalLoading] = useState(false);
  const [actionMsg, setActionMsg] = useState<string | null>(null);
  const evalRef = useRef<SVGSVGElement>(null);

  useEffect(() => { loadArchiveStats(); }, []);

  const loadArchiveStats = async () => {
    const r = await apiClient.getArchiveStats();
    if (r.ok) setArchiveStats(r.data);
  };

  const handleSearch = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!searchQuery.trim()) return;
    setLoading(true); setError(""); setGames([]); setSelectedGame(null); setMoves([]);
    try {
      const r = searchType === "username"
        ? await apiClient.getGameHistoryByUsername(searchQuery)
        : await apiClient.getGameHistory(searchQuery);
      if (r.ok) setGames(r.data.games || []);
      else setError(r.error?.message || "Search failed");
    } catch { setError("Network error"); }
    finally { setLoading(false); }
  };

  const selectGame = async (game: any) => {
    setSelectedGame(game); setMoves([]); setScrubberIdx(0); setEvalData([]); setActionMsg(null);
    setLoading(true);
    try {
      const r = await apiClient.getGameMoves(game.id);
      if (r.ok) {
        const m = r.data.moves || [];
        setMoves(m);
        setScrubberIdx(m.length);
      }
    } catch { setError("Failed to fetch moves"); }
    finally { setLoading(false); }
  };

  const loadEval = async () => {
    if (!selectedGame) return;
    setEvalLoading(true);
    const r = await apiClient.getGameEval(selectedGame.id);
    if (r.ok) setEvalData(r.data.evals || []);
    setEvalLoading(false);
  };

  const handleFlag = async () => {
    if (!selectedGame) return;
    const r = await apiClient.flagGame(selectedGame.id, "flagged from explorer");
    setActionMsg(r.ok ? "Game flagged for review." : `Error: ${r.error?.message}`);
  };

  const handleForceResign = async (color: "white" | "black") => {
    if (!selectedGame) return;
    const r = await apiClient.forceResign(selectedGame.id, color);
    setActionMsg(r.ok ? `Force resign sent (${color}).` : `Error: ${r.error?.message}`);
  };

  const copySpectateLink = () => {
    if (!selectedGame) return;
    navigator.clipboard.writeText(`xfchess://spectate/${selectedGame.id}`).catch(() => {});
    setActionMsg("Spectate link copied.");
    setTimeout(() => setActionMsg(null), 2000);
  };

  const currentFen = scrubberIdx === 0
    ? "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    : moves[scrubberIdx - 1]?.fen ?? moves[scrubberIdx - 1]?.position ?? "—";

  const evalMax = evalData.length ? Math.max(...evalData.map(e => Math.abs(e.eval_cp ?? 0)), 500) : 500;

  const inputS: React.CSSProperties = {
    background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff",
    borderRadius: "8px", padding: "8px 12px", fontSize: "12px",
  };

  return (
    <div style={{ padding: "1.5rem", display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div>
          <h1 style={{ margin: 0, color: "#fff", fontSize: "1.5rem" }}>GAME <span style={{ color: "var(--primary)" }}>EXPLORER</span></h1>
          <p style={{ color: "var(--text-dim)", margin: "0.25rem 0 0" }}>Search, replay, and moderate games</p>
        </div>
        {archiveStats && (
          <div style={{ display: "flex", gap: "1.5rem", background: "rgba(0,0,0,0.2)", padding: "0.75rem 1.25rem", borderRadius: "16px", border: "1px solid var(--border)", alignItems: "center" }}>
            <div>
              <div style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: "700", letterSpacing: "1px" }}>ARCHIVE</div>
              <div style={{ color: "var(--accent)", fontWeight: "900", fontSize: "14px" }}>{(archiveStats.games_archive_size_bytes / 1024).toFixed(1)} KB</div>
            </div>
            <div>
              <div style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: "700", letterSpacing: "1px" }}>WALLETS</div>
              <div style={{ color: "#fff", fontWeight: "900", fontSize: "14px" }}>{archiveStats.unique_wallets_count}</div>
            </div>
            <div style={{ display: "flex", gap: "6px" }}>
              <button onClick={() => window.open(apiClient.getArchiveDownloadUrl("games"), "_blank")}
                style={{ ...inputS, padding: "5px 12px", cursor: "pointer", background: "var(--primary)", color: "#000", fontWeight: "700", border: "none" }}>
                DL GAMES
              </button>
              <button onClick={() => window.open(apiClient.getArchiveDownloadUrl("wallets"), "_blank")}
                style={{ ...inputS, padding: "5px 12px", cursor: "pointer" }}>
                DL WALLETS
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Search */}
      <form onSubmit={handleSearch} style={{ display: "flex", gap: "10px" }}>
        <select value={searchType} onChange={e => setSearchType(e.target.value as any)} style={{ ...inputS, width: "140px" }}>
          <option value="username">USERNAME</option>
          <option value="wallet">WALLET</option>
        </select>
        <input value={searchQuery} onChange={e => setSearchQuery(e.target.value)}
          placeholder={searchType === "username" ? "Search by username…" : "Search by wallet address…"}
          style={{ ...inputS, flex: 1 }} />
        <button type="submit" className="primary" style={{ padding: "8px 24px", borderRadius: "100px" }} disabled={loading}>
          {loading ? "…" : "SEARCH"}
        </button>
      </form>

      {error && <div style={{ color: "#ef4444", background: "rgba(239,68,68,0.1)", padding: "0.75rem 1rem", borderRadius: "10px", fontSize: "13px" }}>{error}</div>}

      <div style={{ display: "grid", gridTemplateColumns: selectedGame ? "320px 1fr" : "1fr", gap: "1.5rem" }}>
        {/* Game list */}
        <div style={{ backgroundColor: "var(--surface)", borderRadius: "24px", border: "1px solid var(--border)", overflow: "hidden" }}>
          <div style={{ padding: "1rem 1.25rem", borderBottom: "1px solid var(--border)", fontSize: "11px", fontWeight: "800", letterSpacing: "1.5px", color: "var(--primary)" }}>
            RESULTS ({games.length})
          </div>
          <div style={{ maxHeight: "600px", overflowY: "auto" }}>
            {games.length === 0 && !loading && (
              <div style={{ padding: "3rem", textAlign: "center", color: "var(--text-dim)", fontStyle: "italic" }}>No games found.</div>
            )}
            {games.map(g => (
              <div key={g.id} onClick={() => selectGame(g)}
                style={{
                  padding: "1rem 1.25rem", cursor: "pointer", borderBottom: "1px solid rgba(255,255,255,0.03)",
                  background: selectedGame?.id === g.id ? "rgba(255,255,255,0.05)" : "transparent",
                  borderLeft: selectedGame?.id === g.id ? "3px solid var(--primary)" : "3px solid transparent",
                }}
                onMouseEnter={e => { if (selectedGame?.id !== g.id) e.currentTarget.style.background = "rgba(255,255,255,0.02)"; }}
                onMouseLeave={e => { if (selectedGame?.id !== g.id) e.currentTarget.style.background = "transparent"; }}>
                <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "4px" }}>
                  <span style={{ color: "#fff", fontWeight: "700", fontSize: "13px" }}>#{String(g.id).slice(0, 8)}</span>
                  <span style={{ fontSize: "10px", padding: "1px 7px", borderRadius: "100px",
                    background: g.status === "completed" ? "rgba(34,197,94,0.1)" : "rgba(234,179,8,0.1)",
                    color: g.status === "completed" ? "#22c55e" : "#eab308" }}>
                    {g.status?.toUpperCase()}
                  </span>
                </div>
                <div style={{ fontSize: "12px", color: "var(--text-dim)" }}>
                  {g.white_username || "?"} vs {g.black_username || "?"}
                </div>
                <div style={{ fontSize: "11px", color: "var(--text-dim)", marginTop: "3px", display: "flex", justifyContent: "space-between" }}>
                  <span>{g.start_time ? new Date(g.start_time * 1000).toLocaleDateString() : "—"}</span>
                  {g.stake_amount > 0 && <span style={{ color: "var(--accent)" }}>{g.stake_amount} SOL</span>}
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Game detail panel */}
        {selectedGame && (
          <div style={{ display: "flex", flexDirection: "column", gap: "1.25rem" }}>
            {/* Game info + actions */}
            <div style={{ backgroundColor: "var(--surface)", borderRadius: "24px", border: "1px solid var(--border)", padding: "1.25rem 1.5rem" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: "1rem" }}>
                <div>
                  <div style={{ fontSize: "11px", color: "var(--text-dim)", letterSpacing: "1px", marginBottom: "4px" }}>GAME ID</div>
                  <div style={{ fontFamily: "monospace", color: "#fff", fontSize: "13px" }}>{selectedGame.id}</div>
                </div>
                <div style={{ display: "flex", gap: "8px" }}>
                  <button onClick={copySpectateLink}
                    style={{ ...inputS, padding: "6px 14px", cursor: "pointer", fontWeight: "700", fontSize: "11px" }}>
                    SPECTATE LINK
                  </button>
                  <button onClick={handleFlag}
                    style={{ ...inputS, padding: "6px 14px", cursor: "pointer", fontWeight: "700", fontSize: "11px", borderColor: "#f59e0b", color: "#f59e0b" }}>
                    FLAG
                  </button>
                  <button onClick={() => handleForceResign("white")}
                    style={{ ...inputS, padding: "6px 14px", cursor: "pointer", fontWeight: "700", fontSize: "11px", borderColor: "#ef4444", color: "#ef4444" }}>
                    RESIGN WHITE
                  </button>
                  <button onClick={() => handleForceResign("black")}
                    style={{ ...inputS, padding: "6px 14px", cursor: "pointer", fontWeight: "700", fontSize: "11px", borderColor: "#ef4444", color: "#ef4444" }}>
                    RESIGN BLACK
                  </button>
                </div>
              </div>
              {actionMsg && <div style={{ fontSize: "12px", color: actionMsg.startsWith("Error") ? "#f87171" : "#4ade80", marginBottom: "8px" }}>{actionMsg}</div>}
              <div style={{ display: "flex", gap: "2rem", fontSize: "12px" }}>
                <div><span style={{ color: "var(--text-dim)" }}>White: </span><span style={{ color: "#fff" }}>{selectedGame.white_username || "—"}</span></div>
                <div><span style={{ color: "var(--text-dim)" }}>Black: </span><span style={{ color: "#fff" }}>{selectedGame.black_username || "—"}</span></div>
                <div><span style={{ color: "var(--text-dim)" }}>Status: </span><span style={{ color: "#4ade80" }}>{selectedGame.status}</span></div>
                {selectedGame.stake_amount > 0 && <div><span style={{ color: "var(--text-dim)" }}>Stake: </span><span style={{ color: "var(--accent)" }}>{selectedGame.stake_amount} SOL</span></div>}
              </div>
            </div>

            {/* Position scrubber */}
            <div style={{ backgroundColor: "var(--surface)", borderRadius: "24px", border: "1px solid var(--border)", padding: "1.25rem 1.5rem" }}>
              <div style={{ fontSize: "11px", color: "var(--primary)", fontWeight: "800", letterSpacing: "1.5px", marginBottom: "1rem" }}>POSITION REPLAY</div>
              {moves.length === 0
                ? <div style={{ color: "var(--text-dim)", fontStyle: "italic", fontSize: "12px" }}>No moves recorded.</div>
                : <>
                  <div style={{ display: "flex", gap: "12px", alignItems: "center", marginBottom: "10px" }}>
                    <button onClick={() => setScrubberIdx(0)} style={{ ...inputS, padding: "4px 10px", cursor: "pointer", fontSize: "11px" }}>|◀</button>
                    <button onClick={() => setScrubberIdx(i => Math.max(0, i - 1))} style={{ ...inputS, padding: "4px 10px", cursor: "pointer", fontSize: "11px" }}>◀</button>
                    <input type="range" min={0} max={moves.length} value={scrubberIdx}
                      onChange={e => setScrubberIdx(Number(e.target.value))}
                      style={{ flex: 1, accentColor: "var(--primary)" }} />
                    <button onClick={() => setScrubberIdx(i => Math.min(moves.length, i + 1))} style={{ ...inputS, padding: "4px 10px", cursor: "pointer", fontSize: "11px" }}>▶</button>
                    <button onClick={() => setScrubberIdx(moves.length)} style={{ ...inputS, padding: "4px 10px", cursor: "pointer", fontSize: "11px" }}>▶|</button>
                    <span style={{ color: "var(--text-dim)", fontSize: "12px", minWidth: "60px" }}>Move {scrubberIdx}/{moves.length}</span>
                  </div>
                  {scrubberIdx > 0 && (
                    <div style={{ fontFamily: "monospace", fontSize: "12px", color: "var(--accent)", background: "rgba(0,0,0,0.3)", padding: "8px 12px", borderRadius: "8px", border: "1px solid var(--border)" }}>
                      {moves[scrubberIdx - 1]?.move_uci ?? "—"}
                      {currentFen !== "—" && <span style={{ color: "var(--text-dim)", marginLeft: "16px", fontSize: "11px" }}>{currentFen}</span>}
                    </div>
                  )}
                  {/* Move list */}
                  <div style={{ display: "flex", flexWrap: "wrap", gap: "4px", marginTop: "10px", maxHeight: "80px", overflowY: "auto" }}>
                    {moves.map((m, i) => (
                      <span key={i} onClick={() => setScrubberIdx(i + 1)}
                        style={{ padding: "2px 8px", borderRadius: "6px", fontSize: "11px", fontFamily: "monospace", cursor: "pointer",
                          background: scrubberIdx === i + 1 ? "var(--primary)" : "rgba(255,255,255,0.06)",
                          color: scrubberIdx === i + 1 ? "#000" : "var(--text-dim)",
                          border: "1px solid var(--border)" }}>
                        {i % 2 === 0 ? `${Math.floor(i / 2) + 1}.` : ""}{m.move_uci}
                      </span>
                    ))}
                  </div>
                </>
              }
            </div>

            {/* Eval graph */}
            <div style={{ backgroundColor: "var(--surface)", borderRadius: "24px", border: "1px solid var(--border)", padding: "1.25rem 1.5rem" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1rem" }}>
                <div style={{ fontSize: "11px", color: "var(--primary)", fontWeight: "800", letterSpacing: "1.5px" }}>EVAL DEVIATION GRAPH</div>
                <button onClick={loadEval} style={{ ...inputS, padding: "5px 14px", cursor: "pointer", fontSize: "11px", fontWeight: "700" }} disabled={evalLoading}>
                  {evalLoading ? "LOADING…" : "LOAD EVAL"}
                </button>
              </div>
              {evalData.length === 0
                ? <div style={{ color: "var(--text-dim)", fontStyle: "italic", fontSize: "12px" }}>Click LOAD EVAL to analyze this game.</div>
                : (() => {
                  const W = 600; const H = 100;
                  const pts = evalData.map((e, i) => {
                    const x = (i / (evalData.length - 1)) * W;
                    const y = H / 2 - Math.max(-evalMax, Math.min(evalMax, e.eval_cp ?? 0)) / evalMax * (H / 2 - 4);
                    return `${x},${y}`;
                  }).join(" ");
                  const devPts = evalData.map((e, i) => {
                    const x = (i / (evalData.length - 1)) * W;
                    const d = Math.min(evalMax, Math.abs(e.deviation ?? 0));
                    return { x, y: H - d / evalMax * (H / 2 - 4), d: e.deviation ?? 0 };
                  });
                  return (
                    <div>
                      <svg ref={evalRef} viewBox={`0 0 ${W} ${H}`} width="100%" height={H} style={{ display: "block", borderRadius: "8px", background: "rgba(0,0,0,0.3)" }}>
                        <line x1="0" y1={H / 2} x2={W} y2={H / 2} stroke="rgba(255,255,255,0.1)" strokeWidth="1" />
                        <polyline points={pts} fill="none" stroke="var(--primary)" strokeWidth="1.5" />
                        {devPts.map((p, i) => p.d > 50 && (
                          <circle key={i} cx={p.x} cy={p.y} r="3" fill={p.d > 150 ? "#ef4444" : "#f59e0b"} opacity="0.8" />
                        ))}
                      </svg>
                      <div style={{ display: "flex", gap: "16px", marginTop: "8px", fontSize: "11px", color: "var(--text-dim)" }}>
                        <span><span style={{ color: "#ef4444" }}>●</span> Blunder (&gt;150cp)</span>
                        <span><span style={{ color: "#f59e0b" }}>●</span> Mistake (50-150cp)</span>
                        <span>Highest dev: {Math.max(...evalData.map(e => Math.abs(e.deviation ?? 0))).toFixed(0)} cp</span>
                      </div>
                    </div>
                  );
                })()
              }
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
