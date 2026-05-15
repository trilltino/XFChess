import { useState, useEffect } from "react";
import { Command } from "@tauri-apps/plugin-shell";
import { apiClient } from "../services/api";

interface WalletData {
  pubkey: string;
  balance_lamports: number;
  balance_sol: string;
}

interface Report {
  game_id: number;
  white: string;
  black: string;
  suspect: string;
  verdict: string;
  wager: string;
  score: number;
  reason: string;
  status: string;
}

interface Stats {
  activeSessions: number;
  totalGames: number;
  transactionsConfirmed: number;
  cpuUsage: number;
  ramUsage: number;
  rates: Record<string, number>;
  wallets?: {
    feepayer: WalletData;
    vps_signer: WalletData;
    kyc_signer: WalletData;
    treasury: WalletData;
  };
  reports: Report[];
}

export default function Dashboard() {
  const [activeTab, setActiveTab] = useState<"CONSOLE" | "MODERATION">("CONSOLE");
  const [stats, setStats] = useState<Stats>({
    activeSessions: 0,
    totalGames: 0,
    transactionsConfirmed: 0,
    cpuUsage: 0,
    ramUsage: 0,
    rates: {},
    reports: [],
  });
  const [logs, setLogs] = useState<string[]>([]);
  const [deploying, setDeploying] = useState(false);
  const serverIp = "178.104.55.19";

  const addLog = (msg: string) => {
    setLogs(prev => [...prev.slice(-100), `[${new Date().toLocaleTimeString()}] ${msg}`]);
  };

  useEffect(() => {
    const fetchAllMetrics = async () => {
      try {
        const backendRes = await fetch(`http://${serverIp}:8090/metrics`);
        const backendText = await backendRes.text();
        
        const activeSessions = parseInt(backendText.match(/active_sessions (\d+)/)?.[1] || "0");
        const totalGames = parseInt(backendText.match(/games_created_total (\d+)/)?.[1] || "0");
        const txConfirmed = parseInt(backendText.match(/transactions_confirmed_total\{chain="solana"\} (\d+)/)?.[1] || "0");

        const promQuery = async (query: string) => {
          const res = await fetch(`http://${serverIp}:9090/api/v1/query?query=${encodeURIComponent(query)}`);
          const json = await res.json();
          return json.data.result[0]?.value[1] || "0";
        };

        const cpu = parseFloat(await promQuery('100 - (avg(rate(node_cpu_seconds_total{mode="idle"}[1m])) * 100)'));
        const ramUsed = parseFloat(await promQuery('node_memory_MemTotal_bytes - node_memory_MemAvailable_bytes'));
        const ramTotal = parseFloat(await promQuery('node_memory_MemTotal_bytes'));
        const ramPercent = (ramUsed / ramTotal) * 100;

        const walletRes = await apiClient.getWalletBalances();
        const wallets = walletRes.ok ? walletRes.data : undefined;

        const ratesRes = await apiClient.getExchangeRates();
        const rates = ratesRes.ok ? ratesRes.data.rates : {};

        const reportsRes = await apiClient.getAntiCheatReports();
        const reports = reportsRes.ok ? reportsRes.data.reports : [];

        setStats({
          activeSessions,
          totalGames,
          transactionsConfirmed: txConfirmed,
          cpuUsage: Math.round(cpu),
          ramUsage: Math.round(ramPercent),
          rates,
          wallets,
          reports,
        });
      } catch (err) {
        console.error("Failed to fetch metrics:", err);
      }
    };

    fetchAllMetrics();
    const interval = setInterval(fetchAllMetrics, 5000);
    return () => clearInterval(interval);
  }, []);

  const runDeployment = async () => {
    setDeploying(true);
    addLog("Initiating production rollout...");
    try {
      const command = Command.sidecar("../deploy/scripts/deploy.bat");
      const child = await command.spawn();
      command.stdout.on('data', line => addLog(line));
      command.stderr.on('data', line => addLog(`ERROR: ${line}`));
      command.on('close', data => {
        addLog(`Rollout finished (Exit code: ${data.code})`);
        setDeploying(false);
      });
    } catch (err) {
      addLog(`Failed: ${err}`);
      setDeploying(false);
    }
  };

  const getCurrencySymbol = (code: string) => {
    switch (code.toLowerCase()) {
      case 'usd': return '$';
      case 'gbp': return '£';
      case 'eur': return '€';
      case 'cad': return 'CA$';
      case 'brl': return 'R$';
      default: return '';
    }
  };

  const resolveDispute = async (gameId: number, action: "refund" | "winner") => {
    // In a real app, this would call the Rust backend to sign the ResolveDispute instruction
    // using the local dispute_authority Keypair.
    addLog(`Calling ResolveDispute on-chain for game ${gameId} with action: ${action}`);
    alert(`Signed ResolveDispute instruction for Game ${gameId}. Result: ${action === 'refund' ? 'Victim Refunded' : 'False Positive Cleared'}`);
  };

  return (
    <div style={{ padding: "1.5rem", height: "100%", display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      {/* Top Section: Metrics Cards */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: "1.25rem" }}>
        <MetricCard label="ACTIVE SESSIONS" value={stats.activeSessions} icon="🎮" color="var(--primary)" />
        <MetricCard label="TOTAL GAMES" value={stats.totalGames} icon="♟️" color="#3b82f6" />
        <MetricCard label="CONFIRMED TXS" value={stats.transactionsConfirmed} icon="⛓️" color="var(--accent)" />
        <MetricCard label="TREASURY BALANCE" value={stats.wallets?.treasury?.balance_sol || "0.00 SOL"} icon="💰" color="#4ade80" />
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "3fr 1fr", gap: "1.5rem", flex: 1, minHeight: 0 }}>
        {/* Main Console Area */}
        <div style={{ 
          display: "flex", 
          flexDirection: "column", 
          backgroundColor: "rgba(10, 33, 26, 0.4)",
          backdropFilter: "blur(20px)",
          border: "1px solid var(--border)",
          borderRadius: "24px",
          overflow: "hidden",
          boxShadow: "0 10px 40px rgba(0,0,0,0.3)"
        }}>
          <div style={{ 
            padding: "0.75rem 1.25rem", 
            backgroundColor: "rgba(255,255,255,0.05)", 
            borderBottom: "1px solid var(--border)",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center"
          }}>
            <div style={{ display: "flex", gap: "1rem" }}>
              <button 
                onClick={() => setActiveTab("CONSOLE")}
                style={{ 
                  background: "none", border: "none", color: activeTab === "CONSOLE" ? "var(--primary)" : "var(--text-dim)", 
                  fontSize: "11px", fontWeight: "bold", letterSpacing: "1px", cursor: "pointer", padding: "4px 8px", borderRadius: "4px",
                  backgroundColor: activeTab === "CONSOLE" ? "rgba(173, 92, 47, 0.1)" : "transparent"
                }}>
                PRODUCTION CONSOLE
              </button>
              <button 
                onClick={() => setActiveTab("MODERATION")}
                style={{ 
                  background: "none", border: "none", color: activeTab === "MODERATION" ? "#ef4444" : "var(--text-dim)", 
                  fontSize: "11px", fontWeight: "bold", letterSpacing: "1px", cursor: "pointer", padding: "4px 8px", borderRadius: "4px",
                  backgroundColor: activeTab === "MODERATION" ? "rgba(239, 68, 68, 0.1)" : "transparent"
                }}>
                ANTI-CHEAT MODERATION {stats.reports.length > 0 && `(${stats.reports.length})`}
              </button>
            </div>
            <div style={{ display: "flex", gap: "0.5rem" }}>
              <div style={{ width: "8px", height: "8px", borderRadius: "50%", backgroundColor: "rgba(255,255,255,0.1)" }} />
              <div style={{ width: "8px", height: "8px", borderRadius: "50%", backgroundColor: "rgba(255,255,255,0.1)" }} />
              <div style={{ width: "8px", height: "8px", borderRadius: "50%", backgroundColor: activeTab === "MODERATION" ? "#ef4444" : "var(--primary)" }} />
            </div>
          </div>
          
          {activeTab === "CONSOLE" && (
            <div style={{ 
              flex: 1, 
              padding: "1.25rem", 
              overflowY: "auto", 
              fontFamily: "'Fira Code', monospace", 
              fontSize: "12px",
              color: "var(--primary)",
              lineHeight: "1.6"
            }}>
              {logs.length === 0 && <div style={{ color: "rgba(255,255,255,0.1)" }}>Infrastructure link established. Waiting for command...</div>}
              {logs.map((log, i) => (
                <div key={i} style={{ marginBottom: "2px" }}>
                  <span style={{ color: "var(--accent)", marginRight: "8px", opacity: 0.7 }}>&gt;</span>
                  {log}
                </div>
              ))}
            </div>
          )}

          {activeTab === "MODERATION" && (
            <div style={{ flex: 1, padding: "1.5rem", overflowY: "auto" }}>
              {stats.reports.length === 0 ? (
                <div style={{ color: "var(--text-dim)", fontStyle: "italic" }}>No games currently flagged for review.</div>
              ) : (
                <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
                  {stats.reports.map((report) => (
                    <div key={report.game_id} style={{ 
                      backgroundColor: "rgba(239, 68, 68, 0.05)", 
                      border: "1px solid rgba(239, 68, 68, 0.2)", 
                      borderRadius: "12px", 
                      padding: "1.25rem",
                      display: "flex",
                      flexDirection: "column",
                      gap: "1rem"
                    }}>
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
                        <div>
                          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                            <span style={{ color: "white", fontWeight: "bold", fontSize: "16px" }}>Game #{report.game_id}</span>
                            <span style={{ 
                              backgroundColor: report.verdict === "Flag" ? "#ef4444" : "#f59e0b",
                              color: "white", padding: "2px 8px", borderRadius: "100px", fontSize: "10px", fontWeight: "bold"
                            }}>
                              {report.verdict.toUpperCase()}
                            </span>
                          </div>
                          <div style={{ color: "var(--text-dim)", fontSize: "12px", marginTop: "4px" }}>
                            Wager: <span style={{ color: "#4ade80", fontWeight: "bold" }}>{report.wager}</span> | Status: {report.status}
                          </div>
                        </div>
                        <div style={{ textAlign: "right" }}>
                          <div style={{ color: "var(--text)", fontSize: "12px" }}>Suspect: <strong style={{ color: "#ef4444" }}>{report.suspect}</strong></div>
                          <div style={{ color: "var(--text-dim)", fontSize: "11px", fontFamily: "monospace" }}>Score: {(report.score * 100).toFixed(0)}% Match</div>
                        </div>
                      </div>
                      
                      <div style={{ backgroundColor: "rgba(0,0,0,0.3)", padding: "12px", borderRadius: "8px", fontSize: "12px", fontFamily: "monospace", color: "var(--text-dim)" }}>
                        <strong>Reason:</strong> {report.reason}
                      </div>

                      <div style={{ display: "flex", gap: "1rem", justifyContent: "flex-end", marginTop: "4px" }}>
                        <button 
                          onClick={() => resolveDispute(report.game_id, "winner")}
                          style={{ background: "transparent", border: "1px solid var(--border)", color: "var(--text)", padding: "6px 12px", borderRadius: "6px", fontSize: "12px", cursor: "pointer" }}
                        >
                          False Positive (Release Funds)
                        </button>
                        <button 
                          onClick={() => resolveDispute(report.game_id, "refund")}
                          style={{ background: "#ef4444", border: "none", color: "white", padding: "6px 12px", borderRadius: "6px", fontSize: "12px", fontWeight: "bold", cursor: "pointer" }}
                        >
                          Refund Victim (Resolve Dispute)
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
          <div style={{ 
            padding: "1.5rem", 
            backgroundColor: "var(--surface)", 
            borderRadius: "24px", 
            border: "1px solid var(--border)",
            display: "flex",
            flexDirection: "column",
            gap: "1rem",
            backdropFilter: "blur(10px)"
          }}>
            <h3 style={{ color: "var(--primary)", margin: 0, fontSize: "12px", letterSpacing: "2px", fontWeight: "800" }}>OPERATIONS</h3>
            <button 
              onClick={runDeployment}
              disabled={deploying}
              className="primary"
              style={{
                width: "100%",
                padding: "1rem",
                borderRadius: "100px",
                fontSize: "13px",
                boxShadow: "0 4px 15px rgba(173, 92, 47, 0.3)"
              }}
            >
              {deploying ? "ROLLING OUT..." : "🚀 ROLLOUT UPDATE"}
            </button>
            <button 
              style={{
                width: "100%",
                padding: "0.85rem",
                borderRadius: "100px",
                background: "var(--glass)",
                border: "1px solid var(--border)",
                color: "var(--text-dim)",
                fontSize: "12px"
              }}
            >
              ♻️ RESTART VPS
            </button>
            <div style={{ marginTop: "0.5rem" }}>
              <div style={{ fontSize: "10px", color: "var(--text-dim)", letterSpacing: "1px" }}>SERVER HEALTH</div>
              <div style={{ width: "100%", height: "6px", backgroundColor: "rgba(0,0,0,0.3)", borderRadius: "100px", marginTop: "8px", overflow: "hidden" }}>
                <div style={{ width: `${stats.cpuUsage}%`, height: "100%", backgroundColor: stats.cpuUsage > 80 ? "#ef4444" : "var(--primary)", borderRadius: "100px", transition: "width 0.5s cubic-bezier(0.16, 1, 0.3, 1)" }} />
              </div>
              <div style={{ display: "flex", justifyContent: "space-between", fontSize: "10px", color: "var(--text-dim)", marginTop: "6px", fontWeight: "600" }}>
                <span>CPU: {stats.cpuUsage}%</span>
                <span>RAM: {stats.ramUsage}%</span>
              </div>
            </div>
          </div>

          <div style={{ 
            padding: "1.5rem", 
            backgroundColor: "rgba(255,255,255,0.02)", 
            borderRadius: "24px", 
            border: "1px solid var(--border)",
          }}>
            <h4 style={{ color: "rgba(255,255,255,0.2)", margin: "0 0 16px 0", fontSize: "11px", letterSpacing: "1px" }}>ACTIVE NODES</h4>
            <div style={{ fontSize: "12px", color: "var(--text-dim)", display: "flex", flexDirection: "column", gap: "12px" }}>
              <NodeStatus label="VPS Relay" status="ONLINE" />
              <NodeStatus label="Node Exporter" status="ONLINE" />
              <NodeStatus label="Gossip Hub" status="ONLINE" />
            </div>
          </div>

          <div style={{ 
            padding: "1.5rem", 
            backgroundColor: "rgba(255,255,255,0.02)", 
            borderRadius: "24px", 
            border: "1px solid var(--border)",
          }}>
            <h4 style={{ color: "rgba(255,255,255,0.2)", margin: "0 0 16px 0", fontSize: "11px", letterSpacing: "1px" }}>PLATFORM WALLETS</h4>
            <div style={{ fontSize: "12px", color: "var(--text-dim)", display: "flex", flexDirection: "column", gap: "12px" }}>
              <WalletStatus label="Feepayer" data={stats.wallets?.feepayer} />
              <WalletStatus label="VPS Signer" data={stats.wallets?.vps_signer} />
              <WalletStatus label="KYC Signer" data={stats.wallets?.kyc_signer} />
            </div>
          </div>

          <div style={{ 
            padding: "1.5rem", 
            backgroundColor: "rgba(255,255,255,0.02)", 
            borderRadius: "24px", 
            border: "1px solid var(--border)",
            flex: 1
          }}>
            <h4 style={{ color: "rgba(255,255,255,0.2)", margin: "0 0 16px 0", fontSize: "11px", letterSpacing: "1px" }}>SOL EXCHANGE RATES</h4>
            <div style={{ fontSize: "12px", color: "var(--text-dim)", display: "flex", flexDirection: "column", gap: "12px" }}>
              {Object.keys(stats.rates).length > 0 ? (
                Object.entries(stats.rates).map(([currency, rate]) => (
                  <div key={currency} style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <span style={{ textTransform: "uppercase" }}>{currency}</span>
                    <span style={{ 
                      color: "var(--primary)",
                      fontWeight: "bold",
                    }}>{getCurrencySymbol(currency)}{Number(rate).toFixed(2)}</span>
                  </div>
                ))
              ) : (
                <div style={{ fontStyle: "italic", opacity: 0.5 }}>Loading rates...</div>
              )}
            </div>
          </div>

        </div>
      </div>
    </div>
  );
}

function MetricCard({ label, value, icon, color }: { label: string, value: string | number, icon: string, color: string }) {
  return (
    <div style={{
      backgroundColor: "var(--surface)",
      padding: "1.5rem",
      borderRadius: "24px",
      border: "1px solid var(--border)",
      display: "flex",
      flexDirection: "column",
      gap: "0.5rem",
      position: "relative",
      overflow: "hidden",
      backdropFilter: "blur(10px)",
      boxShadow: "0 4px 20px rgba(0,0,0,0.2)"
    }}>
      <div style={{ position: "absolute", top: "-10px", right: "-10px", fontSize: "48px", opacity: 0.05, filter: "grayscale(1)" }}>{icon}</div>
      <div style={{ fontSize: "10px", color: "var(--text-dim)", letterSpacing: "2px", fontWeight: "700" }}>{label}</div>
      <div style={{ fontSize: "28px", fontWeight: "800", color: "#fff" }}>{value}</div>
      <div style={{ width: "32px", height: "4px", backgroundColor: color, borderRadius: "100px" }} />
    </div>
  );
}

function NodeStatus({ label, status }: { label: string, status: string }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
      <span>{label}</span>
      <span style={{ 
        color: status === "ONLINE" ? "var(--primary)" : "#f59e0b",
        fontSize: "10px",
        fontWeight: "bold",
        padding: "2px 8px",
        borderRadius: "100px",
        backgroundColor: "rgba(173, 92, 47, 0.1)",
        border: "1px solid rgba(173, 92, 47, 0.2)"
      }}>{status}</span>
    </div>
  );
}

function WalletStatus({ label, data }: { label: string, data?: { pubkey: string, balance_sol: string } }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <span style={{ fontWeight: "600", color: "var(--text)" }}>{label}</span>
        <span style={{ 
          color: "var(--primary)",
          fontWeight: "bold",
        }}>{data ? data.balance_sol : "..."}</span>
      </div>
      <div style={{ 
        fontSize: "9px", 
        fontFamily: "'Fira Code', monospace", 
        color: "var(--text-dim)", 
        opacity: 0.6,
        whiteSpace: "nowrap",
        overflow: "hidden",
        textOverflow: "ellipsis"
      }}>
        {data ? data.pubkey : "Connecting..."}
      </div>
    </div>
  );
}
