import { useState, useEffect } from "react";
import { apiClient } from "../services/api";
import { useAuth } from "../hooks/useAuth";
import { VPS_HOST } from "../config/environments";

interface WalletData { pubkey: string; balance_lamports: number; balance_sol: string; }
interface Report { game_id: number; white: string; black: string; suspect: string; verdict: string; wager: string; score: number; reason: string; status: string; created_at?: number; assigned_to?: string; }
interface NodeCheck { label: string; url: string; status: "checking" | "online" | "offline"; }
interface DeployEntry { ts: string; note: string; code: number; }

const DEPLOY_HISTORY_KEY = "deploy_history";
function loadDeployHistory(): DeployEntry[] {
  try { return JSON.parse(localStorage.getItem(DEPLOY_HISTORY_KEY) || "[]"); } catch { return []; }
}

const FEEPAYER_THRESHOLD_KEY = "feepayer_threshold_sol";
function getFeepayerThreshold() { return parseFloat(localStorage.getItem(FEEPAYER_THRESHOLD_KEY) || "0.5"); }

export default function Dashboard() {
  type MainTab = "CONSOLE" | "MODERATION" | "AUDIT" | "INFRA";
  const [activeTab, setActiveTab] = useState<MainTab>("CONSOLE");
  const { authState } = useAuth();
  const backendUrl = authState.backend_url;
  // Prometheus (:9090) is internal-only on the VPS and not forwarded by the
  // SSH tunnel, so it's only ever reached directly — never through
  // backend_url. In PRODUCTION that means the VPS IP (best-effort: only
  // populates when :9090 is directly reachable from this machine). In LOCAL
  // it must stay on loopback — pointing a local session at the VPS IP made
  // every 5s poll hang for the OS's full TCP connect timeout instead of
  // failing fast, since an unreachable-but-not-actively-refusing remote host
  // doesn't reject the way a closed local port does.
  const promHost = authState.env === "production" ? VPS_HOST : "127.0.0.1";

  const [activeSessions, setActiveSessions] = useState(0);
  const [totalGames, setTotalGames] = useState(0);
  const [txConfirmed, setTxConfirmed] = useState(0);
  const [cpuUsage, setCpuUsage] = useState(0);
  const [ramUsage, setRamUsage] = useState(0);
  const [rates, setRates] = useState<Record<string, number>>({});
  type WalletKey = "feepayer" | "vps_signer" | "kyc_signer" | "treasury";
  const WALLET_LABELS: Record<WalletKey, string> = {
    feepayer: "FEEPAYER",
    vps_signer: "VPS AUTHORITY",
    kyc_signer: "KYC AUTHORITY",
    treasury: "TREASURY AUTHORITY",
  };
  const [wallets, setWallets] = useState<Record<WalletKey, WalletData> | undefined>();
  const [reports, setReports] = useState<Report[]>([]);

  const [logs, setLogs] = useState<string[]>([]);
  const [deploying] = useState(false);
  const [deployHistory] = useState<DeployEntry[]>(loadDeployHistory);

  const [taskStatus, setTaskStatus] = useState<Record<string, { last_tick: number; status: string }>>({});
  const [dbStats, setDbStats] = useState<{ sessions_rows: number; games_rows: number; users_rows: number; db_mb: number } | null>(null);
  const [tlsExpiry, setTlsExpiry] = useState<{ domain: string; days_remaining: number | null; status: string }[]>([]);
  const [nodeChecks, setNodeChecks] = useState<NodeCheck[]>([
    { label: "Backend API", url: `${backendUrl}/health`, status: "checking" },
  ]);

  const [ipBanInput, setIpBanInput] = useState({ ip: "", reason: "" });
  const [ipBanMsg, setIpBanMsg] = useState<string | null>(null);
  const [assigningDispute, setAssigningDispute] = useState<number | null>(null);
  const [assignedMsg, setAssignedMsg] = useState<Record<number, string>>({});
  const [disputeAdminToken, setDisputeAdminToken] = useState("");
  const [resolutionNotes, setResolutionNotes] = useState<Record<number, string>>({});
  const [resolvingDispute, setResolvingDispute] = useState<number | null>(null);
  const [resolvedMsg, setResolvedMsg] = useState<Record<number, string>>({});
  const [auditEntries, setAuditEntries] = useState<{ timestamp: number; actor: string; action: string; target: string; result: string }[]>([]);

  const feepayerThreshold = getFeepayerThreshold();
  const addLog = (msg: string) => setLogs(prev => [...prev.slice(-199), `[${new Date().toLocaleTimeString()}] ${msg}`]);

  useEffect(() => {
    const poll = async () => {
      try {
        const metrics = await fetch(`${backendUrl}/metrics`).then(r => r.text()).catch(() => "");
        setActiveSessions(parseInt(metrics.match(/active_sessions (\d+)/)?.[1] || "0"));
        setTotalGames(parseInt(metrics.match(/games_created_total (\d+)/)?.[1] || "0"));
        setTxConfirmed(parseInt(metrics.match(/transactions_confirmed_total\{chain="solana"\} (\d+)/)?.[1] || "0"));

        const promQuery = async (q: string) => {
          try {
            // Bound the wait ourselves — an unreachable-but-not-refusing
            // remote host (e.g. production Prometheus with no route from
            // this machine) otherwise hangs for the OS's full TCP connect
            // timeout, stalling this 5s poll loop for far longer than that.
            const controller = new AbortController();
            const timeout = setTimeout(() => controller.abort(), 3000);
            const r = await fetch(
              `http://${promHost}:9090/api/v1/query?query=${encodeURIComponent(q)}`,
              { signal: controller.signal },
            ).then(r => r.json());
            clearTimeout(timeout);
            return r.data.result[0]?.value[1] || "0";
          } catch { return "0"; }
        };
        const cpu = parseFloat(await promQuery('100-(avg(rate(node_cpu_seconds_total{mode="idle"}[1m]))*100)'));
        const ramUsed = parseFloat(await promQuery('node_memory_MemTotal_bytes-node_memory_MemAvailable_bytes'));
        const ramTotal = parseFloat(await promQuery('node_memory_MemTotal_bytes'));
        setCpuUsage(Math.round(cpu || 0));
        setRamUsage(Math.round((ramUsed / (ramTotal || 1)) * 100 || 0));

        const wb = await apiClient.getWalletBalances(); if (wb.ok) setWallets(wb.data);
        const rr = await apiClient.getExchangeRates();  if (rr.ok) setRates(rr.data.rates);
        const ar = await apiClient.getAntiCheatReports(); if (ar.ok && ar.data) setReports(ar.data.reports);
      } catch {}
    };
    poll();
    const id = setInterval(poll, 5000);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    const pollInfra = async () => {
      const ts = await apiClient.getTasksStatus(); if (ts.ok) setTaskStatus(ts.data);
      const ds = await apiClient.getDbStats();     if (ds.ok) setDbStats(ds.data);
      const tl = await apiClient.getTlsExpiry();   if (tl.ok) setTlsExpiry(Array.isArray(tl.data) ? tl.data : [tl.data]);
    };
    pollInfra();
    const id = setInterval(pollInfra, 30000);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    if (activeTab !== "AUDIT") return;
    const fetch_ = async () => {
      const r = await apiClient.getAuditLog(100); if (r.ok) setAuditEntries(r.data.entries ?? []);
    };
    fetch_(); const id = setInterval(fetch_, 10000); return () => clearInterval(id);
  }, [activeTab]);

  useEffect(() => {
    if (activeTab !== "CONSOLE") return;
    const fetch_ = async () => {
      const r = await apiClient.getLogsStream();
      if (r.ok && r.data?.lines) {
        setLogs(prev => { const nl = (r.data.lines as string[]).filter(l => !prev.includes(l)); return [...prev, ...nl].slice(-200); });
      }
    };
    fetch_(); const id = setInterval(fetch_, 5000); return () => clearInterval(id);
  }, [activeTab]);

  useEffect(() => {
    const check = async () => {
      const updated = await Promise.all(
        nodeChecks.map(async n => {
          try {
            const r = await fetch(n.url, { signal: AbortSignal.timeout(3000) });
            return { ...n, status: r.ok ? "online" as const : "offline" as const };
          } catch { return { ...n, status: "offline" as const }; }
        })
      );
      setNodeChecks(updated);
    };
    check(); const id = setInterval(check, 30000); return () => clearInterval(id);
  }, []);

  // In-app deployment is not implemented (T9, tauri/docs/TAURI_REMEDIATION.md).
  // Deploys run from a terminal via ops\scripts\deploy.ps1.
  const runDeployment = async () => {
    addLog("NOT IMPLEMENTED: deploys run from a terminal, not this panel.");
    addLog("Run: powershell -File ops\\scripts\\deploy.ps1 -Server 178.104.55.19 [-Domain your.domain]");
  };

  const handleIpBan = async () => {
    if (!ipBanInput.ip || !ipBanInput.reason) return;
    const r = await apiClient.ipBan(ipBanInput.ip, ipBanInput.reason);
    setIpBanMsg(r.ok ? `Recorded ${ipBanInput.ip}.` : `Error: ${r.error?.message}`);
    if (r.ok) setIpBanInput({ ip: "", reason: "" });
  };

  const handleAssignDispute = async (gameId: number) => {
    setAssigningDispute(gameId);
    const r = await apiClient.assignDispute(gameId, "admin");
    setAssignedMsg(prev => ({ ...prev, [gameId]: r.ok ? "Assigned to you." : `Error: ${r.error?.message}` }));
    setAssigningDispute(null);
    const ar = await apiClient.getAntiCheatReports(); if (ar.ok && ar.data) setReports(ar.data.reports);
  };

  // "refund" = the suspect's flag stands and the (non-suspect) victim is
  // awarded the win; "winner" = false positive, the original result is
  // upheld and the dispute is dismissed with no on-chain outcome change.
  const resolveDispute = async (report: Report, action: "refund" | "winner") => {
    const note = resolutionNotes[report.game_id]?.trim();
    if (!disputeAdminToken || !note) {
      setResolvedMsg(prev => ({ ...prev, [report.game_id]: "Error: admin token and a resolution note are both required." }));
      return;
    }
    const victimIsWhite = report.suspect === report.black;
    const decision = action === "winner" ? "DISMISS" : (victimIsWhite ? "WHITE_WINS" : "BLACK_WINS");
    setResolvingDispute(report.game_id);
    const r = await apiClient.resolveDispute(report.game_id, decision, note, report.white, report.black, disputeAdminToken);
    if (r.ok && r.data) {
      const txSig = r.data.tx_sig;
      addLog(`ResolveDispute game ${report.game_id} decision=${decision} tx=${txSig}`);
      setResolvedMsg(prev => ({ ...prev, [report.game_id]: `Resolved (${decision}). Tx: ${txSig}` }));
      const ar = await apiClient.getAntiCheatReports(); if (ar.ok && ar.data) setReports(ar.data.reports);
    } else {
      setResolvedMsg(prev => ({ ...prev, [report.game_id]: `Error: ${r.error?.message}` }));
    }
    setResolvingDispute(null);
  };

  const now = Date.now() / 1000;
  const fpBalance = parseFloat(wallets?.feepayer?.balance_sol?.replace(" SOL", "") ?? "999");
  const fpLow = fpBalance < feepayerThreshold;
  const sym = (code: string) => ({ usd: "$", gbp: "£", eur: "€", cad: "CA$", brl: "R$" }[code.toLowerCase()] ?? "");

  const tabBtn = (t: MainTab) => ({
    background: "none", border: "none", fontSize: "11px", fontWeight: "bold" as const, letterSpacing: "1px",
    cursor: "pointer", padding: "4px 8px", borderRadius: "4px",
    color: activeTab === t ? "var(--primary)" : "var(--text-dim)",
    backgroundColor: activeTab === t ? "rgba(173,92,47,0.1)" : "transparent",
  });

  return (
    <div style={{ padding: "1.5rem", height: "100%", display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      {fpLow && (
        <div style={{ padding: "0.75rem 1.25rem", backgroundColor: "rgba(239,68,68,0.15)", border: "1px solid rgba(239,68,68,0.4)", borderRadius: "12px", color: "#f87171", fontSize: "12px", fontWeight: "700" }}>
          ⚠ FEEPAYER LOW — {wallets?.feepayer?.balance_sol} (threshold: {feepayerThreshold} SOL). TOP UP REQUIRED.
        </div>
      )}

      <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: "1.25rem" }}>
        <MetricCard label="ACTIVE SESSIONS" value={activeSessions} icon="🎮" color="var(--primary)" />
        <MetricCard label="TOTAL GAMES"     value={totalGames}    icon="♟️" color="#3b82f6" />
        <MetricCard label="CONFIRMED TXS"   value={txConfirmed}   icon="⛓️" color="var(--accent)" />
        <MetricCard label="TREASURY"        value={wallets?.treasury?.balance_sol || "0.00 SOL"} icon="💰" color="#4ade80" />
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "3fr 1fr", gap: "1.5rem", flex: 1, minHeight: 0 }}>
        <div style={{ display: "flex", flexDirection: "column", backgroundColor: "rgba(10,33,26,0.4)", backdropFilter: "blur(20px)", border: "1px solid var(--border)", borderRadius: "24px", overflow: "hidden" }}>
          {/* Tab bar */}
          <div style={{ padding: "0.75rem 1.25rem", backgroundColor: "rgba(255,255,255,0.05)", borderBottom: "1px solid var(--border)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <div style={{ display: "flex", gap: "0.5rem" }}>
              {(["CONSOLE", "MODERATION", "AUDIT", "INFRA"] as MainTab[]).map(t => (
                <button key={t} onClick={() => setActiveTab(t)} style={tabBtn(t)}>
                  {t}{t === "MODERATION" && reports.length > 0 ? ` (${reports.length})` : ""}
                </button>
              ))}
            </div>
          </div>

          {/* CONSOLE */}
          {activeTab === "CONSOLE" && (
            <div style={{ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" }}>
              <div style={{ flex: 1, padding: "1.25rem", overflowY: "auto", fontFamily: "monospace", fontSize: "12px", lineHeight: "1.6" }}>
                {logs.length === 0 && <div style={{ color: "rgba(255,255,255,0.1)" }}>Waiting for log output…</div>}
                {logs.map((l, i) => {
                  const col = l.includes("ERROR") ? "#f87171" : l.includes("WARN") ? "#fbbf24" : "var(--primary)";
                  return <div key={i} style={{ marginBottom: "2px", color: col }}><span style={{ color: "var(--accent)", marginRight: "8px", opacity: 0.7 }}>&gt;</span>{l}</div>;
                })}
              </div>
              {deployHistory.length > 0 && (
                <div style={{ padding: "0.75rem 1.25rem", borderTop: "1px solid var(--border)", background: "rgba(0,0,0,0.2)" }}>
                  <div style={{ fontSize: "10px", color: "var(--text-dim)", letterSpacing: "1px", marginBottom: "6px" }}>DEPLOY HISTORY</div>
                  <div style={{ display: "flex", flexDirection: "column", gap: "3px", maxHeight: "80px", overflowY: "auto" }}>
                    {[...deployHistory].reverse().map((d, i) => (
                      <div key={i} style={{ display: "flex", gap: "12px", fontSize: "11px", fontFamily: "monospace" }}>
                        <span style={{ color: d.code === 0 ? "#4ade80" : "#f87171" }}>{d.code === 0 ? "✓" : "✗"}</span>
                        <span style={{ color: "var(--text-dim)" }}>{d.ts}</span>
                        <span style={{ color: "#fff" }}>{d.note}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}

          {/* MODERATION */}
          {activeTab === "MODERATION" && (
            <div style={{ flex: 1, padding: "1.5rem", overflowY: "auto", display: "flex", flexDirection: "column", gap: "1.5rem" }}>
              <div style={{ padding: "1rem 1.25rem", backgroundColor: "rgba(255,255,255,0.03)", borderRadius: "12px", border: "1px solid var(--border)" }}>
                <div style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: "800", letterSpacing: "1px", marginBottom: "4px" }}>IP BAN</div>
                <div style={{ fontSize: "10px", color: "var(--text-dim)", marginBottom: "10px", fontStyle: "italic" }}>Recorded here for reference only — not yet enforced at the network layer (no request-blocking middleware is wired up).</div>
                <div style={{ display: "flex", gap: "8px" }}>
                  <input value={ipBanInput.ip} onChange={e => setIpBanInput(p => ({ ...p, ip: e.target.value }))} placeholder="IP address…"
                    style={{ flex: 1, background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "6px 10px", fontSize: "12px" }} />
                  <input value={ipBanInput.reason} onChange={e => setIpBanInput(p => ({ ...p, reason: e.target.value }))} placeholder="Reason…"
                    style={{ flex: 2, background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "6px 10px", fontSize: "12px" }} />
                  <button onClick={handleIpBan} style={{ padding: "6px 14px", borderRadius: "8px", backgroundColor: "#ef4444", color: "#fff", border: "none", fontSize: "12px", fontWeight: "700", cursor: "pointer" }}>BAN IP</button>
                </div>
                {ipBanMsg && <div style={{ marginTop: "6px", fontSize: "11px", color: ipBanMsg.startsWith("Error") ? "#f87171" : "#4ade80" }}>{ipBanMsg}</div>}
              </div>

              <div style={{ padding: "1rem 1.25rem", backgroundColor: "rgba(255,255,255,0.03)", borderRadius: "12px", border: "1px solid var(--border)" }}>
                <div style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: "800", letterSpacing: "1px", marginBottom: "10px" }}>DISPUTE RESOLUTION — 2ND FACTOR</div>
                <input value={disputeAdminToken} onChange={e => setDisputeAdminToken(e.target.value)} type="password" placeholder="ADMIN_TOKEN (required to resolve any dispute below)…"
                  style={{ width: "100%", background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "6px 10px", fontSize: "12px", boxSizing: "border-box" }} />
              </div>

              {reports.length === 0
                ? <div style={{ color: "var(--text-dim)", fontStyle: "italic" }}>No games flagged for review.</div>
                : reports.map(r => {
                    const openSecs = now - (r.created_at ?? now);
                    const openHrs = Math.floor(openSecs / 3600);
                    const stale = openHrs >= 48;
                    return (
                      <div key={r.game_id} style={{ backgroundColor: "rgba(239,68,68,0.05)", border: `1px solid ${stale ? "rgba(251,191,36,0.4)" : "rgba(239,68,68,0.2)"}`, borderRadius: "12px", padding: "1.25rem", display: "flex", flexDirection: "column", gap: "0.75rem" }}>
                        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
                          <div>
                            <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                              <span style={{ color: "#fff", fontWeight: "bold", fontSize: "16px" }}>Game #{r.game_id}</span>
                              <span style={{ backgroundColor: r.verdict === "Flag" ? "#ef4444" : "#f59e0b", color: "#fff", padding: "2px 8px", borderRadius: "100px", fontSize: "10px", fontWeight: "bold" }}>{r.verdict.toUpperCase()}</span>
                              {stale && <span style={{ backgroundColor: "rgba(251,191,36,0.2)", color: "#fbbf24", padding: "2px 8px", borderRadius: "100px", fontSize: "10px", fontWeight: "bold" }}>⚠ {openHrs}H OLD</span>}
                            </div>
                            <div style={{ color: "var(--text-dim)", fontSize: "12px", marginTop: "4px" }}>
                              Wager: <span style={{ color: "#4ade80" }}>{r.wager}</span> · Status: {r.status}
                              {r.assigned_to && <span style={{ color: "var(--accent)", marginLeft: "8px" }}>Assigned: {r.assigned_to}</span>}
                            </div>
                          </div>
                          <div style={{ textAlign: "right" }}>
                            <div style={{ fontSize: "12px" }}>Suspect: <strong style={{ color: "#ef4444" }}>{r.suspect}</strong></div>
                            <div style={{ fontSize: "11px", fontFamily: "monospace", color: "var(--text-dim)" }}>{(r.score * 100).toFixed(0)}% match</div>
                            <div style={{ fontSize: "10px", color: "var(--text-dim)" }}>{openHrs > 0 ? `${openHrs}h ago` : "just now"}</div>
                          </div>
                        </div>
                        <div style={{ background: "rgba(0,0,0,0.3)", padding: "10px", borderRadius: "8px", fontSize: "12px", fontFamily: "monospace", color: "var(--text-dim)" }}>
                          {r.reason}
                        </div>
                        {assignedMsg[r.game_id] && <div style={{ fontSize: "11px", color: "#4ade80" }}>{assignedMsg[r.game_id]}</div>}
                        <input value={resolutionNotes[r.game_id] ?? ""} onChange={e => setResolutionNotes(prev => ({ ...prev, [r.game_id]: e.target.value }))}
                          placeholder="Resolution note (required, emailed to both players)…"
                          style={{ background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "6px 10px", fontSize: "12px" }} />
                        {resolvedMsg[r.game_id] && <div style={{ fontSize: "11px", color: resolvedMsg[r.game_id].startsWith("Error") ? "#f87171" : "#4ade80", wordBreak: "break-all" }}>{resolvedMsg[r.game_id]}</div>}
                        <div style={{ display: "flex", gap: "8px", justifyContent: "flex-end" }}>
                          {!r.assigned_to && (
                            <button onClick={() => handleAssignDispute(r.game_id)} disabled={assigningDispute === r.game_id}
                              style={{ background: "transparent", border: "1px solid var(--border)", color: "var(--text-dim)", padding: "6px 12px", borderRadius: "6px", fontSize: "12px", cursor: "pointer" }}>
                              {assigningDispute === r.game_id ? "…" : "Assign to me"}
                            </button>
                          )}
                          <button onClick={() => resolveDispute(r, "winner")} disabled={resolvingDispute === r.game_id}
                            style={{ background: "transparent", border: "1px solid var(--border)", color: "var(--text)", padding: "6px 12px", borderRadius: "6px", fontSize: "12px", cursor: "pointer" }}>
                            {resolvingDispute === r.game_id ? "…" : "False Positive (dismiss)"}
                          </button>
                          <button onClick={() => resolveDispute(r, "refund")} disabled={resolvingDispute === r.game_id}
                            style={{ background: "#ef4444", border: "none", color: "#fff", padding: "6px 12px", borderRadius: "6px", fontSize: "12px", fontWeight: "bold", cursor: "pointer" }}>
                            {resolvingDispute === r.game_id ? "…" : "Award Win to Victim"}
                          </button>
                        </div>
                      </div>
                    );
                  })
              }
            </div>
          )}

          {/* AUDIT */}
          {activeTab === "AUDIT" && (
            <div style={{ flex: 1, overflowY: "auto" }}>
              <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "11px" }}>
                <thead style={{ position: "sticky", top: 0, backgroundColor: "rgba(10,33,26,0.95)" }}>
                  <tr>{["TIME", "ACTOR", "ACTION", "TARGET", "RESULT"].map(h => (
                    <th key={h} style={{ padding: "10px 12px", textAlign: "left", color: "var(--text-dim)", fontWeight: "800", fontSize: "10px", letterSpacing: "1px", borderBottom: "1px solid var(--border)" }}>{h}</th>
                  ))}</tr>
                </thead>
                <tbody>
                  {auditEntries.length === 0 && <tr><td colSpan={5} style={{ padding: "3rem", textAlign: "center", color: "var(--text-dim)", fontStyle: "italic" }}>No audit entries yet.</td></tr>}
                  {auditEntries.map((e, i) => (
                    <tr key={i} style={{ borderBottom: "1px solid rgba(255,255,255,0.03)" }}>
                      <td style={{ padding: "8px 12px", fontFamily: "monospace", color: "var(--text-dim)", whiteSpace: "nowrap" }}>{new Date(e.timestamp * 1000).toLocaleTimeString()}</td>
                      <td style={{ padding: "8px 12px", color: "#fff" }}>{e.actor}</td>
                      <td style={{ padding: "8px 12px", color: "var(--primary)", fontFamily: "monospace" }}>{e.action}</td>
                      <td style={{ padding: "8px 12px", fontFamily: "monospace", color: "var(--text-dim)" }}>{e.target}</td>
                      <td style={{ padding: "8px 12px", color: e.result === "ok" ? "#4ade80" : "var(--text-dim)" }}>{e.result}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {/* INFRA */}
          {activeTab === "INFRA" && (
            <div style={{ flex: 1, padding: "1.5rem", overflowY: "auto", display: "flex", flexDirection: "column", gap: "1.5rem" }}>
              {dbStats && (
                <InfraSection title="DATABASE">
                  <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: "12px" }}>
                    <Stat label="Sessions" value={dbStats.sessions_rows.toLocaleString()} />
                    <Stat label="Games" value={dbStats.games_rows.toLocaleString()} />
                    <Stat label="Users" value={dbStats.users_rows.toLocaleString()} />
                    <Stat label="DB Size" value={`${dbStats.db_mb.toFixed(2)} MB`} />
                  </div>
                </InfraSection>
              )}
              {tlsExpiry.length > 0 && (
                <InfraSection title="TLS CERTIFICATES">
                  {tlsExpiry.map((c, i) => {
                    const warn = c.days_remaining != null && c.days_remaining < 14;
                    return (
                      <div key={i} style={{ display: "flex", justifyContent: "space-between", padding: "8px 0", borderBottom: "1px solid rgba(255,255,255,0.05)", fontSize: "12px" }}>
                        <span style={{ color: "#fff" }}>{c.domain}</span>
                        <span style={{ color: c.status === "no_cert" ? "var(--text-dim)" : warn ? "#fbbf24" : "#4ade80", fontWeight: "700" }}>
                          {c.status === "no_cert" ? "NOT CONFIGURED" : c.days_remaining != null ? `${c.days_remaining}d` : "Found"}
                        </span>
                      </div>
                    );
                  })}
                </InfraSection>
              )}
              {Object.keys(taskStatus).length > 0 && (
                <InfraSection title="SCHEDULED TASKS">
                  {Object.entries(taskStatus).map(([name, t]) => {
                    const age = Math.floor(now - t.last_tick);
                    const stale = age > 60;
                    return (
                      <div key={name} style={{ display: "flex", justifyContent: "space-between", padding: "8px 0", borderBottom: "1px solid rgba(255,255,255,0.05)", fontSize: "12px" }}>
                        <span style={{ color: "#fff", fontFamily: "monospace" }}>{name.replace(/_/g, " ")}</span>
                        <span style={{ color: stale ? "#fbbf24" : "#4ade80", fontSize: "10px", fontWeight: "700" }}>{stale ? `STALE ${age}s` : `OK ${age}s`}</span>
                      </div>
                    );
                  })}
                </InfraSection>
              )}
            </div>
          )}
        </div>

        {/* Sidebar */}
        <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
          <div style={{ padding: "1.5rem", backgroundColor: "var(--surface)", borderRadius: "24px", border: "1px solid var(--border)", backdropFilter: "blur(10px)", display: "flex", flexDirection: "column", gap: "1rem" }}>
            <h3 style={{ color: "var(--primary)", margin: 0, fontSize: "12px", letterSpacing: "2px", fontWeight: "800" }}>OPERATIONS</h3>
            <button onClick={runDeployment} disabled={deploying} className="primary"
              style={{ width: "100%", padding: "1rem", borderRadius: "100px", fontSize: "13px", boxShadow: "0 4px 15px rgba(173,92,47,0.3)" }}>
              {deploying ? "ROLLING OUT…" : "🚀 ROLLOUT UPDATE"}
            </button>
            <div>
              <div style={{ fontSize: "10px", color: "var(--text-dim)", letterSpacing: "1px" }}>SERVER HEALTH</div>
              <div style={{ width: "100%", height: "6px", backgroundColor: "rgba(0,0,0,0.3)", borderRadius: "100px", marginTop: "8px", overflow: "hidden" }}>
                <div style={{ width: `${cpuUsage}%`, height: "100%", backgroundColor: cpuUsage > 80 ? "#ef4444" : "var(--primary)", borderRadius: "100px", transition: "width 0.5s" }} />
              </div>
              <div style={{ display: "flex", justifyContent: "space-between", fontSize: "10px", color: "var(--text-dim)", marginTop: "6px", fontWeight: "600" }}>
                <span>CPU: {cpuUsage}%</span><span>RAM: {ramUsage}%</span>
              </div>
            </div>
          </div>

          <div style={{ padding: "1.5rem", backgroundColor: "rgba(255,255,255,0.02)", borderRadius: "24px", border: "1px solid var(--border)" }}>
            <h4 style={{ color: "rgba(255,255,255,0.2)", margin: "0 0 16px", fontSize: "11px", letterSpacing: "1px" }}>ACTIVE NODES</h4>
            <div style={{ display: "flex", flexDirection: "column", gap: "12px", fontSize: "12px", color: "var(--text-dim)" }}>
              {nodeChecks.map(n => (
                <div key={n.label} style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span>{n.label}</span>
                  <span style={{ fontSize: "10px", fontWeight: "bold", padding: "2px 8px", borderRadius: "100px",
                    backgroundColor: n.status === "online" ? "rgba(74,222,128,0.1)" : n.status === "offline" ? "rgba(239,68,68,0.1)" : "rgba(255,255,255,0.04)",
                    color: n.status === "online" ? "#4ade80" : n.status === "offline" ? "#f87171" : "var(--text-dim)",
                    border: `1px solid ${n.status === "online" ? "rgba(74,222,128,0.2)" : n.status === "offline" ? "rgba(239,68,68,0.2)" : "rgba(255,255,255,0.05)"}` }}>
                    {n.status.toUpperCase()}
                  </span>
                </div>
              ))}
            </div>
          </div>

          <div style={{ padding: "1.5rem", backgroundColor: "rgba(255,255,255,0.02)", borderRadius: "24px", border: "1px solid var(--border)" }}>
            <h4 style={{ color: "rgba(255,255,255,0.2)", margin: "0 0 16px", fontSize: "11px", letterSpacing: "1px" }}>PLATFORM WALLETS</h4>
            <div style={{ display: "flex", flexDirection: "column", gap: "12px", fontSize: "12px", color: "var(--text-dim)" }}>
              {(Object.keys(WALLET_LABELS) as WalletKey[]).map(key => (
                <WalletStatus key={key} label={WALLET_LABELS[key]} data={wallets?.[key]} warn={key === "feepayer" && fpLow} usdRate={rates.usd ?? rates.USD} />
              ))}
            </div>
          </div>

          <div style={{ padding: "1.5rem", backgroundColor: "rgba(255,255,255,0.02)", borderRadius: "24px", border: "1px solid var(--border)", flex: 1 }}>
            <h4 style={{ color: "rgba(255,255,255,0.2)", margin: "0 0 16px", fontSize: "11px", letterSpacing: "1px" }}>SOL RATES</h4>
            <div style={{ display: "flex", flexDirection: "column", gap: "12px", fontSize: "12px", color: "var(--text-dim)" }}>
              {Object.keys(rates).length === 0 && <div style={{ fontStyle: "italic", opacity: 0.5 }}>Loading…</div>}
              {Object.entries(rates).map(([cur, rate]) => (
                <div key={cur} style={{ display: "flex", justifyContent: "space-between" }}>
                  <span style={{ textTransform: "uppercase" }}>{cur}</span>
                  <span style={{ color: "var(--primary)", fontWeight: "bold" }}>{sym(cur)}{Number(rate).toFixed(2)}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function MetricCard({ label, value, icon, color }: { label: string; value: string | number; icon: string; color: string }) {
  return (
    <div style={{ backgroundColor: "var(--surface)", padding: "1.5rem", borderRadius: "24px", border: "1px solid var(--border)", display: "flex", flexDirection: "column", gap: "0.5rem", position: "relative", overflow: "hidden", backdropFilter: "blur(10px)" }}>
      <div style={{ position: "absolute", top: "-10px", right: "-10px", fontSize: "48px", opacity: 0.05, filter: "grayscale(1)" }}>{icon}</div>
      <div style={{ fontSize: "10px", color: "var(--text-dim)", letterSpacing: "2px", fontWeight: "700" }}>{label}</div>
      <div style={{ fontSize: "28px", fontWeight: "800", color: "#fff" }}>{value}</div>
      <div style={{ width: "32px", height: "4px", backgroundColor: color, borderRadius: "100px" }} />
    </div>
  );
}

function WalletStatus({ label, data, warn, usdRate }: { label: string; data?: { pubkey: string; balance_sol: string; balance_lamports: number }; warn?: boolean; usdRate?: number }) {
  const usd = data && usdRate ? (data.balance_lamports / 1e9) * usdRate : null;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
      <div style={{ display: "flex", justifyContent: "space-between" }}>
        <span style={{ fontWeight: "600", color: "var(--text)" }}>{label}</span>
        <span style={{ color: warn ? "#f87171" : "var(--primary)", fontWeight: "bold" }}>
          {data ? data.balance_sol : "…"}
          {usd !== null && <span style={{ color: "var(--text-dim)", fontWeight: "normal", marginLeft: "6px" }}>(${usd.toFixed(2)})</span>}
        </span>
      </div>
      <div style={{ fontSize: "9px", fontFamily: "monospace", color: "var(--text-dim)", opacity: 0.6, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        {data ? data.pubkey : "Connecting…"}
      </div>
    </div>
  );
}

function InfraSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ backgroundColor: "rgba(255,255,255,0.03)", borderRadius: "12px", padding: "1rem 1.25rem", border: "1px solid var(--border)" }}>
      <div style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: "800", letterSpacing: "1px", marginBottom: "12px" }}>{title}</div>
      {children}
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
      <div style={{ fontSize: "10px", color: "var(--text-dim)" }}>{label}</div>
      <div style={{ fontSize: "16px", fontWeight: "800", color: "#fff" }}>{value}</div>
    </div>
  );
}
