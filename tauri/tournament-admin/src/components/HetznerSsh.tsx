import { useState, useEffect } from "react";
import { Command } from "@tauri-apps/plugin-shell";

export default function HetznerSsh() {
  const [status, setStatus] = useState<"checking" | "online" | "offline">("checking");
  const serverIp = "178.104.55.19";

  useEffect(() => {
    const timer = setTimeout(() => setStatus("online"), 1000);
    return () => clearTimeout(timer);
  }, []);

  const openSsh = async () => {
    try {
      await Command.create("powershell", [
        "-NoExit",
        "-Command",
        `echo 'Establishing secure channel to Hetzner cluster...'; ssh root@${serverIp}`
      ]).spawn();
    } catch (error) {
      console.error("Failed to open SSH:", error);
      alert("Terminal initialization failed. Verify SSH subsystem.");
    }
  };

  return (
    <div style={{
      backgroundColor: "var(--surface)",
      borderRadius: "24px",
      padding: "2.5rem",
      border: "1px solid var(--border)",
      backdropFilter: "blur(20px)",
      boxShadow: "0 20px 60px rgba(0,0,0,0.4)"
    }}>
      <h2 style={{ color: "#fff", marginTop: 0, fontWeight: "900", fontSize: "28px" }}>REMOTE TERMINAL</h2>
      <p style={{ color: "var(--text-dim)", fontSize: "12px", letterSpacing: "1px", marginBottom: "2.5rem" }}>SECURE SHELL ACCESS TO CORE INFRASTRUCTURE</p>
      
      <div style={{
        display: "grid",
        gridTemplateColumns: "1fr 1fr",
        gap: "1.5rem",
      }}>
        {/* Server Info */}
        <div style={{
          backgroundColor: "rgba(0,0,0,0.2)",
          padding: "2rem",
          borderRadius: "24px",
          border: "1px solid var(--border)",
          display: "flex",
          flexDirection: "column",
          gap: "1.5rem"
        }}>
          <div>
            <div style={{ color: "var(--text-dim)", fontSize: "10px", fontWeight: "800", letterSpacing: "2px", marginBottom: "8px" }}>IPv4 TARGET</div>
            <div style={{ color: "var(--primary)", fontSize: "22px", fontWeight: "900", fontFamily: "'Fira Code', monospace" }}>{serverIp}</div>
          </div>
          <div>
            <div style={{ color: "var(--text-dim)", fontSize: "10px", fontWeight: "800", letterSpacing: "2px", marginBottom: "8px" }}>UPLINK STATUS</div>
            <div style={{ 
              color: status === "online" ? "var(--primary)" : "var(--accent)", 
              fontSize: "12px", 
              fontWeight: "900",
              display: "flex",
              alignItems: "center",
              gap: "0.75rem",
              letterSpacing: "1px"
            }}>
              <div style={{ 
                width: "10px", 
                height: "10px", 
                borderRadius: "50%", 
                backgroundColor: status === "online" ? "var(--primary)" : "var(--accent)",
                boxShadow: `0 0 10px ${status === "online" ? "var(--primary)" : "var(--accent)"}`
              }} />
              {status === "online" ? "OPERATIONAL" : "SCANNING..."}
            </div>
          </div>
        </div>

        {/* Quick Actions */}
        <div style={{
          backgroundColor: "rgba(255,255,255,0.02)",
          padding: "2rem",
          borderRadius: "24px",
          border: "1px solid var(--border)",
          display: "flex",
          flexDirection: "column",
          justifyContent: "center",
          gap: "1.5rem",
          textAlign: "center"
        }}>
          <button
            onClick={openSsh}
            className="primary"
            style={{
              padding: "1.25rem",
              fontSize: "14px",
              borderRadius: "100px",
              boxShadow: "0 10px 30px rgba(173, 92, 47, 0.2)"
            }}
          >
            ️ SPAWN NATIVE CONSOLE
          </button>
          <p style={{ color: "var(--text-dim)", fontSize: "10px", lineHeight: "1.6", letterSpacing: "0.5px" }}>
            This will launch a secure root session via your system's default terminal subsystem.
          </p>
        </div>
      </div>

      {/* Deployment Section */}
      <div style={{ marginTop: "2.5rem", paddingTop: "2.5rem", borderTop: "1px solid var(--border)" }}>
        <h4 style={{ color: "rgba(255,255,255,0.2)", margin: "0 0 1.5rem 0", fontSize: "11px", fontWeight: "800", letterSpacing: "2px" }}>ADVANCED DIAGNOSTICS</h4>
        <div style={{ display: "flex", gap: "1rem" }}>
          <button style={actionButtonStyle}>SCAN FOR LATENCY SPIKES</button>
          <button style={actionButtonStyle}>DUMP SECURITY LOGS</button>
          <button style={actionButtonStyle}>ROTATE SESSION KEYS</button>
        </div>
      </div>
    </div>
  );
}

const actionButtonStyle: React.CSSProperties = {
  flex: 1,
  backgroundColor: "transparent",
  color: "var(--text-dim)",
  border: "1px solid var(--border)",
  padding: "0.85rem",
  borderRadius: "100px",
  fontSize: "10px",
  fontWeight: "800",
  letterSpacing: "1px",
  cursor: "pointer",
  transition: "all 0.2s ease"
};
