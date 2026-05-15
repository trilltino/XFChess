import { useState, useEffect } from "react";
import { apiClient } from "../services/api";

export default function GameExplorer() {
  const [searchQuery, setSearchQuery] = useState("");
  const [searchType, setSearchType] = useState<"username" | "wallet">("username");
  const [games, setGames] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [selectedGameMoves, setSelectedGameMoves] = useState<any[] | null>(null);
  const [selectedGameId, setSelectedGameId] = useState<string | null>(null);
  const [archiveStats, setArchiveStats] = useState<any>(null);

  useEffect(() => {
    loadArchiveStats();
  }, []);

  const loadArchiveStats = async () => {
    const response = await apiClient.getArchiveStats();
    if (response.ok) {
      setArchiveStats(response.data);
    }
  };

  const handleDownload = (type: "games" | "wallets") => {
    const url = apiClient.getArchiveDownloadUrl(type);
    window.open(url, "_blank");
  };

  const handleSearch = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!searchQuery.trim()) return;

    setLoading(true);
    setError("");
    setGames([]);
    setSelectedGameMoves(null);

    try {
      let response;
      if (searchType === "username") {
        response = await apiClient.getGameHistoryByUsername(searchQuery);
      } else {
        response = await apiClient.getGameHistory(searchQuery);
      }

      if (response.ok) {
        setGames(response.data.games || []);
      } else {
        setError(response.error?.message || "Search failed");
      }
    } catch (err) {
      setError("Network error during search");
    } finally {
      setLoading(false);
    }
  };

  const fetchMoves = async (gameId: string) => {
    setLoading(true);
    setSelectedGameId(gameId);
    try {
      const response = await apiClient.getGameMoves(gameId);
      if (response.ok) {
        setSelectedGameMoves(response.data.moves || []);
      } else {
        setError("Failed to fetch moves");
      }
    } catch (err) {
      setError("Network error fetching moves");
    } finally {
      setLoading(false);
    }
  };

  const formatDate = (ts: number) => {
    return new Date(ts * 1000).toLocaleString();
  };

  const inputStyle: React.CSSProperties = {
    backgroundColor: "rgba(255,255,255,0.03)",
    border: "1px solid rgba(255,255,255,0.1)",
    borderRadius: "12px",
    padding: "0.8rem 1.2rem",
    color: "white",
    fontSize: "14px",
    width: "100%",
    outline: "none",
    transition: "all 0.3s ease"
  };

  const buttonStyle: React.CSSProperties = {
    padding: "0.8rem 1.5rem",
    borderRadius: "12px",
    border: "none",
    cursor: "pointer",
    fontWeight: "bold",
    transition: "all 0.3s ease"
  };

  return (
    <div style={{ padding: "2rem", maxWidth: "1200px", margin: "0 auto" }}>
      <div style={{ 
        display: "flex", 
        flexDirection: "column", 
        gap: "2rem",
        backgroundColor: "var(--surface)",
        padding: "2.5rem",
        borderRadius: "24px",
        border: "1px solid var(--border)",
        boxShadow: "0 20px 60px rgba(0,0,0,0.3)"
      }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <h1 style={{ margin: 0, fontSize: "2rem", color: "white", letterSpacing: "-1px" }}>GAME <span style={{ color: "var(--primary)" }}>EXPLORER</span></h1>
            <p style={{ color: "var(--text-dim)", marginTop: "0.5rem" }}>Watch game logs and history by username or wallet</p>
          </div>

          {archiveStats && (
            <div style={{ 
              display: "flex", 
              gap: "2rem", 
              backgroundColor: "rgba(0,0,0,0.2)", 
              padding: "1rem 1.5rem", 
              borderRadius: "16px",
              border: "1px solid var(--border)"
            }}>
              <div>
                <div style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: "bold" }}>ARCHIVE SIZE</div>
                <div style={{ color: "var(--accent)", fontWeight: "900" }}>{(archiveStats.games_archive_size_bytes / 1024).toFixed(1)} KB</div>
              </div>
              <div>
                <div style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: "bold" }}>INDEXED WALLETS</div>
                <div style={{ color: "white", fontWeight: "900" }}>{archiveStats.unique_wallets_count}</div>
              </div>
              <div style={{ display: "flex", gap: "0.5rem" }}>
                <button 
                  onClick={() => handleDownload("games")}
                  style={{ ...buttonStyle, fontSize: "10px", padding: "0.5rem 1rem", backgroundColor: "var(--primary)", color: "white" }}
                >
                  DL GAMES (.XFG)
                </button>
                <button 
                  onClick={() => handleDownload("wallets")}
                  style={{ ...buttonStyle, fontSize: "10px", padding: "0.5rem 1rem", border: "1px solid var(--border)", backgroundColor: "transparent", color: "var(--text-dim)" }}
                >
                  DL WALLETS (.IDX)
                </button>
              </div>
            </div>
          )}
        </div>

        <form onSubmit={handleSearch} style={{ display: "flex", gap: "1rem", alignItems: "center" }}>
          <select 
            value={searchType}
            onChange={(e) => setSearchType(e.target.value as any)}
            style={{ ...inputStyle, width: "150px" }}
          >
            <option value="username">USERNAME</option>
            <option value="wallet">WALLET</option>
          </select>
          <input
            type="text"
            placeholder={searchType === "username" ? "Search by username (e.g. Magnus)..." : "Search by wallet address..."}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            style={inputStyle}
          />
          <button type="submit" className="primary" style={buttonStyle} disabled={loading}>
            {loading ? "SEARCHING..." : "SEARCH"}
          </button>
        </form>

        {error && (
          <div style={{ color: "#ef4444", backgroundColor: "rgba(239, 68, 68, 0.1)", padding: "1rem", borderRadius: "12px", border: "1px solid #ef4444" }}>
            {error}
          </div>
        )}

        <div style={{ display: "grid", gridTemplateColumns: selectedGameMoves ? "1fr 1fr" : "1fr", gap: "2rem" }}>
          {/* Game List */}
          <div>
            <h3 style={{ color: "var(--primary)", fontSize: "0.8rem", textTransform: "uppercase", letterSpacing: "1px", marginBottom: "1rem" }}>
              RECENT GAMES ({games.length})
            </h3>
            <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
              {games.length === 0 && !loading && (
                <div style={{ color: "var(--text-dim)", textAlign: "center", padding: "3rem", border: "1px dashed var(--border)", borderRadius: "16px" }}>
                  No games found. Try a different search.
                </div>
              )}
              {games.map(game => (
                <div 
                  key={game.id}
                  onClick={() => fetchMoves(game.id)}
                  style={{
                    backgroundColor: selectedGameId === game.id ? "rgba(255,255,255,0.05)" : "transparent",
                    border: `1px solid ${selectedGameId === game.id ? "var(--primary)" : "var(--border)"}`,
                    padding: "1.25rem",
                    borderRadius: "16px",
                    cursor: "pointer",
                    transition: "all 0.2s ease"
                  }}
                >
                  <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "0.5rem" }}>
                    <span style={{ color: "white", fontWeight: "bold" }}>GAME #{game.id.slice(0, 8)}</span>
                    <span style={{ 
                      fontSize: "0.7rem", 
                      padding: "2px 8px", 
                      borderRadius: "100px",
                      backgroundColor: game.status === "completed" ? "rgba(34, 197, 94, 0.1)" : "rgba(234, 179, 8, 0.1)",
                      color: game.status === "completed" ? "#22c55e" : "#eab308"
                    }}>
                      {game.status.toUpperCase()}
                    </span>
                  </div>
                  <div style={{ display: "flex", gap: "1rem", alignItems: "center", marginBottom: "0.5rem" }}>
                    <div style={{ flex: 1 }}>
                      <div style={{ fontSize: "0.7rem", color: "var(--text-dim)" }}>WHITE</div>
                      <div style={{ color: "white", fontSize: "0.9rem" }}>{game.white_username || "Anonymous"}</div>
                    </div>
                    <div style={{ color: "var(--text-dim)" }}>VS</div>
                    <div style={{ flex: 1, textAlign: "right" }}>
                      <div style={{ fontSize: "0.7rem", color: "var(--text-dim)" }}>BLACK</div>
                      <div style={{ color: "white", fontSize: "0.9rem" }}>{game.black_username || "Anonymous"}</div>
                    </div>
                  </div>
                  <div style={{ fontSize: "0.75rem", color: "var(--text-dim)", display: "flex", justifyContent: "space-between" }}>
                    <span>{formatDate(game.start_time)}</span>
                    <span style={{ color: "var(--accent)" }}>{game.stake_amount} SOL STAKE</span>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Move Logs */}
          {selectedGameMoves && (
            <div>
              <h3 style={{ color: "var(--primary)", fontSize: "0.8rem", textTransform: "uppercase", letterSpacing: "1px", marginBottom: "1rem" }}>
                MOVE LOGS (GAME #{selectedGameId?.slice(0, 8)})
              </h3>
              <div style={{ 
                backgroundColor: "rgba(0,0,0,0.2)", 
                borderRadius: "16px", 
                border: "1px solid var(--border)",
                maxHeight: "600px",
                overflowY: "auto",
                padding: "1rem"
              }}>
                <table style={{ width: "100%", borderCollapse: "collapse" }}>
                  <thead>
                    <tr style={{ textAlign: "left", fontSize: "0.7rem", color: "var(--text-dim)", borderBottom: "1px solid var(--border)" }}>
                      <th style={{ padding: "0.5rem" }}>#</th>
                      <th style={{ padding: "0.5rem" }}>PLAYER</th>
                      <th style={{ padding: "0.5rem" }}>MOVE (UCI)</th>
                      <th style={{ padding: "0.5rem" }}>TIME</th>
                    </tr>
                  </thead>
                  <tbody>
                    {selectedGameMoves.length === 0 && (
                      <tr>
                        <td colSpan={4} style={{ padding: "2rem", textAlign: "center", color: "var(--text-dim)" }}>No moves recorded for this game.</td>
                      </tr>
                    )}
                    {selectedGameMoves.map(move => (
                      <tr key={move.id} style={{ fontSize: "0.85rem", borderBottom: "1px solid rgba(255,255,255,0.02)" }}>
                        <td style={{ padding: "0.75rem", color: "var(--primary)" }}>{move.move_number}</td>
                        <td style={{ padding: "0.75rem", color: "white" }}>
                          <span title={move.player}>{move.player.slice(0, 4)}...{move.player.slice(-4)}</span>
                        </td>
                        <td style={{ padding: "0.75rem", fontFamily: "monospace", color: "var(--accent)" }}>{move.move_uci}</td>
                        <td style={{ padding: "0.75rem", color: "var(--text-dim)", fontSize: "0.7rem" }}>{formatDate(move.timestamp).split(",")[1]}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
