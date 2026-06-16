import { useEffect, useState } from "react";
import { apiClient } from "../services/api";

const LAMPORTS_PER_SOL = 1_000_000_000;

type Puzzle = {
  id: string;
  name: string | null;
  fen: string;
  line: string;
  rating: number;
  rating_dev: number;
  themes: string;
  plays: number;
  nb_wins: number;
  featured: number;
  enabled: number;
};

type Bounty = {
  id: number;
  scope: string;
  puzzle_id: string | null;
  band_lo: number | null;
  band_hi: number | null;
  reward_lamports: number;
  budget_lamports: number;
  spent_lamports: number;
  max_per_wallet: number;
  status: string;
  created_at: number;
};

const card: React.CSSProperties = {
  backgroundColor: "rgba(10, 33, 26, 0.6)",
  border: "1px solid var(--border)",
  borderRadius: "16px",
  padding: "1rem 1.25rem",
  marginBottom: "1rem",
};
const input: React.CSSProperties = {
  backgroundColor: "#1a1a1a",
  border: "1px solid #404040",
  borderRadius: "8px",
  color: "#eee",
  padding: "0.4rem 0.6rem",
  fontSize: "13px",
};
const btn: React.CSSProperties = {
  backgroundColor: "var(--primary, #ad5c2f)",
  color: "#fff",
  border: "none",
  borderRadius: "8px",
  padding: "0.45rem 0.9rem",
  cursor: "pointer",
  fontSize: "13px",
};

