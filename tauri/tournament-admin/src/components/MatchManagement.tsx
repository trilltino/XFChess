import { useState, useEffect } from "react";
import { apiClient } from "../services/api";

export default function MatchManagement() {
  const [sessions, setSessions] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    loadSessions();
    const interval = setInterval(loadSessions, 10000); // Auto-refresh active matches every 10s
    return () => clearInterval(interval);
  }, []);

  const loadSessions = async () => {
    try {
      const response = await apiClient.getActiveSessions();
      if (response.ok) {
        setSessions(response.data.sessions || []);
      }
    } catch (err) {
      console.error("Error loading sessions", err);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ padding: "1.5rem" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "2rem" }}>
        <div>
          <h1 style={{ margin: 0, color: "white", fontSize: "1.5rem" }}>LIVE <span style={{ color: "var(--primary)" }}>MATCHES</span></h1>
          <p style={{ color: "var(--text-dim)", margin: "0.25rem 0 0 0" }}>Real-time monitoring of active game sessions</p>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
          <div style={{ fontSize: "12px", color: "var(--primary)", fontWeight: "bold" }}>● {sessions.length} ACTIVE</div>
          <button onClick={loadSessions} className="primary" style={{ padding: "0.6rem 1.5rem", borderRadius: "100px" }}>REFRESH</button>
        </div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(400px, 1fr))", gap: "1.5rem" }}>
        {loading && sessions.length === 0 ? (
          <div style={{ gridColumn: "1 / -1", textAlign: "center", padding: "4rem", color: "var(--text-dim)" }}>Scanning network for active nodes...</div>
        ) : sessions.length === 0 ? (
          <div style={{ gridColumn: "1 / -1", textAlign: "center", padding: "4rem", color: "var(--text-dim)", border: "1px dashed var(--border)", borderRadius: "24px" }}>
            No active matches currently in progress.
          </div>
        ) : (
          sessions.map(session => (
            <div key={session.game_id} style={{ 
              backgroundColor: "var(--surface)", 
              padding: "1.5rem", 
              borderRadius: "24px", 
              border: "1px solid var(--border)",
              display: "flex",
              flexDirection: "column",
              gap: "1rem",
              backdropFilter: "blur(10px)"
            }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ color: "var(--primary)", fontSize: "12px", fontWeight: "bold" }}>GAME #{session.game_id}</span>
                <span style={{ fontSize: "10px", color: "#4ade80", fontWeight: "bold", padding: "2px 8px", backgroundColor: "rgba(74, 222, 128, 0.1)", borderRadius: "100px" }}>LIVE</span>
              </div>
              
              <div style={{ display: "flex", justifyContent: "center", alignItems: "center", gap: "1rem", padding: "1rem 0" }}>
                <div style={{ textAlign: "center", flex: 1 }}>
                  <div style={{ color: "white", fontWeight: "bold" }}>{session.white.slice(0, 4)}...{session.white.slice(-4)}</div>
                  <div style={{ fontSize: "10px", color: "var(--text-dim)" }}>WHITE</div>
                </div>
                <div style={{ color: "var(--primary)", fontWeight: "bold" }}>VS</div>
                <div style={{ textAlign: "center", flex: 1 }}>
                  <div style={{ color: "white", fontWeight: "bold" }}>{session.black.slice(0, 4)}...{session.black.slice(-4)}</div>
                  <div style={{ fontSize: "10px", color: "var(--text-dim)" }}>BLACK</div>
                </div>
              </div>

              <div style={{ backgroundColor: "rgba(0,0,0,0.2)", padding: "0.75rem", borderRadius: "12px", fontSize: "11px", fontFamily: "monospace", color: "var(--accent)" }}>
                FEN: {session.fen.length > 50 ? session.fen.slice(0, 50) + "..." : session.fen}
              </div>

              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ fontSize: "10px", color: "var(--text-dim)" }}>LAST MOVE: {new Date(session.last_activity * 1000).toLocaleTimeString()}</span>
                <button style={{ backgroundColor: "rgba(239, 68, 68, 0.1)", border: "1px solid rgba(239, 68, 68, 0.2)", color: "#ef4444", padding: "4px 12px", borderRadius: "100px", fontSize: "11px", cursor: "pointer" }}>
                  FORCE ABORT
                </button>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
