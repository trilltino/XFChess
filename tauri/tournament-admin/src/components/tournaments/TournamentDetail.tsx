import { useState, useEffect } from "react";
import { apiClient, type TournamentDetail } from "../../services/api";

interface TournamentDetailProps {
  tournamentId: number;
  onBack: () => void;
  onEdit: (tournamentId: number) => void;
}

export default function TournamentDetail({ tournamentId, onBack, onEdit }: TournamentDetailProps) {
  const [tournament, setTournament] = useState<TournamentDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [activeTab, setActiveTab] = useState("overview");
  const [blinkCopied, setBlinkCopied] = useState(false);

  useEffect(() => {
    loadTournament();
  }, [tournamentId]);

  const copyBlinkUrl = async () => {
    try {
      const baseUrl = apiClient.getBaseUrl();
      const domain = baseUrl.replace('http://', 'https://'); // Enforce https for actions
      const actionUrl = `https://dial.to/?action=solana-action:${domain}/api/actions/tournament/${tournament?.tournament_id}`;
      await navigator.clipboard.writeText(actionUrl);
      setBlinkCopied(true);
      setTimeout(() => setBlinkCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy", err);
    }
  };

  const loadTournament = async () => {
    try {
      setLoading(true);
      const response = await apiClient.getTournament(tournamentId);
      if (response.ok && response.data) {
        setTournament(response.data);
      } else {
        setError(response.error?.message || "Failed to load tournament");
      }
    } catch (err) {
      setError("Network error loading tournament");
    } finally {
      setLoading(false);
    }
  };

  const formatLamports = (lamports: number) => {
    const sol = lamports / 1_000_000_000;
    return sol.toFixed(4) + " SOL";
  };

  const formatTimestamp = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

  const getStatusColor = (status: string) => {
    switch (status.toLowerCase()) {
      case "active": return "var(--primary)";
      case "completed": return "#3b82f6";
      case "scheduled": return "var(--accent)";
      case "registration": return "#4ade80";
      default: return "var(--text-dim)";
    }
  };

  const renderOverview = () => {
    if (!tournament) return null;

    return (
      <div style={{
        display: "grid",
        gridTemplateColumns: "repeat(auto-fit, minmax(300px, 1fr))",
        gap: "1.5rem",
      }}>
        <InfoCard title="TOURNAMENT CORE">
          <DetailRow label="SEQUENCE" value={`#${tournament.tournament_id}`} />
          <DetailRow label="IDENTIFIER" value={tournament.name} />
          <DetailRow label="STATUS" value={tournament.status.toUpperCase()} color={getStatusColor(tournament.status)} />
          <DetailRow label="PROTOCOL" value={tournament.format.toUpperCase()} />
          {tournament.format === "Swiss" && (
            <DetailRow label="SWISS ROUNDS" value={tournament.total_rounds || "N/A"} />
          )}
          {tournament.scheduled_at && (
            <DetailRow label="DEPLOYS AT" value={formatTimestamp(tournament.scheduled_at)} />
          )}
        </InfoCard>

        <InfoCard title="PLAYER LOAD">
          <DetailRow label="CAPACITY" value={`${tournament.players.length} / ${tournament.max_players}`} />
          <DetailRow label="ENTRY FEE" value={formatLamports(tournament.entry_fee_lamports)} />
          {tournament.elo_min && (
            <DetailRow label="ELO RANGE" value={`${tournament.elo_min} - ${tournament.elo_max || "∞"}`} />
          )}
          <DetailRow label="KYC CLEARANCE" value={tournament.kyc_required ? "REQUIRED" : "OPTIONAL"} color={tournament.kyc_required ? "var(--accent)" : "var(--text-dim)"} />
        </InfoCard>

        {tournament.entry_fee_lamports > 0 && (
          <InfoCard title="ECONOMICS">
            <DetailRow label="PLATFORM CUT" value={formatLamports(tournament.platform_fee_lamports || 0)} color="var(--text-dim)" />
            <DetailRow label="TOTAL POOL" value={formatLamports(tournament.prize_pool || 0)} color="var(--accent)" />
            {tournament.prize_shares && (
              <div style={{ marginTop: "1.5rem" }}>
                <div style={{ fontSize: "10px", color: "var(--text-dim)", marginBottom: "10px", fontWeight: "800" }}>REWARD DISTRIBUTION</div>
                {[1, 2, 3, 4].map(place => {
                  const share = tournament.prize_shares![place - 1];
                  if (share === 0) return null;
                  const amount = ((tournament.prize_pool || 0) * share) / 10000;
                  return (
                    <div key={place} style={{ display: "flex", justifyContent: "space-between", fontSize: "12px", marginBottom: "6px" }}>
                      <span style={{ color: "var(--text-dim)" }}>RANK {place}</span>
                      <span style={{ color: "#fff", fontWeight: "700" }}>{formatLamports(amount)}</span>
                    </div>
                  );
                })}
              </div>
            )}
          </InfoCard>
        )}

        <InfoCard title="MARKETING (BLINKS)">
          <div style={{ marginBottom: "1rem", color: "var(--text-dim)", fontSize: "12px", lineHeight: "1.5" }}>
            Share this Blink URL on Twitter or Discord to allow players to register instantly from their wallet.
          </div>
          <div style={{ 
            backgroundColor: "rgba(0,0,0,0.3)", 
            padding: "0.75rem", 
            borderRadius: "8px", 
            fontFamily: "monospace", 
            fontSize: "10px", 
            color: "var(--primary)",
            wordBreak: "break-all",
            marginBottom: "1rem",
            border: "1px solid rgba(255,255,255,0.05)"
          }}>
            https://dial.to/?action=solana-action:{apiClient.getBaseUrl().replace('http://', 'https://')}/api/actions/tournament/{tournament.tournament_id}
          </div>
          <button 
            onClick={copyBlinkUrl}
            style={{
              width: "100%",
              padding: "0.75rem",
              borderRadius: "100px",
              backgroundColor: blinkCopied ? "#4ade80" : "var(--glass)",
              color: blinkCopied ? "#000" : "#fff",
              border: blinkCopied ? "none" : "1px solid var(--border)",
              fontWeight: "bold",
              fontSize: "12px",
              cursor: "pointer",
              transition: "all 0.2s"
            }}
          >
            {blinkCopied ? "COPIED TO CLIPBOARD" : "COPY BLINK URL"}
          </button>
        </InfoCard>
      </div>
    );
  };

  const renderPlayers = () => {
    if (!tournament) return null;

    return (
      <div style={{
        backgroundColor: "var(--surface)",
        borderRadius: "24px",
        overflow: "hidden",
        border: "1px solid var(--border)",
        backdropFilter: "blur(20px)"
      }}>
        <div style={{
          padding: "1.25rem 1.5rem",
          backgroundColor: "rgba(255,255,255,0.05)",
          borderBottom: "1px solid var(--border)",
          fontWeight: "800",
          fontSize: "12px",
          letterSpacing: "1.5px",
          color: "var(--primary)",
        }}>
          MANIFEST: {tournament.players.length} CONNECTED ENTITIES
        </div>
        <div style={{ maxHeight: "500px", overflow: "auto", padding: "0.5rem" }}>
          {tournament.players.map((player, index) => (
            <div
              key={player}
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                padding: "1rem 1.25rem",
                borderRadius: "12px",
                marginBottom: "4px",
                transition: "background-color 0.2s ease"
              }}
              onMouseEnter={(e) => e.currentTarget.style.backgroundColor = "rgba(255,255,255,0.03)"}
              onMouseLeave={(e) => e.currentTarget.style.backgroundColor = "transparent"}
            >
              <div style={{ color: "#ffffff", fontSize: "13px", fontFamily: "'Fira Code', monospace" }}>
                <span style={{ color: "var(--primary)", marginRight: "1rem", opacity: 0.5 }}>
                  {String(index + 1).padStart(2, '0')}
                </span>
                {player}
              </div>
              {tournament.player_elos && tournament.player_elos[index] && (
                <div style={{ 
                    color: "var(--accent)", 
                    fontSize: "11px", 
                    fontWeight: "bold",
                    backgroundColor: "rgba(244, 187, 68, 0.1)",
                    padding: "2px 8px",
                    borderRadius: "4px"
                }}>
                  ELO: {tournament.player_elos[index]}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    );
  };

  const renderMatches = () => {
    return (
        <div style={{ padding: "4rem", textAlign: "center", color: "var(--text-dim)", border: "1px dashed var(--border)", borderRadius: "24px" }}>
            BRACKET VISUALIZER OFFLINE. WAIT FOR TOURNAMENT INITIALIZATION.
        </div>
    );
  };

  if (loading) return <div style={{ textAlign: "center", padding: "4rem", color: "var(--text-dim)" }}>DECRYPTING DATA...</div>;

  return (
    <div style={{ width: "100%" }}>
      {/* Header */}
      <div style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        marginBottom: "2.5rem",
      }}>
        <div style={{ display: "flex", alignItems: "center", gap: "1.5rem" }}>
          <button
            onClick={onBack}
            style={{
              padding: "0.6rem 1.25rem",
              backgroundColor: "var(--glass)",
              color: "var(--text-dim)",
              border: "1px solid var(--border)",
              borderRadius: "100px",
              cursor: "pointer",
              fontSize: "12px",
              fontWeight: "700"
            }}
          >
            ← RETURN
          </button>
          <h2 style={{ 
            color: "#fff", 
            margin: 0,
            fontSize: "28px",
            fontWeight: "900"
          }}>
            {tournament.name}
          </h2>
        </div>
        
        <div style={{ display: "flex", gap: "1rem" }}>
          <button
            onClick={() => onEdit(tournament!.tournament_id)}
            className="primary"
            style={{
              padding: "0.75rem 2rem",
              borderRadius: "100px",
            }}
          >
            MODIFY CONFIG
          </button>
        </div>
      </div>

      {/* Nav Tabs */}
      <div style={{ marginBottom: "2rem" }}>
        <div style={{
          display: "flex",
          gap: "8px",
          borderBottom: "1px solid var(--border)",
          paddingBottom: "1px"
        }}>
          {["overview", "players", "matches"].map(tab => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              style={{
                padding: "1rem 2rem",
                backgroundColor: "transparent",
                border: "none",
                borderBottom: activeTab === tab ? "3px solid var(--primary)" : "3px solid transparent",
                color: activeTab === tab ? "var(--primary)" : "var(--text-dim)",
                cursor: "pointer",
                fontSize: "11px",
                fontWeight: "800",
                letterSpacing: "1.5px",
                transition: "all 0.2s ease",
                borderRadius: 0
              }}
            >
                {tab.toUpperCase()}
            </button>
          ))}
        </div>
      </div>

      {/* Tab Content */}
      <div style={{ animation: "fadeIn 0.4s ease" }}>
        {activeTab === "overview" && renderOverview()}
        {activeTab === "players" && renderPlayers()}
        {activeTab === "matches" && renderMatches()}
      </div>
    </div>
  );
}

// Utility Components
const InfoCard = ({ title, children }: { title: string, children: React.ReactNode }) => (
  <div style={{
    backgroundColor: "var(--surface)",
    padding: "2rem",
    borderRadius: "24px",
    border: "1px solid var(--border)",
    backdropFilter: "blur(20px)",
    boxShadow: "0 10px 40px rgba(0,0,0,0.3)"
  }}>
    <h4 style={{ color: "var(--primary)", fontSize: "11px", fontWeight: "800", letterSpacing: "2px", margin: "0 0 1.5rem 0" }}>{title}</h4>
    {children}
  </div>
);

const DetailRow = ({ label, value, color }: { label: string, value: any, color?: string }) => (
  <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "12px", alignItems: "baseline" }}>
    <span style={{ color: "var(--text-dim)", fontSize: "11px", fontWeight: "700" }}>{label}</span>
    <span style={{ color: color || "#fff", fontSize: "14px", fontWeight: "800" }}>{value}</span>
  </div>
);
