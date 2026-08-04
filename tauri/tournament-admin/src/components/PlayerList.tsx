import { useState, useEffect } from "react";
import { apiClient } from "../services/api";

interface Player {
  wallet: string;
  username: string;
  kyc_status: string;
  elo: number;
  banned: boolean;
  ban_reason?: string;
}

interface PlayerDetailProps {
  wallet: string;
  onClose: () => void;
}

interface GameResultEntry { game_id: string; result: "win" | "loss" | "draw" | "in_progress"; stake_amount: number; ended_at: number | null; }

function PlayerDetail({ wallet, onClose }: PlayerDetailProps) {
  // No per-game ELO snapshot is recorded anywhere (on-chain PlayerProfile
  // only keeps the current rating) — this used to show a fabricated sparkline.
  // Real per-game outcomes are what's actually available.
  const [history, setHistory] = useState<GameResultEntry[]>([]);
  const [newElo, setNewElo] = useState("");
  const [eloReason, setEloReason] = useState("");
  const [banReason, setBanReason] = useState("");
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => {
    apiClient.getPlayerHistory(wallet).then(r => { if (r.ok && r.data) setHistory(r.data.history ?? []); });
  }, [wallet]);

  const handleEloOverride = async () => {
    const n = parseInt(newElo);
    if (isNaN(n) || !eloReason) return;
    const r = await apiClient.eloOverride(wallet, n, eloReason);
    setMsg(r.ok ? `ELO set to ${n}.` : `Error: ${r.error?.message}`);
  };

  const handleBan = async () => {
    if (!banReason) return;
    const r = await apiClient.banPlayer(wallet, banReason);
    setMsg(r.ok ? "Player banned." : `Error: ${r.error?.message}`);
  };


  const resultColor = { win: "#4ade80", loss: "#ef4444", draw: "var(--text-dim)", in_progress: "#3b82f6" } as const;

  return (
    <div style={{ position: "fixed", inset: 0, backgroundColor: "rgba(0,0,0,0.7)", zIndex: 100, display: "flex", alignItems: "center", justifyContent: "center" }} onClick={onClose}>
      <div style={{ backgroundColor: "var(--bg)", border: "1px solid var(--border)", borderRadius: "24px", padding: "2rem", width: "600px", maxHeight: "80vh", overflowY: "auto" }} onClick={e => e.stopPropagation()}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1.5rem" }}>
          <h3 style={{ color: "#fff", margin: 0, fontSize: "16px" }}>Player Detail</h3>
          <button onClick={onClose} style={{ background: "none", border: "none", color: "var(--text-dim)", cursor: "pointer", fontSize: "18px" }}>✕</button>
        </div>
        <div style={{ fontFamily: "monospace", fontSize: "12px", color: "var(--text-dim)", marginBottom: "1.5rem", wordBreak: "break-all" }}>{wallet}</div>

        {/* Recent game results (no ELO-per-game history is tracked anywhere) */}
        <div style={{ marginBottom: "1.5rem" }}>
          <div style={{ fontSize: "11px", color: "var(--text-dim)", letterSpacing: "1px", marginBottom: "8px" }}>RECENT RESULTS</div>
          {history.length === 0
            ? <div style={{ color: "var(--text-dim)", fontStyle: "italic", fontSize: "12px" }}>No game history.</div>
            : <div style={{ display: "flex", flexWrap: "wrap", gap: "6px" }}>
                {history.map((h, i) => (
                  <span key={i} title={`${h.result}${h.ended_at ? " — " + new Date(h.ended_at * 1000).toLocaleDateString() : ""}`}
                    style={{ fontSize: "10px", fontWeight: "800", padding: "3px 8px", borderRadius: "100px",
                      background: `${resultColor[h.result]}22`, color: resultColor[h.result] }}>
                    {h.result.toUpperCase()}
                  </span>
                ))}
              </div>
          }
        </div>

        {/* ELO override — display-only: this does not touch the real elo
            column, on-chain rating, or matchmaking/tournament elo gating. */}
        <div style={{ marginBottom: "1.5rem", padding: "1rem", background: "rgba(255,255,255,0.04)", borderRadius: "12px", border: "1px solid var(--border)" }}>
          <div style={{ fontSize: "11px", color: "var(--text-dim)", letterSpacing: "1px", marginBottom: "4px" }}>ADMIN-PANEL DISPLAY OVERRIDE (COSMETIC)</div>
          <div style={{ fontSize: "10px", color: "var(--text-dim)", marginBottom: "10px", fontStyle: "italic" }}>Only changes what this panel shows for this player — does not affect matchmaking, tournament ELO gating, or the on-chain rating.</div>
          <div style={{ display: "flex", gap: "8px", marginBottom: "6px" }}>
            <input value={newElo} onChange={e => setNewElo(e.target.value)} type="number" placeholder="New ELO…"
              style={{ flex: 1, background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "6px 10px", fontSize: "12px" }} />
            <input value={eloReason} onChange={e => setEloReason(e.target.value)} placeholder="Reason…"
              style={{ flex: 2, background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "6px 10px", fontSize: "12px" }} />
            <button onClick={handleEloOverride} style={{ padding: "6px 14px", borderRadius: "8px", backgroundColor: "var(--primary)", color: "#000", border: "none", fontWeight: "700", fontSize: "12px", cursor: "pointer" }}>SET</button>
          </div>
        </div>

        {/* Ban */}
        <div style={{ marginBottom: "1.5rem", padding: "1rem", background: "rgba(239,68,68,0.08)", borderRadius: "12px", border: "1px solid rgba(239,68,68,0.3)" }}>
          <div style={{ fontSize: "11px", color: "#f87171", letterSpacing: "1px", marginBottom: "10px" }}>BAN / SUSPEND</div>
          <div style={{ display: "flex", gap: "8px" }}>
            <input value={banReason} onChange={e => setBanReason(e.target.value)} placeholder="Ban reason…"
              style={{ flex: 1, background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "6px 10px", fontSize: "12px" }} />
            <button onClick={handleBan} style={{ padding: "6px 14px", borderRadius: "8px", backgroundColor: "#ef4444", color: "#fff", border: "none", fontWeight: "700", fontSize: "12px", cursor: "pointer" }}>BAN</button>
          </div>
        </div>

        {msg && <div style={{ marginTop: "12px", fontSize: "12px", color: msg.startsWith("Error") ? "#f87171" : "#4ade80" }}>{msg}</div>}
      </div>
    </div>
  );
}