export default function Puzzles() {
  const [eloMin, setEloMin] = useState(0);
  const [eloMax, setEloMax] = useState(3000);
  const [name, setName] = useState("");
  const [theme, setTheme] = useState("");
  const [rows, setRows] = useState<Puzzle[]>([]);
  const [total, setTotal] = useState(0);
  const [selected, setSelected] = useState<Puzzle | null>(null);
  const [bounties, setBounties] = useState<Bounty[]>([]);
  const [msg, setMsg] = useState("");

  // Funding form
  const [scope, setScope] = useState<"puzzle" | "band" | "daily">("puzzle");
  const [rewardSol, setRewardSol] = useState(0.01);
  const [budgetSol, setBudgetSol] = useState(0.1);
  const [bandLo, setBandLo] = useState(1400);
  const [bandHi, setBandHi] = useState(1600);
  const [maxPerWallet, setMaxPerWallet] = useState(1);

  const loadPuzzles = async () => {
    const res = await apiClient.listPuzzles({ eloMin, eloMax, name, theme, limit: 50 });
    if (res.ok && res.data) {
      setRows(res.data.puzzles || []);
      setTotal(res.data.total || 0);
    } else {
      setMsg(res.error?.message || "list failed");
    }
  };

  const loadBounties = async () => {
    const res = await apiClient.getPuzzleBounties();
    if (res.ok && res.data) setBounties(res.data.bounties || []);
  };

  useEffect(() => {
    loadPuzzles();
    loadBounties();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const fund = async () => {
    const body: any = {
      scope,
      reward_lamports: Math.round(rewardSol * LAMPORTS_PER_SOL),
      budget_lamports: Math.round(budgetSol * LAMPORTS_PER_SOL),
      max_per_wallet: maxPerWallet,
    };
    if (scope === "puzzle") {
      if (!selected) { setMsg("select a puzzle first"); return; }
      body.puzzle_id = selected.id;
    } else if (scope === "band") {
      body.band_lo = bandLo;
      body.band_hi = bandHi;
    }
    const res = await apiClient.fundPuzzle(body);
    if (res.ok) {
      setMsg(`funded bounty #${res.data?.bounty_id}`);
      loadBounties();
    } else {
      setMsg(res.error?.message || "fund failed");
    }
  };

  const sol = (l: number) => (l / LAMPORTS_PER_SOL).toFixed(4);

  return (
    <div>
      {msg && (
        <div style={{ ...card, color: "#facc15" }}>{msg}</div>
      )}

      {/* Browser */}
      <div style={card}>
        <h3 style={{ color: "#ad5c2f", marginTop: 0 }}>Puzzle Browser ({total})</h3>
        <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", marginBottom: "0.75rem" }}>
          <input style={input} type="number" value={eloMin} onChange={(e) => setEloMin(+e.target.value)} placeholder="ELO min" />
          <input style={input} type="number" value={eloMax} onChange={(e) => setEloMax(+e.target.value)} placeholder="ELO max" />
          <input style={input} value={name} onChange={(e) => setName(e.target.value)} placeholder="name / id" />
          <input style={input} value={theme} onChange={(e) => setTheme(e.target.value)} placeholder="theme" />
          <button style={btn} onClick={loadPuzzles}>Search</button>
        </div>
        <div style={{ maxHeight: "320px", overflow: "auto" }}>
          <table style={{ width: "100%", fontSize: "12px", color: "#ddd", borderCollapse: "collapse" }}>
            <thead>
              <tr style={{ textAlign: "left", color: "#888" }}>
                <th>ID</th><th>Name</th><th>Rating</th><th>Themes</th><th>Plays</th><th>Win%</th><th></th>
              </tr>
            </thead>
            <tbody>
              {rows.map((p) => (
                <tr key={p.id} style={{ borderTop: "1px solid #2a2a2a", background: selected?.id === p.id ? "rgba(173,92,47,0.15)" : "transparent" }}>
                  <td>{p.id}</td>
                  <td>{p.name || "—"}</td>
                  <td>{p.rating}</td>
                  <td style={{ maxWidth: 220, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p.themes}</td>
                  <td>{p.plays}</td>
                  <td>{p.plays ? Math.round((100 * p.nb_wins) / p.plays) : 0}%</td>
                  <td><button style={{ ...btn, padding: "0.2rem 0.5rem" }} onClick={() => setSelected(p)}>Inspect</button></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {/* Inspector */}
      {selected && (
        <div style={card}>
          <h3 style={{ color: "#ad5c2f", marginTop: 0 }}>Inspector — {selected.id}</h3>
          <div style={{ fontSize: "12px", color: "#bbb" }}>
            <div><b>FEN:</b> <code>{selected.fen}</code></div>
            <div><b>Solution (line):</b> <code>{selected.line}</code></div>
            <div><b>Rating:</b> {selected.rating} ±{selected.rating_dev}</div>
            <div><b>Themes:</b> {selected.themes}</div>
          </div>
          <div style={{ display: "flex", gap: "0.5rem", marginTop: "0.75rem", flexWrap: "wrap" }}>
            <button style={btn} onClick={async () => {
              const n = prompt("Name this puzzle:", selected.name || "");
              if (n != null) { await apiClient.namePuzzle(selected.id, n); loadPuzzles(); }
            }}>Set name</button>
            <button style={btn} onClick={async () => { await apiClient.featurePuzzle(selected.id, !selected.featured); loadPuzzles(); }}>
              {selected.featured ? "Unfeature" : "Feature"}
            </button>
            <button style={{ ...btn, backgroundColor: selected.enabled ? "#ef4444" : "#22c55e" }} onClick={async () => { await apiClient.enablePuzzle(selected.id, !selected.enabled); loadPuzzles(); }}>
              {selected.enabled ? "Disable" : "Enable"}
            </button>
          </div>
        </div>
      )}

      {/* Funding */}
      <div style={card}>
        <h3 style={{ color: "#ad5c2f", marginTop: 0 }}>Fund a Bounty</h3>
        <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", alignItems: "center" }}>
          <select style={input} value={scope} onChange={(e) => setScope(e.target.value as any)}>
            <option value="puzzle">This puzzle{selected ? ` (${selected.id})` : ""}</option>
            <option value="band">ELO band</option>
            <option value="daily">Daily</option>
          </select>
          {scope === "band" && (
            <>
              <input style={input} type="number" value={bandLo} onChange={(e) => setBandLo(+e.target.value)} placeholder="band lo" />
              <input style={input} type="number" value={bandHi} onChange={(e) => setBandHi(+e.target.value)} placeholder="band hi" />
            </>
          )}
          <label style={{ color: "#bbb", fontSize: 12 }}>reward (SOL)
            <input style={{ ...input, marginLeft: 6, width: 90 }} type="number" step="0.001" value={rewardSol} onChange={(e) => setRewardSol(+e.target.value)} />
          </label>
          <label style={{ color: "#bbb", fontSize: 12 }}>budget (SOL)
            <input style={{ ...input, marginLeft: 6, width: 90 }} type="number" step="0.01" value={budgetSol} onChange={(e) => setBudgetSol(+e.target.value)} />
          </label>
          <label style={{ color: "#bbb", fontSize: 12 }}>max/wallet
            <input style={{ ...input, marginLeft: 6, width: 60 }} type="number" value={maxPerWallet} onChange={(e) => setMaxPerWallet(+e.target.value)} />
          </label>
          <button style={{ ...btn, backgroundColor: "#22c55e" }} onClick={fund}>BUILD &amp; FUND</button>
        </div>
        <p style={{ color: "#777", fontSize: 11, marginBottom: 0 }}>
          Funds are locked against the VPS authority budget; a server-verified solve pays the reward.
        </p>
      </div>

      {/* Bounties burn-down */}
      <div style={card}>
        <h3 style={{ color: "#ad5c2f", marginTop: 0 }}>Active Bounties</h3>
        <table style={{ width: "100%", fontSize: "12px", color: "#ddd", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ textAlign: "left", color: "#888" }}>
              <th>#</th><th>Scope</th><th>Target</th><th>Reward</th><th>Spent / Budget</th><th>Status</th><th></th>
            </tr>
          </thead>
          <tbody>
            {bounties.map((b) => (
              <tr key={b.id} style={{ borderTop: "1px solid #2a2a2a" }}>
                <td>{b.id}</td>
                <td>{b.scope}</td>
                <td>{b.scope === "puzzle" ? b.puzzle_id : b.scope === "band" ? `${b.band_lo}–${b.band_hi}` : "daily"}</td>
                <td>{sol(b.reward_lamports)} ◎</td>
                <td>{sol(b.spent_lamports)} / {sol(b.budget_lamports)} ◎</td>
                <td>{b.status}</td>
                <td>
                  {b.status === "active" && (
                    <button style={{ ...btn, padding: "0.2rem 0.5rem", backgroundColor: "#ef4444" }}
                      onClick={async () => { await apiClient.closePuzzleBounty(b.id); loadBounties(); }}>
                      Close
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
