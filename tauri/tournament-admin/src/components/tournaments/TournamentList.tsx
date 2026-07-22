import { useState, useEffect } from "react";
import { apiClient, type TournamentSummary } from "../../services/api";
import { lamportsToUsd } from "../../services/sol";
import { useSolUsdRate } from "../../hooks/useSolUsdRate";

interface TournamentListProps {
  onTournamentSelect: (tournamentId: number) => void;
}

const CANCELLABLE_STATUSES = ["registration", "scheduled", "active"];
const DELETABLE_STATUSES = ["cancelled", "completed"];

export default function TournamentList({ onTournamentSelect }: TournamentListProps) {
  const [tournaments, setTournaments] = useState<TournamentSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState("all");
  const [searchTerm, setSearchTerm] = useState("");
  const [cancellingId, setCancellingId] = useState<number | null>(null);
  const [deletingId, setDeletingId] = useState<number | null>(null);
  const solUsdRate = useSolUsdRate();

  useEffect(() => {
    loadTournaments();
  }, []);

  const loadTournaments = async () => {
    try {
      setLoading(true);
      const response = await apiClient.getTournaments();
      if (response.ok && response.data) {
        setTournaments(response.data);
      } else {
        console.error(response.error?.message || "Failed to load tournaments");
      }
    } catch (err) {
      console.error("Network error loading tournaments", err);
    } finally {
      setLoading(false);
    }
  };

  const filteredTournaments = tournaments.filter(tournament => {
    const matchesFilter = filter === "all" || tournament.status.toLowerCase().includes(filter.toLowerCase());
    const matchesSearch = tournament.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
                         tournament.tournament_id.toString().includes(searchTerm);
    return matchesFilter && matchesSearch;
  });

  const formatPrizePool = (lamports: number) => {
    const usd = lamportsToUsd(lamports, solUsdRate);
    if (usd != null) return "$" + usd.toFixed(2);
    const sol = lamports / 1_000_000_000;
    return sol.toFixed(4) + " SOL";
  };

  const formatEntryFee = (lamports: number) => {
    if (lamports === 0) return "FREE";
    const usd = lamportsToUsd(lamports, solUsdRate);
    if (usd != null) return "$" + usd.toFixed(2);
    const sol = lamports / 1_000_000_000;
    return sol.toFixed(4) + " SOL";
  };

  const handleCancel = async (e: React.MouseEvent, tournament: TournamentSummary) => {
    e.stopPropagation();
    const confirmed = window.confirm(
      `Cancel "${tournament.name}" (#${tournament.tournament_id})?\n\n` +
      `This submits an on-chain cancel_tournament transaction: entry fees are refunded to all ${tournament.registered} registered player(s) and the guaranteed prize pool is returned to the operator. This cannot be undone.`
    );
    if (!confirmed) return;
    setCancellingId(tournament.tournament_id);
    const r = await apiClient.cancelTournament(tournament.tournament_id);
    setCancellingId(null);
    if (r.ok) {
      await loadTournaments();
    } else {
      alert(`Failed to cancel: ${r.error?.message || "Unknown error"}`);
    }
  };

  // Local housekeeping only — removes a Cancelled/Completed tournament from
  // this list. Does not touch on-chain state (nothing left to manage once a
  // tournament is terminal); the backend rejects this for any other status.
  const handleDelete = async (e: React.MouseEvent, tournament: TournamentSummary) => {
    e.stopPropagation();
    const confirmed = window.confirm(`Remove "${tournament.name}" (#${tournament.tournament_id}) from this list? This only clears it from the admin panel — it doesn't touch on-chain state.`);
    if (!confirmed) return;
    setDeletingId(tournament.tournament_id);
    const r = await apiClient.deleteTournament(tournament.tournament_id);
    setDeletingId(null);
    if (r.ok) {
      setTournaments(prev => prev.filter(t => t.tournament_id !== tournament.tournament_id));
    } else {
      alert(`Failed to remove: ${r.error?.message || "Unknown error"}`);
    }
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

  if (loading) {
    return (
      <div style={{
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
        height: "200px",
        color: "var(--text-dim)",
      }}>
        Loading tournaments...
      </div>
    );
  }

  return (
    <div style={{ width: "100%", padding: "1rem" }}>
      {/* Filters and Search */}
      <div style={{
        display: "flex",
        gap: "1rem",
        marginBottom: "2.5rem",
        flexWrap: "wrap",
        alignItems: "center"
      }}>
        <div style={{ flex: 1, minWidth: "300px" }}>
          <input
            type="text"
            placeholder="Search tournaments by name or ID..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            style={{
              width: "100%",
              padding: "0.85rem 1.25rem",
              backgroundColor: "rgba(255, 255, 255, 0.05)",
              border: "1px solid var(--border)",
              borderRadius: "100px",
              color: "#ffffff",
              fontSize: "14px",
              outline: "none",
              backdropFilter: "blur(10px)",
              transition: "border-color 0.2s ease"
            }}
          />
        </div>
        
        <select
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{
            padding: "0.85rem 1.5rem",
            backgroundColor: "rgba(255, 255, 255, 0.05)",
            border: "1px solid var(--border)",
            borderRadius: "100px",
            color: "#ffffff",
            fontSize: "14px",
            outline: "none",
            cursor: "pointer",
            backdropFilter: "blur(10px)"
          }}
        >
          <option value="all">All Status</option>
          <option value="registration">Registration</option>
          <option value="scheduled">Scheduled</option>
          <option value="active">Active</option>
          <option value="completed">Completed</option>
        </select>

        <button
          onClick={loadTournaments}
          className="primary"
          style={{
            padding: "0.85rem 2rem",
            borderRadius: "100px",
            fontSize: "14px",
            boxShadow: "0 4px 15px rgba(173, 92, 47, 0.3)"
          }}
        >
          REFRESH
        </button>
      </div>

      {/* Stats Summary Bar */}
      <div style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        marginBottom: "1.5rem",
        padding: "0 0.5rem"
      }}>
        <div style={{ color: "var(--text-dim)", fontSize: "12px", letterSpacing: "1px", fontWeight: "700" }}>
          TOURNAMENT ARCHIVE <span style={{ color: "var(--primary)", marginLeft: "8px" }}>[{filteredTournaments.length} UNITS]</span>
        </div>
      </div>

      {/* Tournament Grid (Cards instead of Table) */}
      <div style={{
        display: "grid",
        gridTemplateColumns: "repeat(auto-fill, minmax(320px, 1fr))",
        gap: "1.5rem"
      }}>
        {filteredTournaments.length === 0 ? (
          <div style={{
            gridColumn: "1 / -1",
            padding: "4rem",
            textAlign: "center",
            backgroundColor: "rgba(255,255,255,0.02)",
            borderRadius: "24px",
            border: "1px dashed var(--border)",
            color: "var(--text-dim)"
          }}>
            No data nodes found in current sector.
          </div>
        ) : (
          filteredTournaments.map((tournament) => (
            <div
              key={tournament.tournament_id}
              onClick={() => onTournamentSelect(tournament.tournament_id)}
              style={{
                backgroundColor: "var(--surface)",
                padding: "1.5rem",
                borderRadius: "24px",
                border: "1px solid var(--border)",
                cursor: "pointer",
                transition: "all 0.3s cubic-bezier(0.16, 1, 0.3, 1)",
                position: "relative",
                overflow: "hidden",
                backdropFilter: "blur(10px)"
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.borderColor = "var(--primary)";
                e.currentTarget.style.transform = "translateY(-4px)";
                e.currentTarget.style.backgroundColor = "rgba(255,255,255,0.05)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.borderColor = "var(--border)";
                e.currentTarget.style.transform = "translateY(0)";
                e.currentTarget.style.backgroundColor = "var(--surface)";
              }}
            >
              <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "1rem" }}>
                <div style={{ color: "var(--primary)", fontSize: "12px", fontWeight: "800", letterSpacing: "1px" }}>
                  ID #{tournament.tournament_id}
                </div>
                <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
                  {CANCELLABLE_STATUSES.includes(tournament.status.toLowerCase()) && (
                    <button
                      onClick={(e) => handleCancel(e, tournament)}
                      disabled={cancellingId === tournament.tournament_id}
                      style={{
                        fontSize: "10px",
                        fontWeight: "800",
                        letterSpacing: "1px",
                        padding: "4px 10px",
                        borderRadius: "100px",
                        background: "rgba(239,68,68,0.1)",
                        color: "#f87171",
                        border: "1px solid rgba(239,68,68,0.3)",
                        cursor: cancellingId === tournament.tournament_id ? "default" : "pointer",
                        opacity: cancellingId === tournament.tournament_id ? 0.5 : 1,
                      }}
                    >
                      {cancellingId === tournament.tournament_id ? "CANCELLING…" : "CANCEL"}
                    </button>
                  )}
                  <div style={{
                    color: getStatusColor(tournament.status),
                    fontSize: "10px",
                    fontWeight: "900",
                    letterSpacing: "1px",
                    padding: "4px 10px",
                    borderRadius: "100px",
                    background: "rgba(255,255,255,0.05)",
                    border: `1px solid ${getStatusColor(tournament.status)}44`
                  }}>
                    {tournament.status.toUpperCase()}
                  </div>
                  {DELETABLE_STATUSES.includes(tournament.status.toLowerCase()) && (
                    <button
                      onClick={(e) => handleDelete(e, tournament)}
                      disabled={deletingId === tournament.tournament_id}
                      title="Remove from this list (local only, doesn't touch on-chain state)"
                      style={{
                        width: "22px",
                        height: "22px",
                        borderRadius: "50%",
                        background: "rgba(255,255,255,0.06)",
                        color: "var(--text-dim)",
                        border: "1px solid var(--border)",
                        cursor: deletingId === tournament.tournament_id ? "default" : "pointer",
                        opacity: deletingId === tournament.tournament_id ? 0.5 : 1,
                        fontSize: "12px",
                        lineHeight: 1,
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        padding: 0,
                      }}
                    >
                      ✕
                    </button>
                  )}
                </div>
              </div>

              <h3 style={{ fontSize: "18px", color: "#fff", marginBottom: "0.5rem", fontWeight: "800" }}>
                {tournament.name}
              </h3>

              <div style={{ display: "flex", gap: "1rem", marginBottom: "1.5rem" }}>
                <div style={{ flex: 1 }}>
                  <div style={{ fontSize: "10px", color: "var(--text-dim)", marginBottom: "4px" }}>PRIZE POOL</div>
                  <div style={{ fontSize: "14px", fontWeight: "700", color: "var(--accent)" }}>{formatPrizePool(tournament.prize_pool)}</div>
                </div>
                <div style={{ flex: 1 }}>
                  <div style={{ fontSize: "10px", color: "var(--text-dim)", marginBottom: "4px" }}>ENTRY FEE</div>
                  <div style={{ fontSize: "14px", fontWeight: "700", color: "#fff" }}>{formatEntryFee(tournament.entry_fee_lamports)}</div>
                </div>
                {(tournament.platform_fee_lamports ?? 0) > 0 && (
                  <div style={{ flex: 1 }}>
                    <div style={{ fontSize: "10px", color: "var(--text-dim)", marginBottom: "4px" }}>PLATFORM FEE</div>
                    <div style={{ fontSize: "14px", fontWeight: "700", color: "var(--text-dim)" }}>{formatEntryFee(tournament.platform_fee_lamports!)}</div>
                  </div>
                )}
              </div>

              <div style={{ width: "100%", height: "4px", backgroundColor: "rgba(0,0,0,0.2)", borderRadius: "100px", marginBottom: "8px", overflow: "hidden" }}>
                <div style={{ 
                  width: `${(tournament.registered / tournament.max_players) * 100}%`, 
                  height: "100%", 
                  backgroundColor: "var(--primary)", 
                  borderRadius: "100px" 
                }} />
              </div>
              
              <div style={{ display: "flex", justifyContent: "space-between", fontSize: "11px", color: "var(--text-dim)" }}>
                <span>REGISTRATION LOAD</span>
                <span>{tournament.registered} / {tournament.max_players} PLAYERS</span>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
