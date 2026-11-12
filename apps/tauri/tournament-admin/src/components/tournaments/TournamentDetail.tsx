import { useState } from "react";
import { duplicateConfig } from "../../services/adminFlows";
import CreateTournament from "./CreateTournament";
import TransactionDebugger from "../TransactionDebugger";
import LifecyclePanel from "./detail/LifecyclePanel";
import MatchesTab from "./detail/MatchesTab";
import OverviewTab from "./detail/OverviewTab";
import PlayersTab from "./detail/PlayersTab";
import { ghostButtonStyle } from "./detail/ui";
import { useTournamentData } from "./detail/useTournamentData";

const TABS = ["overview", "players", "matches"] as const;
type Tab = (typeof TABS)[number];

export default function TournamentDetail({ tournamentId, onBack, onOpenTournament }: {
  tournamentId: number;
  onBack: () => void;
  /** Called with a newly created tournament (e.g. a duplicate). */
  onOpenTournament: (tournamentId: number) => void;
}) {
  const data = useTournamentData(tournamentId);
  const { tournament, loading, loadErrors, reload } = data;
  const [tab, setTab] = useState<Tab>("overview");
  const [duplicating, setDuplicating] = useState(false);
  const [analyzeSignature, setAnalyzeSignature] = useState<string | null>(null);

  if (duplicating && tournament) {
    return <CreateTournament initialConfig={duplicateConfig(tournament)} onCancel={() => setDuplicating(false)} onTournamentCreated={onOpenTournament} />;
  }
  if (loading && !tournament) return <div style={{ padding: "2rem", color: "var(--text-dim)" }}>Loading tournament…</div>;
  if (!tournament) {
    return <div role="alert" style={{ padding: "2rem" }}>
      Could not load tournament #{tournamentId}.{" "}
      <button onClick={() => void reload()} style={ghostButtonStyle}>RETRY</button>{" "}
      <button onClick={onBack} style={ghostButtonStyle}>BACK</button>
    </div>;
  }

  return (
    <div style={{ width: "100%" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1.5rem" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "1.5rem" }}>
          <button onClick={onBack} style={{ ...ghostButtonStyle, borderRadius: "100px" }}>← ALL TOURNAMENTS</button>
          <h2 style={{ color: "#fff", margin: 0, fontSize: "28px", fontWeight: 900 }}>{tournament.name}</h2>
        </div>
        <button onClick={() => setDuplicating(true)} className="primary" style={{ padding: "0.75rem 2rem", borderRadius: "100px" }}>DUPLICATE</button>
      </div>

      <LifecyclePanel
        tournament={tournament}
        onChainPrizeLamports={data.reconciliation?.on_chain_prize_pool_lamports}
        onChanged={reload}
      />

      <div role="tablist" style={{ display: "flex", gap: "8px", borderBottom: "1px solid var(--border)", marginBottom: "2rem" }}>
        {TABS.map(t => (
          <button key={t} role="tab" aria-selected={tab === t} onClick={() => setTab(t)}
            style={{ padding: "1rem 2rem", backgroundColor: "transparent", border: "none", borderBottom: tab === t ? "3px solid var(--primary)" : "3px solid transparent", color: tab === t ? "var(--primary)" : "var(--text-dim)", cursor: "pointer", fontSize: "11px", fontWeight: 800, letterSpacing: "1.5px" }}>
            {t.toUpperCase()}
          </button>
        ))}
      </div>

      {Object.entries(loadErrors).filter(([, m]) => m).map(([name, m]) => <p role="alert" key={name} style={{ color: "#f87171", fontSize: "12px" }}>{m}</p>)}
      {analyzeSignature && (
        <div style={{ marginBottom: "1.5rem" }}>
          <button onClick={() => setAnalyzeSignature(null)} style={ghostButtonStyle}>CLOSE ANALYSIS</button>
          <TransactionDebugger key={analyzeSignature} initialSignature={analyzeSignature} />
        </div>
      )}

      {tab === "overview" && (
        <OverviewTab tournament={tournament} escrowSol={data.escrowSol} reconciliation={data.reconciliation}
          auditEntries={data.auditEntries} operations={data.operations} transactions={data.transactions}
          onAnalyze={setAnalyzeSignature} />
      )}
      {tab === "players" && <PlayersTab tournament={tournament} onChanged={reload} />}
      {tab === "matches" && <MatchesTab tournament={tournament} bracket={data.bracket} onChanged={reload} />}
    </div>
  );
}