export default function PlayerList() {
  const [allPlayers, setAllPlayers] = useState<Player[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [search, setSearch] = useState("");
  const [filterKyc, setFilterKyc] = useState<string>("all");
  const [filterBanned, setFilterBanned] = useState<string>("all");
  const [eloMin, setEloMin] = useState("");
  const [eloMax, setEloMax] = useState("");
  const [selectedWallet, setSelectedWallet] = useState<string | null>(null);

  useEffect(() => { loadPlayers(); }, []);

  const loadPlayers = async () => {
    try {
      setLoading(true);
      const r = await apiClient.getPlayers(200);
      if (r.ok) setAllPlayers(r.data.players ?? []);
      else setError("Failed to load players");
    } catch { setError("Network error"); }
    finally { setLoading(false); }
  };

  const filtered = allPlayers.filter(p => {
    if (search && !p.wallet.toLowerCase().includes(search.toLowerCase()) && !p.username?.toLowerCase().includes(search.toLowerCase())) return false;
    if (filterKyc !== "all" && p.kyc_status !== filterKyc) return false;
    if (filterBanned === "banned" && !p.banned) return false;
    if (filterBanned === "active" && p.banned) return false;
    if (eloMin && p.elo < parseInt(eloMin)) return false;
    if (eloMax && p.elo > parseInt(eloMax)) return false;
    return true;
  });

  const exportCsv = () => {
    const rows = [["wallet", "username", "elo", "kyc_status", "banned"].join(",")];
    filtered.forEach(p => rows.push([p.wallet, p.username || "", p.elo, p.kyc_status, String(p.banned)].join(",")));
    const blob = new Blob([rows.join("\n")], { type: "text/csv" });
    const a = document.createElement("a"); a.href = URL.createObjectURL(blob); a.download = "players.csv"; a.click();
  };

  return (
    <div style={{ padding: "1.5rem" }}>
      {selectedWallet && <PlayerDetail wallet={selectedWallet} onClose={() => setSelectedWallet(null)} />}

      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1.5rem" }}>
        <div>
          <h1 style={{ margin: 0, color: "white", fontSize: "1.5rem" }}>PLAYER <span style={{ color: "var(--primary)" }}>DIRECTORY</span></h1>
          <p style={{ color: "var(--text-dim)", margin: "0.25rem 0 0" }}>{filtered.length} of {allPlayers.length} players</p>
        </div>
        <div style={{ display: "flex", gap: "8px" }}>
          <button onClick={exportCsv} style={{ padding: "0.6rem 1.5rem", borderRadius: "100px", background: "rgba(255,255,255,0.06)", color: "var(--text-dim)", border: "1px solid var(--border)", fontSize: "12px", cursor: "pointer" }}>
            EXPORT CSV
          </button>
          <button onClick={loadPlayers} className="primary" style={{ padding: "0.6rem 1.5rem", borderRadius: "100px" }}>REFRESH</button>
        </div>
      </div>

      {/* Filter bar */}
      <div style={{ display: "flex", gap: "10px", marginBottom: "1.5rem", flexWrap: "wrap" }}>
        <input value={search} onChange={e => setSearch(e.target.value)} placeholder="Search wallet or username…"
          style={{ flex: "1 1 200px", background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 12px", fontSize: "12px" }} />
        <select value={filterKyc} onChange={e => setFilterKyc(e.target.value)}
          style={{ background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 12px", fontSize: "12px" }}>
          <option value="all">All KYC</option>
          <option value="verified">Verified</option>
          <option value="pending">Pending</option>
          <option value="none">None</option>
        </select>
        <select value={filterBanned} onChange={e => setFilterBanned(e.target.value)}
          style={{ background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 12px", fontSize: "12px" }}>
          <option value="all">All Players</option>
          <option value="active">Active only</option>
          <option value="banned">Banned only</option>
        </select>
        <input value={eloMin} onChange={e => setEloMin(e.target.value)} type="number" placeholder="ELO min"
          style={{ width: "90px", background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 10px", fontSize: "12px" }} />
        <input value={eloMax} onChange={e => setEloMax(e.target.value)} type="number" placeholder="ELO max"
          style={{ width: "90px", background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 10px", fontSize: "12px" }} />
      </div>

      {error && <div style={{ color: "#ef4444", marginBottom: "1rem" }}>{error}</div>}

      <div style={{ backgroundColor: "var(--surface)", borderRadius: "24px", border: "1px solid var(--border)", overflow: "hidden" }}>
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ textAlign: "left", backgroundColor: "rgba(255,255,255,0.02)", borderBottom: "1px solid var(--border)" }}>
              {["USERNAME", "WALLET", "ELO", "KYC", "STATUS", "ACTIONS"].map(h => (
                <th key={h} style={{ padding: "1rem", color: "var(--text-dim)", fontSize: "11px", letterSpacing: "1px" }}>{h}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr><td colSpan={6} style={{ padding: "3rem", textAlign: "center", color: "var(--text-dim)" }}>Loading…</td></tr>
            ) : filtered.length === 0 ? (
              <tr><td colSpan={6} style={{ padding: "3rem", textAlign: "center", color: "var(--text-dim)" }}>No players match.</td></tr>
            ) : (
              filtered.map(p => (
                <tr key={p.wallet} style={{ borderBottom: "1px solid rgba(255,255,255,0.02)" }}
                  onMouseEnter={e => e.currentTarget.style.background = "rgba(255,255,255,0.02)"}
                  onMouseLeave={e => e.currentTarget.style.background = "transparent"}>
                  <td style={{ padding: "0.9rem 1rem", color: "#fff", fontWeight: "bold" }}>{p.username || "—"}</td>
                  <td style={{ padding: "0.9rem 1rem", color: "var(--text-dim)", fontFamily: "monospace", fontSize: "12px" }}>{p.wallet.slice(0, 12)}…</td>
                  <td style={{ padding: "0.9rem 1rem", color: "var(--accent)", fontWeight: "700" }}>{p.elo}</td>
                  <td style={{ padding: "0.9rem 1rem" }}>
                    <span style={{ fontSize: "10px", padding: "2px 8px", borderRadius: "100px",
                      backgroundColor: p.kyc_status === "verified" ? "rgba(34,197,94,0.1)" : "rgba(234,179,8,0.1)",
                      color: p.kyc_status === "verified" ? "#22c55e" : "#eab308",
                      border: `1px solid ${p.kyc_status === "verified" ? "#22c55e44" : "#eab30844"}` }}>
                      {p.kyc_status?.toUpperCase() || "UNKNOWN"}
                    </span>
                  </td>
                  <td style={{ padding: "0.9rem 1rem" }}>
                    {p.banned
                      ? <span style={{ fontSize: "10px", padding: "2px 8px", borderRadius: "100px", backgroundColor: "rgba(239,68,68,0.15)", color: "#f87171", border: "1px solid rgba(239,68,68,0.3)" }}>BANNED</span>
                      : <span style={{ fontSize: "10px", padding: "2px 8px", borderRadius: "100px", backgroundColor: "rgba(74,222,128,0.1)", color: "#4ade80", border: "1px solid rgba(74,222,128,0.2)" }}>ACTIVE</span>
                    }
                  </td>
                  <td style={{ padding: "0.9rem 1rem" }}>
                    <button onClick={() => setSelectedWallet(p.wallet)}
                      style={{ backgroundColor: "transparent", border: "1px solid var(--border)", color: "#fff", padding: "4px 12px", borderRadius: "4px", fontSize: "11px", cursor: "pointer" }}>
                      DETAILS
                    </button>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
