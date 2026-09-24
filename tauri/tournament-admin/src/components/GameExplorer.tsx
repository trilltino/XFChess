import { useEffect, useMemo, useState } from "react";
import { apiClient } from "../services/api";

type TxDebugState = {
  signature: string;
  loading: boolean;
  data?: any;
  error?: string;
};

const startFen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

function short(value?: string | null) {
  if (!value) return "-";
  return value.length > 18 ? `${value.slice(0, 8)}...${value.slice(-6)}` : value;
}

function when(ts?: number | null) {
  if (!ts) return "-";
  return new Date(ts * 1000).toLocaleString();
}

export default function GameExplorer() {
  const [searchQuery, setSearchQuery] = useState("");
  const [games, setGames] = useState<any[]>([]);
  const [bundle, setBundle] = useState<any | null>(null);
  const [capabilities, setCapabilities] = useState<any | null>(null);
  const [archiveStats, setArchiveStats] = useState<any>(null);
  const [scrubberIdx, setScrubberIdx] = useState(0);
  const [loading, setLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [error, setError] = useState("");
  const [actionMsg, setActionMsg] = useState<string | null>(null);
  const [txDebug, setTxDebug] = useState<TxDebugState | null>(null);

  useEffect(() => {
    apiClient.getDebugCapabilities().then((r) => r.ok && setCapabilities(r.data));
    apiClient.getArchiveStats().then((r) => r.ok && setArchiveStats(r.data));
  }, []);

  const selectedGame = bundle?.game ?? null;
  const moves = bundle?.moves ?? [];
  const currentFen =
    scrubberIdx === 0
      ? startFen
      : moves[scrubberIdx - 1]?.fen_after ?? moves[scrubberIdx - 1]?.fen ?? "-";

  const eventRows = useMemo(() => {
    const moveEvents = bundle?.events?.moves?.entries ?? [];
    const chatEvents = bundle?.events?.chat?.entries ?? [];
    return [
      ...moveEvents.map((e: any) => ({ ...e, stream: "moves" })),
      ...chatEvents.map((e: any) => ({ ...e, stream: "chat" })),
    ].sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0));
  }, [bundle]);

  const inputS: React.CSSProperties = {
    background: "rgba(255,255,255,0.06)",
    border: "1px solid var(--border)",
    color: "#fff",
    borderRadius: "8px",
    padding: "8px 12px",
    fontSize: "12px",
  };

  const panelS: React.CSSProperties = {
    backgroundColor: "var(--surface)",
    borderRadius: "18px",
    border: "1px solid var(--border)",
    padding: "1.1rem 1.25rem",
  };

  const handleSearch = async (e?: React.FormEvent) => {
    e?.preventDefault();
    const q = searchQuery.trim();
    if (!q) return;
    setLoading(true);
    setError("");
    setBundle(null);
    setGames([]);
    try {
      const r = await apiClient.searchDebugGames(q);
      if (r.ok) setGames(r.data.games || []);
      else setError(r.error?.message || "Search failed");
    } catch {
      setError("Network error");
    } finally {
      setLoading(false);
    }
  };

  const selectGame = async (game: any) => {
    setDetailLoading(true);
    setError("");
    setActionMsg(null);
    setTxDebug(null);
    setScrubberIdx(0);
    try {
      const r = await apiClient.getGameDebugBundle(game.id);
      if (r.ok) {
        setBundle(r.data);
        setScrubberIdx((r.data.moves || []).length);
      } else {
        setError(r.error?.message || "Failed to load debug bundle");
      }
    } catch {
      setError("Failed to load debug bundle");
    } finally {
      setDetailLoading(false);
    }
  };

  const handleFlag = async () => {
    if (!selectedGame) return;
    const r = await apiClient.flagGame(Number(selectedGame.id), "flagged from debug cockpit");
    setActionMsg(r.ok ? "Game flagged for review." : `Error: ${r.error?.message}`);
  };

  const handleForceResign = async (color: "white" | "black") => {
    if (!selectedGame) return;
    const r = await apiClient.forceResign(Number(selectedGame.id), color);
    setActionMsg(r.ok ? `Force resign sent (${color}).` : `Error: ${r.error?.message}`);
  };

  const copyPgn = () => {
    if (!bundle?.pgn) return;
    navigator.clipboard.writeText(bundle.pgn).catch(() => {});
    setActionMsg("PGN copied.");
  };

  const analyzeTx = async (signature: string) => {
    setTxDebug({ signature, loading: true });
    const r = await apiClient.debugTransaction(signature);
    setTxDebug(
      r.ok
        ? { signature, loading: false, data: r.data }
        : { signature, loading: false, error: r.error?.message || "Analysis failed" }
    );
  };

  const refreshBundle = async () => {
    if (!bundle?.game_id) return;
    const r = await apiClient.getGameDebugBundle(bundle.game_id);
    if (r.ok) setBundle(r.data);
  };

  const retryMoneyAction = async (id: string) => {
    setActionMsg("Retrying money-action reconciliation...");
    const r = await apiClient.retryMoneyAction(id);
    setActionMsg(r.ok ? "Reconciliation retried." : `Error: ${r.error?.message}`);
    await refreshBundle();
  };

  const markMoneyActionReview = async (id: string) => {
    setActionMsg("Marking money action for admin review...");
    const r = await apiClient.markMoneyActionReview(id);
    setActionMsg(r.ok ? "Money action marked for review." : `Error: ${r.error?.message}`);
    await refreshBundle();
  };

  return (
    <div style={{ padding: "1.5rem", display: "flex", flexDirection: "column", gap: "1.25rem" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "1rem" }}>
        <div>
          <h1 style={{ margin: 0, color: "#fff", fontSize: "1.5rem" }}>
            GAME <span style={{ color: "var(--primary)" }}>DEBUG COCKPIT</span>
          </h1>
          <p style={{ color: "var(--text-dim)", margin: "0.25rem 0 0" }}>
            Search by game, wallet, username, tournament, or transaction signature
          </p>
        </div>
        {archiveStats && (
          <div style={{ ...panelS, padding: "0.75rem 1rem", display: "flex", gap: "1rem" }}>
            <div>
              <div style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: 800 }}>ARCHIVE</div>
              <div style={{ color: "var(--accent)", fontWeight: 900 }}>{(archiveStats.games_archive_size_bytes / 1024).toFixed(1)} KB</div>
            </div>
            <div>
              <div style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: 800 }}>WALLETS</div>
              <div style={{ color: "#fff", fontWeight: 900 }}>{archiveStats.unique_wallets_count}</div>
            </div>
          </div>
        )}
      </div>

      <form onSubmit={handleSearch} style={{ display: "flex", gap: "10px" }}>
        <input
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="Game ID, wallet, username, tournament ID, or tx signature"
          style={{ ...inputS, flex: 1 }}
        />
        <button type="submit" className="primary" style={{ padding: "8px 24px", borderRadius: "100px" }} disabled={loading}>
          {loading ? "SEARCHING" : "SEARCH"}
        </button>
      </form>

      {capabilities?.sources && (
        <div style={{ display: "flex", flexWrap: "wrap", gap: "6px" }}>
          {Object.entries(capabilities.sources).map(([name, on]) => (
            <span key={name} style={{
              fontSize: "10px",
              padding: "3px 8px",
              borderRadius: "100px",
              border: "1px solid var(--border)",
              color: on ? "#4ade80" : "var(--text-dim)",
              background: on ? "rgba(74,222,128,0.08)" : "rgba(255,255,255,0.03)",
            }}>
              {name.replace(/_/g, " ").toUpperCase()}
            </span>
          ))}
        </div>
      )}

      {error && <div style={{ color: "#f87171", background: "rgba(239,68,68,0.1)", padding: "0.75rem 1rem", borderRadius: "10px", fontSize: "13px" }}>{error}</div>}

      <div style={{ display: "grid", gridTemplateColumns: bundle ? "320px 1fr" : "1fr", gap: "1.25rem" }}>
        <div style={{ ...panelS, padding: 0, overflow: "hidden" }}>
          <div style={{ padding: "1rem 1.25rem", borderBottom: "1px solid var(--border)", fontSize: "11px", fontWeight: 800, color: "var(--primary)" }}>
            RESULTS ({games.length})
          </div>
          <div style={{ maxHeight: "680px", overflowY: "auto" }}>
            {games.length === 0 && !loading && (
              <div style={{ padding: "3rem", textAlign: "center", color: "var(--text-dim)", fontStyle: "italic" }}>No games loaded.</div>
            )}
            {games.map((g) => (
              <button
                key={g.id}
                onClick={() => selectGame(g)}
                style={{
                  display: "block",
                  width: "100%",
                  textAlign: "left",
                  padding: "1rem 1.25rem",
                  cursor: "pointer",
                  border: 0,
                  borderBottom: "1px solid rgba(255,255,255,0.04)",
                  background: selectedGame?.id === g.id ? "rgba(255,255,255,0.06)" : "transparent",
                  color: "#fff",
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", gap: "8px" }}>
                  <span style={{ fontWeight: 800, fontSize: "13px", fontFamily: "monospace" }}>#{String(g.id).slice(0, 10)}</span>
                  <span style={{ color: g.status === "completed" ? "#4ade80" : "#facc15", fontSize: "10px", fontWeight: 800 }}>{String(g.status || "unknown").toUpperCase()}</span>
                </div>
                <div style={{ color: "var(--text-dim)", fontSize: "12px", marginTop: "5px" }}>
                  {g.white_username || short(g.player_white)} vs {g.black_username || short(g.player_black)}
                </div>
                <div style={{ color: "var(--text-dim)", fontSize: "11px", marginTop: "4px" }}>{when(g.start_time)}</div>
              </button>
            ))}
          </div>
        </div>

        {bundle && (
          <div style={{ display: "flex", flexDirection: "column", gap: "1rem", opacity: detailLoading ? 0.65 : 1 }}>
            <div style={panelS}>
              <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", alignItems: "flex-start" }}>
                <div>
                  <div style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: 800 }}>GAME ID</div>
                  <div style={{ color: "#fff", fontFamily: "monospace", fontSize: "13px" }}>{bundle.game_id}</div>
                </div>
                <div style={{ display: "flex", gap: "8px", flexWrap: "wrap", justifyContent: "flex-end" }}>
                  <button onClick={handleFlag} style={{ ...inputS, color: "#f59e0b", borderColor: "#f59e0b", cursor: "pointer" }}>FLAG</button>
                  <button onClick={() => handleForceResign("white")} style={{ ...inputS, color: "#f87171", borderColor: "#ef4444", cursor: "pointer" }}>RESIGN WHITE</button>
                  <button onClick={() => handleForceResign("black")} style={{ ...inputS, color: "#f87171", borderColor: "#ef4444", cursor: "pointer" }}>RESIGN BLACK</button>
                </div>
              </div>
              {actionMsg && <div style={{ marginTop: "8px", color: actionMsg.startsWith("Error") ? "#f87171" : "#4ade80", fontSize: "12px" }}>{actionMsg}</div>}
              <div style={{ display: "grid", gridTemplateColumns: "repeat(4, minmax(0, 1fr))", gap: "1rem", marginTop: "1rem", fontSize: "12px" }}>
                <div><span style={{ color: "var(--text-dim)" }}>White </span><span style={{ color: "#fff" }}>{selectedGame.white_username || short(selectedGame.player_white)}</span></div>
                <div><span style={{ color: "var(--text-dim)" }}>Black </span><span style={{ color: "#fff" }}>{selectedGame.black_username || short(selectedGame.player_black)}</span></div>
                <div><span style={{ color: "var(--text-dim)" }}>Status </span><span style={{ color: "#4ade80" }}>{selectedGame.status}</span></div>
                <div><span style={{ color: "var(--text-dim)" }}>Stake </span><span style={{ color: "var(--accent)" }}>{selectedGame.stake_amount || 0} SOL</span></div>
              </div>
              <div style={{ display: "flex", gap: "6px", flexWrap: "wrap", marginTop: "1rem" }}>
                {(bundle.available_sections || []).map((s: string) => <span key={s} style={{ fontSize: "10px", color: "#4ade80", background: "rgba(74,222,128,0.08)", borderRadius: "100px", padding: "3px 8px" }}>{s.toUpperCase()}</span>)}
                {(bundle.missing_sections || []).map((s: string) => <span key={s} style={{ fontSize: "10px", color: "var(--text-dim)", background: "rgba(255,255,255,0.04)", borderRadius: "100px", padding: "3px 8px" }}>{s.toUpperCase()} MISSING</span>)}
              </div>
            </div>

            <div style={panelS}>
              <div style={{ fontSize: "11px", color: "var(--primary)", fontWeight: 800, marginBottom: "0.8rem" }}>POSITION REPLAY</div>
              {moves.length === 0 ? (
                <div style={{ color: "var(--text-dim)", fontStyle: "italic", fontSize: "12px" }}>No moves recorded.</div>
              ) : (
                <>
                  <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
                    <button onClick={() => setScrubberIdx(0)} style={{ ...inputS, cursor: "pointer" }}>|&lt;</button>
                    <button onClick={() => setScrubberIdx((i) => Math.max(0, i - 1))} style={{ ...inputS, cursor: "pointer" }}>&lt;</button>
                    <input type="range" min={0} max={moves.length} value={scrubberIdx} onChange={(e) => setScrubberIdx(Number(e.target.value))} style={{ flex: 1, accentColor: "var(--primary)" }} />
                    <button onClick={() => setScrubberIdx((i) => Math.min(moves.length, i + 1))} style={{ ...inputS, cursor: "pointer" }}>&gt;</button>
                    <button onClick={() => setScrubberIdx(moves.length)} style={{ ...inputS, cursor: "pointer" }}>&gt;|</button>
                    <span style={{ color: "var(--text-dim)", fontSize: "12px", minWidth: "72px" }}>Move {scrubberIdx}/{moves.length}</span>
                  </div>
                  <div style={{ marginTop: "10px", fontFamily: "monospace", fontSize: "12px", color: "var(--accent)", background: "rgba(0,0,0,0.28)", padding: "8px 10px", borderRadius: "8px", border: "1px solid var(--border)" }}>
                    {scrubberIdx > 0 ? moves[scrubberIdx - 1]?.move_uci : "Start"} <span style={{ color: "var(--text-dim)", marginLeft: "12px" }}>{currentFen}</span>
                  </div>
                </>
              )}
            </div>

            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "1rem" }}>
              <div style={panelS}>
                <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "0.7rem" }}>
                  <div style={{ fontSize: "11px", color: "var(--primary)", fontWeight: 800 }}>PGN</div>
                  {bundle.pgn && <button onClick={copyPgn} style={{ ...inputS, padding: "4px 10px", cursor: "pointer" }}>COPY</button>}
                </div>
                <pre style={{ whiteSpace: "pre-wrap", maxHeight: "220px", overflow: "auto", color: "#fff", fontSize: "11px", margin: 0 }}>{bundle.pgn || "No PGN available."}</pre>
              </div>

              <div style={panelS}>
                <div style={{ fontSize: "11px", color: "var(--primary)", fontWeight: 800, marginBottom: "0.7rem" }}>EVENT TIMELINE</div>
                {eventRows.length === 0 ? (
                  <div style={{ color: "var(--text-dim)", fontStyle: "italic", fontSize: "12px" }}>No Braid event log entries.</div>
                ) : (
                  <div style={{ maxHeight: "220px", overflow: "auto", display: "grid", gap: "6px" }}>
                    {eventRows.map((e: any, i: number) => (
                      <div key={`${e.stream}-${e.seq}-${i}`} style={{ fontSize: "11px", color: "#fff", borderBottom: "1px solid rgba(255,255,255,0.05)", paddingBottom: "6px" }}>
                        <span style={{ color: "var(--accent)", fontWeight: 800 }}>{e.stream}</span> #{e.seq} {e.kind || ""} <span style={{ color: "var(--text-dim)" }}>{when(e.created_at)}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>

            <div style={panelS}>
              <div style={{ fontSize: "11px", color: "var(--primary)", fontWeight: 800, marginBottom: "0.7rem" }}>RAW MOVES</div>
              <div style={{ overflow: "auto", maxHeight: "260px" }}>
                <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "11px", color: "#fff" }}>
                  <tbody>
                    {moves.map((m: any) => (
                      <tr key={m.id ?? m.move_number} style={{ borderBottom: "1px solid rgba(255,255,255,0.05)" }}>
                        <td style={{ padding: "6px", color: "var(--text-dim)" }}>{m.move_number}</td>
                        <td style={{ padding: "6px", fontFamily: "monospace" }}>{m.move_uci}</td>
                        <td style={{ padding: "6px" }}>{m.move_san || "-"}</td>
                        <td style={{ padding: "6px", color: "var(--text-dim)" }}>{short(m.player)}</td>
                        <td style={{ padding: "6px", color: "var(--text-dim)" }}>{when(m.timestamp)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>

            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "1rem" }}>
              <div style={panelS}>
                <div style={{ fontSize: "11px", color: "var(--primary)", fontWeight: 800, marginBottom: "0.7rem" }}>RELATED TRANSACTIONS</div>
                {(bundle.transactions || []).length === 0 ? (
                  <div style={{ color: "var(--text-dim)", fontStyle: "italic", fontSize: "12px" }}>No linked signatures are stored for this game.</div>
                ) : (
                  <div style={{ display: "grid", gap: "8px" }}>
                    {bundle.transactions.map((tx: any) => (
                      <div key={tx.signature} style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: "8px", alignItems: "center", borderBottom: "1px solid rgba(255,255,255,0.05)", paddingBottom: "8px" }}>
                        <div>
                          <div style={{ color: "#fff", fontFamily: "monospace", fontSize: "11px" }}>{short(tx.signature)}</div>
                          <div style={{ color: "var(--text-dim)", fontSize: "10px" }}>{tx.operation} / {tx.source} / {tx.status}</div>
                        </div>
                        <button onClick={() => analyzeTx(tx.signature)} style={{ ...inputS, padding: "5px 12px", cursor: "pointer" }}>ANALYZE</button>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              <div style={panelS}>
                <div style={{ fontSize: "11px", color: "var(--primary)", fontWeight: 800, marginBottom: "0.7rem" }}>TRANSACTION ANALYSIS</div>
                {!txDebug ? (
                  <div style={{ color: "var(--text-dim)", fontStyle: "italic", fontSize: "12px" }}>Choose Analyze on a linked signature.</div>
                ) : txDebug.loading ? (
                  <div style={{ color: "var(--text-dim)", fontSize: "12px" }}>Analyzing {short(txDebug.signature)} through the backend RPC...</div>
                ) : txDebug.error ? (
                  <div style={{ color: "#f87171", fontSize: "12px" }}>{txDebug.error}</div>
                ) : (
                  <div style={{ display: "grid", gap: "8px", fontSize: "12px" }}>
                    <div><span style={{ color: "var(--text-dim)" }}>Status </span><span style={{ color: txDebug.data?.status?.ok ? "#4ade80" : "#f87171" }}>{txDebug.data?.status?.ok ? "OK" : "FAILED"}</span></div>
                    <div><span style={{ color: "var(--text-dim)" }}>Slot </span><span style={{ color: "#fff" }}>{txDebug.data?.metadata?.slot ?? "-"}</span></div>
                    <div><span style={{ color: "var(--text-dim)" }}>Root Cause </span><span style={{ color: "#fff" }}>{txDebug.data?.root_cause?.category ?? "unclassified"}</span></div>
                    <pre style={{ whiteSpace: "pre-wrap", maxHeight: "220px", overflow: "auto", color: "var(--text-dim)", background: "rgba(0,0,0,0.25)", padding: "8px", borderRadius: "8px", margin: 0 }}>{JSON.stringify(txDebug.data, null, 2)}</pre>
                  </div>
                )}
              </div>
            </div>

            <div style={panelS}>
              <div style={{ fontSize: "11px", color: "var(--primary)", fontWeight: 800, marginBottom: "0.7rem" }}>MONEY ACTIONS</div>
              {(bundle.money_actions || []).length === 0 ? (
                <div style={{ color: "var(--text-dim)", fontStyle: "italic", fontSize: "12px" }}>No tracked money actions are linked to this game yet.</div>
              ) : (
                <div style={{ overflow: "auto", maxHeight: "280px" }}>
                  <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "11px", color: "#fff" }}>
                    <thead>
                      <tr style={{ color: "var(--text-dim)", textAlign: "left" }}>
                        <th style={{ padding: "6px" }}>Action</th>
                        <th style={{ padding: "6px" }}>Status</th>
                        <th style={{ padding: "6px" }}>Wallet</th>
                        <th style={{ padding: "6px" }}>Signature</th>
                        <th style={{ padding: "6px" }}>Attempts</th>
                        <th style={{ padding: "6px" }}>Reason</th>
                        <th style={{ padding: "6px" }}></th>
                      </tr>
                    </thead>
                    <tbody>
                      {bundle.money_actions.map((ma: any) => (
                        <tr key={ma.id} style={{ borderTop: "1px solid rgba(255,255,255,0.05)" }}>
                          <td style={{ padding: "6px", fontWeight: 800 }}>{ma.action_type}</td>
                          <td style={{ padding: "6px", color: ma.status === "resolved" ? "#4ade80" : ma.status === "failed" || ma.status === "needs_admin_review" ? "#f87171" : "#facc15" }}>{ma.status}</td>
                          <td style={{ padding: "6px", fontFamily: "monospace", color: "var(--text-dim)" }}>{short(ma.wallet)}</td>
                          <td style={{ padding: "6px", fontFamily: "monospace" }}>{short(ma.signature)}</td>
                          <td style={{ padding: "6px" }}>{ma.attempt_count ?? 0}</td>
                          <td style={{ padding: "6px", color: "var(--text-dim)" }}>{ma.last_error || ma.reason || "-"}</td>
                          <td style={{ padding: "6px", display: "flex", gap: "6px", justifyContent: "flex-end" }}>
                            {ma.signature && <button onClick={() => analyzeTx(ma.signature)} style={{ ...inputS, padding: "4px 9px", cursor: "pointer" }}>ANALYZE</button>}
                            <button onClick={() => retryMoneyAction(ma.id)} style={{ ...inputS, padding: "4px 9px", cursor: "pointer" }}>RETRY</button>
                            <button onClick={() => markMoneyActionReview(ma.id)} style={{ ...inputS, padding: "4px 9px", cursor: "pointer", color: "#f59e0b" }}>REVIEW</button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>

            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: "1rem" }}>
              <div style={panelS}>
                <div style={{ fontSize: "11px", color: "var(--primary)", fontWeight: 800, marginBottom: "0.7rem" }}>TOURNAMENT</div>
                <pre style={{ whiteSpace: "pre-wrap", maxHeight: "220px", overflow: "auto", color: "var(--text-dim)", fontSize: "11px", margin: 0 }}>{bundle.tournament ? JSON.stringify(bundle.tournament, null, 2) : "No linked tournament context."}</pre>
              </div>
              <div style={panelS}>
                <div style={{ fontSize: "11px", color: "var(--primary)", fontWeight: 800, marginBottom: "0.7rem" }}>ANTI-CHEAT</div>
                <pre style={{ whiteSpace: "pre-wrap", maxHeight: "220px", overflow: "auto", color: "var(--text-dim)", fontSize: "11px", margin: 0 }}>{JSON.stringify(bundle.anti_cheat, null, 2)}</pre>
              </div>
              <div style={panelS}>
                <div style={{ fontSize: "11px", color: "var(--primary)", fontWeight: 800, marginBottom: "0.7rem" }}>MODERATION</div>
                <pre style={{ whiteSpace: "pre-wrap", maxHeight: "220px", overflow: "auto", color: "var(--text-dim)", fontSize: "11px", margin: 0 }}>{JSON.stringify(bundle.moderation, null, 2)}</pre>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
