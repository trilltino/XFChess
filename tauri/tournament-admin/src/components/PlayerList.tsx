import { useState, useEffect } from "react";
import { apiClient } from "../services/api";

export default function PlayerList() {
  const [players, setPlayers] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    loadPlayers();
  }, []);

  const loadPlayers = async () => {
    try {
      setLoading(true);
      const response = await apiClient.getPlayers();
      if (response.ok) {
        setPlayers(response.data.players || []);
      } else {
        setError("Failed to load players");
      }
    } catch (err) {
      setError("Network error loading players");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ padding: "1.5rem" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "2rem" }}>
        <div>
          <h1 style={{ margin: 0, color: "white", fontSize: "1.5rem" }}>PLAYER <span style={{ color: "var(--primary)" }}>DIRECTORY</span></h1>
          <p style={{ color: "var(--text-dim)", margin: "0.25rem 0 0 0" }}>Manage registered users and KYC statuses</p>
        </div>
        <button onClick={loadPlayers} className="primary" style={{ padding: "0.6rem 1.5rem", borderRadius: "100px" }}>REFRESH</button>
      </div>

      {error && <div style={{ color: "#ef4444", marginBottom: "1rem" }}>{error}</div>}

      <div style={{ 
        backgroundColor: "var(--surface)", 
        borderRadius: "24px", 
        border: "1px solid var(--border)",
        overflow: "hidden"
      }}>
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ textAlign: "left", backgroundColor: "rgba(255,255,255,0.02)", borderBottom: "1px solid var(--border)" }}>
              <th style={{ padding: "1rem", color: "var(--text-dim)", fontSize: "12px" }}>USERNAME</th>
              <th style={{ padding: "1rem", color: "var(--text-dim)", fontSize: "12px" }}>WALLET ADDRESS</th>
              <th style={{ padding: "1rem", color: "var(--text-dim)", fontSize: "12px" }}>KYC STATUS</th>
              <th style={{ padding: "1rem", color: "var(--text-dim)", fontSize: "12px" }}>ACTIONS</th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr><td colSpan={4} style={{ padding: "3rem", textAlign: "center", color: "var(--text-dim)" }}>Loading players...</td></tr>
            ) : players.length === 0 ? (
              <tr><td colSpan={4} style={{ padding: "3rem", textAlign: "center", color: "var(--text-dim)" }}>No players registered.</td></tr>
            ) : (
              players.map(player => (
                <tr key={player.wallet} style={{ borderBottom: "1px solid rgba(255,255,255,0.02)" }}>
                  <td style={{ padding: "1rem", color: "white", fontWeight: "bold" }}>{player.username}</td>
                  <td style={{ padding: "1rem", color: "var(--text-dim)", fontFamily: "monospace", fontSize: "13px" }}>{player.wallet}</td>
                  <td style={{ padding: "1rem" }}>
                    <span style={{ 
                      fontSize: "10px", 
                      padding: "2px 8px", 
                      borderRadius: "100px", 
                      backgroundColor: player.kyc_status === "verified" ? "rgba(34, 197, 94, 0.1)" : "rgba(234, 179, 8, 0.1)",
                      color: player.kyc_status === "verified" ? "#22c55e" : "#eab308",
                      border: `1px solid ${player.kyc_status === "verified" ? "#22c55e" : "#eab308"}44`
                    }}>
                      {player.kyc_status.toUpperCase()}
                    </span>
                  </td>
                  <td style={{ padding: "1rem" }}>
                    <button style={{ backgroundColor: "transparent", border: "1px solid var(--border)", color: "white", padding: "4px 12px", borderRadius: "4px", fontSize: "11px", cursor: "pointer" }}>
                      VIEW HISTORY
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
