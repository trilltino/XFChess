import { useState } from "react";
import { Command } from "@tauri-apps/plugin-shell";

export default function DeploymentManager() {
  const [logs, setLogs] = useState<string[]>([]);
  const [deploying, setDeploying] = useState(false);

  const addLog = (msg: string) => {
    setLogs(prev => [...prev.slice(-100), `[${new Date().toLocaleTimeString()}] ${msg}`]);
  };

  const runDeployment = async () => {
    setDeploying(true);
    addLog("Initiating cross-service deployment sequence...");
    
    try {
      const command = Command.sidecar("../deploy/scripts/deploy.bat");
      const child = await command.spawn();
      addLog(`Deployment handshake success (PID: ${child.pid})`);

      command.stdout.on('data', line => addLog(line));
      command.stderr.on('data', line => addLog(`ERR: ${line}`));

      command.on('close', data => {
        addLog(`Deployment sequence complete. Exit code: ${data.code}`);
        setDeploying(false);
      });

      command.on('error', error => {
        addLog(`System error: ${error}`);
        setDeploying(false);
      });

    } catch (err) {
      addLog(`Fatal exception: ${err}`);
      setDeploying(false);
    }
  };

  return (
    <div style={{ padding: "1.5rem", height: "100%", display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div>
          <h2 style={{ color: "#fff", margin: 0, fontWeight: "900", fontSize: "28px" }}>INFRASTRUCTURE ROLLOUT</h2>
          <p style={{ color: "var(--text-dim)", margin: "4px 0", fontSize: "12px", letterSpacing: "1px" }}>PRODUCTION CLUSTER: 178.104.55.19</p>
        </div>
        <button
          onClick={runDeployment}
          disabled={deploying}
          className="primary"
          style={{
            padding: "1rem 2.5rem",
            borderRadius: "100px",
            fontSize: "14px",
            boxShadow: "0 10px 30px rgba(173, 92, 47, 0.2)"
          }}
        >
          {deploying ? "ROLLING OUT..." : "EXECUTE DEPLOYMENT"}
        </button>
      </div>

      <div style={{ 
        flex: 1,
        backgroundColor: "rgba(10, 33, 26, 0.4)", 
        backdropFilter: "blur(20px)",
        border: "1px solid var(--border)", 
        borderRadius: "24px", 
        padding: "1.5rem",
        overflowY: "auto",
        fontFamily: "'Fira Code', monospace",
        fontSize: "12px",
        color: "var(--primary)",
        boxShadow: "0 10px 40px rgba(0,0,0,0.3)"
      }}>
        {logs.length === 0 && <div style={{ color: "rgba(255,255,255,0.05)" }}>Uplink active. Ready for instructions...</div>}
        {logs.map((log, i) => (
          <div key={i} style={{ marginBottom: "4px", paddingBottom: "4px", borderBottom: "1px solid rgba(255,255,255,0.02)" }}>
            <span style={{ color: "var(--accent)", marginRight: "8px", opacity: 0.7 }}>&gt;</span>
            {log}
          </div>
        ))}
      </div>
      
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "1.5rem" }}>
        <ActionCard title="QUICK OPS">
          <button style={actionButtonStyle}>PING REMOTE TARGET</button>
          <button style={actionButtonStyle}>RESTART NGINX GATEWAY</button>
          <button style={actionButtonStyle}>FETCH SERVICE LOGS</button>
        </ActionCard>
        
        <ActionCard title="MONITORING ENDPOINTS">
          <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
            <EndpointRow label="PROMETHEUS" url="http://178.104.55.19:9090" />
            <EndpointRow label="GRAFANA" url="http://178.104.55.19:3000" />
            <EndpointRow label="BACKEND HEALTH" url="http://178.104.55.19:8090/health" />
          </div>
        </ActionCard>
      </div>
    </div>
  );
}

const ActionCard = ({ title, children }: { title: string, children: React.ReactNode }) => (
  <div style={{ 
    backgroundColor: "var(--surface)", 
    padding: "1.5rem", 
    borderRadius: "24px",
    border: "1px solid var(--border)",
    backdropFilter: "blur(10px)"
  }}>
    <h4 style={{ margin: "0 0 16px 0", color: "var(--primary)", fontSize: "11px", fontWeight: "800", letterSpacing: "2px" }}>{title}</h4>
    {children}
  </div>
);

const actionButtonStyle: React.CSSProperties = {
  width: "100%",
  padding: "0.75rem",
  marginBottom: "8px",
  backgroundColor: "rgba(255,255,255,0.03)",
  color: "#fff",
  border: "1px solid var(--border)",
  borderRadius: "100px",
  cursor: "pointer",
  fontSize: "11px",
  fontWeight: "700",
  transition: "all 0.2s ease"
};

const EndpointRow = ({ label, url }: { label: string, url: string }) => (
  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
    <span style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: "700" }}>{label}</span>
    <code style={{ 
        fontSize: "10px", 
        color: "var(--accent)", 
        backgroundColor: "rgba(0,0,0,0.2)", 
        padding: "4px 10px", 
        borderRadius: "100px",
        border: "1px solid var(--border)"
    }}>{url}</code>
  </div>
);
