import { useState, useEffect } from "react";
import { apiClient } from "../services/api";

interface Payout { game_id: string; winner: string; amount_sol: number; tx_sig: string; settled_at: number; }
interface FeeReport { total_fee_sol: number; total_fee_lamports: number; total_wagered_sol: number; game_count: number; period: string; }

export default function Treasury() {
  const [payouts, setPayouts] = useState<Payout[]>([]);
  const [feeReport, setFeeReport] = useState<FeeReport | null>(null);
  const [period, setPeriod] = useState("week");
  const [loading, setLoading] = useState(true);

  // Manual refund form
  const [refundWallet, setRefundWallet] = useState("");
  const [refundLamports, setRefundLamports] = useState("");
  const [refundReason, setRefundReason] = useState("");
  const [refundAdminToken, setRefundAdminToken] = useState("");
  const [refundMsg, setRefundMsg] = useState<string | null>(null);
  const [refundTx, setRefundTx] = useState<string | null>(null);

  useEffect(() => { loadData(); }, [period]);

  const loadData = async () => {
    setLoading(true);
    const [pr, fr] = await Promise.all([apiClient.getTreasuryPayouts(), apiClient.getFeeReport(period)]);
    if (pr.ok) setPayouts(pr.data.payouts ?? []);
    if (fr.ok) setFeeReport(fr.data);
    setLoading(false);
  };

  const handleRefund = async () => {
    const lam = parseInt(refundLamports);
    if (!refundWallet || isNaN(lam) || !refundReason || !refundAdminToken) return;
    const r = await apiClient.manualRefund(refundWallet, lam, refundReason, refundAdminToken);
    if (r.ok) {
      // Backend now signs + submits withdraw_treasury with treasury_authority and
      // returns the confirmed on-chain signature (no more client-side signing).
      setRefundMsg(`Refund submitted on-chain. Sig: ${r.data?.signature ?? "—"}`);
      setRefundTx(r.data?.signature ?? null);
      setRefundWallet(""); setRefundLamports(""); setRefundReason(""); setRefundAdminToken("");
    } else {
      setRefundMsg(`Error: ${r.error?.message}`);
    }
  };

  return (
    <div style={{ padding: "1.5rem", display: "flex", flexDirection: "column", gap: "2rem" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div>
          <h1 style={{ margin: 0, color: "#fff", fontSize: "1.5rem" }}>TREASURY <span style={{ color: "var(--primary)" }}>MANAGEMENT</span></h1>
          <p style={{ color: "var(--text-dim)", margin: "0.25rem 0 0" }}>Prize payouts, fee revenue, and manual refunds</p>
        </div>
        <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
          <select value={period} onChange={e => setPeriod(e.target.value)}
            style={{ background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 12px", fontSize: "12px" }}>
            <option value="day">Today</option>
            <option value="week">This week</option>
            <option value="month">This month</option>
            <option value="all">All time</option>
          </select>
          <button onClick={loadData} className="primary" style={{ padding: "0.6rem 1.5rem", borderRadius: "100px" }}>REFRESH</button>
        </div>
      </div>

      {/* Fee report summary */}
      {feeReport && (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: "1.25rem" }}>
          <StatCard label="FEES COLLECTED" value={`${feeReport.total_fee_sol.toFixed(4)} SOL`} color="var(--accent)" />
          <StatCard label="TOTAL WAGERED" value={`${feeReport.total_wagered_sol.toFixed(2)} SOL`} color="var(--primary)" />
          <StatCard label="GAMES WITH WAGER" value={feeReport.game_count} color="#3b82f6" />
        </div>
      )}

      {/* Prize distribution log */}
      <div style={{ backgroundColor: "var(--surface)", borderRadius: "24px", border: "1px solid var(--border)", overflow: "hidden" }}>
        <div style={{ padding: "1.25rem 1.5rem", backgroundColor: "rgba(255,255,255,0.05)", borderBottom: "1px solid var(--border)", fontSize: "12px", fontWeight: "800", letterSpacing: "1.5px", color: "var(--primary)" }}>
          PRIZE DISTRIBUTION LOG
        </div>
        {loading
          ? <div style={{ padding: "3rem", textAlign: "center", color: "var(--text-dim)" }}>Loading…</div>
          : payouts.length === 0
          ? <div style={{ padding: "3rem", textAlign: "center", color: "var(--text-dim)", fontStyle: "italic" }}>No wagered games in this period.</div>
          : <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "12px" }}>
              <thead>
                <tr style={{ backgroundColor: "rgba(255,255,255,0.02)", borderBottom: "1px solid var(--border)" }}>
                  {["GAME", "WINNER", "AMOUNT", "TX SIG", "SETTLED AT"].map(h => (
                    <th key={h} style={{ padding: "0.75rem 1rem", textAlign: "left", color: "var(--text-dim)", fontSize: "10px", letterSpacing: "1px" }}>{h}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {payouts.map((p, i) => (
                  <tr key={i} style={{ borderBottom: "1px solid rgba(255,255,255,0.03)" }}>
                    <td style={{ padding: "0.75rem 1rem", fontFamily: "monospace", color: "var(--text-dim)" }}>{p.game_id}</td>
                    <td style={{ padding: "0.75rem 1rem", fontFamily: "monospace", color: "#fff" }}>{p.winner ? `${String(p.winner).slice(0, 10)}…` : "—"}</td>
                    <td style={{ padding: "0.75rem 1rem", color: "#4ade80", fontWeight: "700" }}>{p.amount_sol?.toFixed(4)} SOL</td>
                    <td style={{ padding: "0.75rem 1rem", fontFamily: "monospace", color: "var(--text-dim)", fontSize: "11px" }}>{p.tx_sig && p.tx_sig !== "—" ? `${p.tx_sig.slice(0, 16)}…` : "—"}</td>
                    <td style={{ padding: "0.75rem 1rem", color: "var(--text-dim)" }}>{p.settled_at ? new Date(p.settled_at * 1000).toLocaleString() : "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
        }
      </div>

      {/* Manual refund */}
      <div style={{ backgroundColor: "var(--surface)", borderRadius: "24px", border: "1px solid var(--border)", padding: "1.5rem" }}>
        <h3 style={{ color: "var(--primary)", fontSize: "12px", fontWeight: "800", letterSpacing: "2px", margin: "0 0 1.25rem" }}>MANUAL REFUND</h3>
        <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: "0 0 1rem" }}>
          Signs + submits <code>withdraw_treasury</code> with treasury_authority and returns the
          on-chain signature. Requires the ADMIN_TOKEN second factor (money path).
        </p>
        <div style={{ display: "flex", gap: "10px", marginBottom: "10px", flexWrap: "wrap" }}>
          <input value={refundWallet} onChange={e => setRefundWallet(e.target.value)} placeholder="Recipient wallet…"
            style={{ flex: 2, minWidth: "180px", background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 12px", fontSize: "12px" }} />
          <input value={refundLamports} onChange={e => setRefundLamports(e.target.value)} type="number" placeholder="Lamports…"
            style={{ flex: 1, minWidth: "120px", background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 12px", fontSize: "12px" }} />
          <input value={refundReason} onChange={e => setRefundReason(e.target.value)} placeholder="Reason…"
            style={{ flex: 2, minWidth: "160px", background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 12px", fontSize: "12px" }} />
          <input value={refundAdminToken} onChange={e => setRefundAdminToken(e.target.value)} type="password" placeholder="ADMIN_TOKEN (2nd factor)…"
            style={{ flex: 2, minWidth: "180px", background: "rgba(255,255,255,0.06)", border: "1px solid #f59e0b55", color: "#fff", borderRadius: "8px", padding: "8px 12px", fontSize: "12px" }} />
          <button onClick={handleRefund} style={{ padding: "8px 20px", borderRadius: "8px", backgroundColor: "var(--primary)", color: "#000", border: "none", fontWeight: "700", fontSize: "12px", cursor: "pointer" }}>SUBMIT REFUND</button>
        </div>
        {refundMsg && <div style={{ fontSize: "12px", color: refundMsg.startsWith("Error") ? "#f87171" : "#4ade80" }}>{refundMsg}</div>}
        {refundTx && (
          <div style={{ marginTop: "12px" }}>
            <div style={{ fontSize: "10px", color: "var(--text-dim)", letterSpacing: "1px", marginBottom: "6px" }}>ON-CHAIN SIGNATURE</div>
            <div style={{ background: "rgba(0,0,0,0.4)", padding: "10px 12px", borderRadius: "8px", fontFamily: "monospace", fontSize: "11px", color: "var(--primary)", wordBreak: "break-all", border: "1px solid var(--border)", marginBottom: "8px" }}>
              {refundTx}
            </div>
            <button onClick={() => navigator.clipboard.writeText(refundTx).catch(() => {})}
              style={{ padding: "6px 14px", borderRadius: "8px", background: "rgba(255,255,255,0.08)", color: "var(--text-dim)", border: "1px solid var(--border)", fontSize: "11px", cursor: "pointer" }}>
              COPY SIG
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function StatCard({ label, value, color }: { label: string; value: string | number; color: string }) {
  return (
    <div style={{ backgroundColor: "var(--surface)", padding: "1.5rem", borderRadius: "24px", border: "1px solid var(--border)", backdropFilter: "blur(10px)" }}>
      <div style={{ fontSize: "10px", color: "var(--text-dim)", letterSpacing: "2px", fontWeight: "700", marginBottom: "8px" }}>{label}</div>
      <div style={{ fontSize: "24px", fontWeight: "800", color: "#fff" }}>{value}</div>
      <div style={{ width: "32px", height: "4px", backgroundColor: color, borderRadius: "100px", marginTop: "8px" }} />
    </div>
  );
}
