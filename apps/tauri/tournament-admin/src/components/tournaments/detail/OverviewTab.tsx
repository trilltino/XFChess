import { useEffect, useState } from "react";
import { apiClient, type TournamentDetail } from "../../../services/api";
import { cancellationState, type TournamentOperation } from "../../../services/adminFlows";
import { fmtSol, fmtTs, ghostButtonStyle, InfoCard, inputStyle, Message, primaryButtonStyle, Row, statusColor } from "./ui";
import type { Reconciliation } from "./useTournamentData";

interface Template { name: string; data: unknown; created_at: number; updated_at: number }

export default function OverviewTab({ tournament, escrowSol, reconciliation, auditEntries, operations, transactions, onAnalyze }: {
  tournament: TournamentDetail;
  escrowSol: number | null;
  reconciliation: Reconciliation | null;
  auditEntries: any[];
  operations: TournamentOperation[];
  transactions: any[];
  onAnalyze: (signature: string) => void;
}) {
  const prizePool = reconciliation?.on_chain_prize_pool_lamports ?? tournament.prize_pool ?? 0;
  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(300px, 1fr))", gap: "1.5rem" }}>
      <InfoCard title="TOURNAMENT">
        <Row label="ID" value={`#${tournament.tournament_id}`} />
        <Row label="STATUS" value={tournament.status.toUpperCase()} color={statusColor(tournament.status)} />
        <Row label="FORMAT" value={tournament.format === "Swiss" ? `SWISS · ${tournament.total_rounds ?? "?"} ROUNDS` : "SINGLE ELIMINATION"} />
        {tournament.scheduled_at ? <Row label="STARTS" value={fmtTs(tournament.scheduled_at)} /> : null}
        {tournament.is_private && <Row label="ACCESS" value="PRIVATE (PASSWORD)" color="var(--accent)" />}
      </InfoCard>

      <InfoCard title="PLAYERS">
        <Row label="REGISTERED" value={`${tournament.players.length} / ${tournament.max_players}`} />
        {reconciliation && (
          <Row label="CHAIN SYNC"
            value={`${reconciliation.on_chain_player_count} on-chain / ${reconciliation.local_player_count} local`}
            color={reconciliation.reconciliation_status === "in_sync" ? "#4ade80" : "#f87171"} />
        )}
        <Row label="ENTRY FEE" value={tournament.entry_fee_lamports > 0 ? fmtSol(tournament.entry_fee_lamports) : "FREE"} />
        {tournament.elo_min ? <Row label="ELO RANGE" value={`${tournament.elo_min} – ${tournament.elo_max || "∞"}`} /> : null}
        <Row label="KYC" value={tournament.kyc_required ? "REQUIRED" : "OPTIONAL"} color={tournament.kyc_required ? "var(--accent)" : "var(--text-dim)"} />
      </InfoCard>

      <InfoCard title="PRIZES">
        <Row label="GUARANTEED PRIZE" value={prizePool > 0 ? fmtSol(prizePool) : "NONE"} color={prizePool > 0 ? "#4ade80" : "var(--text-dim)"} />
        {tournament.entry_fee_lamports > 0 && <Row label="PLATFORM CUT" value={fmtSol(tournament.platform_fee_lamports || 0)} color="var(--text-dim)" />}
        <Row label="ESCROW BALANCE" value={escrowSol != null ? `${escrowSol.toFixed(4)} SOL` : "—"} />
        {prizePool > 0 && tournament.prize_shares.map((share, i) => share ? (
          <Row key={i} label={`#${i + 1} · ${share / 100}%`} value={fmtSol((prizePool * share) / 10000)} />
        ) : null)}
      </InfoCard>

      <InfoCard title="ADMIN AUDIT">
        {auditEntries.length === 0
          ? <div style={{ color: "var(--text-dim)", fontSize: "12px" }}>No recorded admin actions.</div>
          : auditEntries.slice(0, 8).map((entry, index) => (
            <div key={index} style={{ borderBottom: "1px solid var(--border)", padding: "6px 0", fontSize: "11px" }}>
              <strong>{entry.action}</strong> · {entry.result}
              <div style={{ color: "var(--text-dim)" }}>{fmtTs(entry.timestamp)}</div>
            </div>
          ))}
      </InfoCard>

      <InfoCard title="ON-CHAIN ACTIVITY">
        {operations.map(operation => (
          <div key={operation.id} style={{ padding: "8px 0", overflowWrap: "anywhere", fontSize: "11px" }}>
            <strong>{cancellationState(operation.status).label}</strong>
            <div style={{ color: "var(--text-dim)" }}>{operation.actor} · {operation.reason || "no reason"} · {fmtTs(operation.updated_at)}</div>
            {operation.last_error && <p role="alert" style={{ color: "#f87171" }}>{operation.last_error}</p>}
            {operation.signature && <button onClick={() => onAnalyze(operation.signature!)} style={ghostButtonStyle}>ANALYZE TX</button>}
          </div>
        ))}
        {transactions.length === 0 && operations.length === 0
          ? <div style={{ color: "var(--text-dim)", fontSize: "12px" }}>No tournament transactions yet.</div>
          : transactions.slice(0, 8).map((tx, index) => (
            <div key={index} style={{ borderBottom: "1px solid var(--border)", padding: "6px 0", fontSize: "11px" }}>
              <strong>{tx.operation}</strong> · {tx.status} · retries {tx.retry_count}
              <div style={{ color: "var(--text-dim)", fontFamily: "monospace", overflowWrap: "anywhere" }}>{tx.signature}</div>
              {tx.signature && <button onClick={() => onAnalyze(tx.signature)} style={ghostButtonStyle}>ANALYZE TX</button>}
            </div>
          ))}
      </InfoCard>

      <TemplateCard tournament={tournament} />
    </div>
  );
}

function TemplateCard({ tournament }: { tournament: TournamentDetail }) {
  const [name, setName] = useState("");
  const [templates, setTemplates] = useState<Template[]>([]);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const load = async () => {
    const r = await apiClient.listTemplates();
    if (r.ok) setTemplates(r.data?.templates ?? []);
  };
  useEffect(() => { void load(); }, []);

  const save = async () => {
    setSaving(true);
    setMessage(null);
    const r = await apiClient.saveTemplate(name, tournament);
    setMessage(r.ok ? `Saved "${name}".` : `Error: ${r.error?.message || r.error?.status}`);
    if (r.ok) { setName(""); await load(); }
    setSaving(false);
  };

  const remove = async (templateName: string) => {
    if (!window.confirm(`Delete template "${templateName}"?`)) return;
    const r = await apiClient.deleteTemplate(templateName);
    if (r.ok) await load();
    else setMessage(`Error: ${r.error?.message || r.error?.status}`);
  };

  return (
    <InfoCard title="SAVE AS TEMPLATE">
      <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: "0 0 12px" }}>Reusable preset, stored on the backend and shared across machines.</p>
      <div style={{ display: "flex", gap: "8px" }}>
        <input value={name} onChange={e => setName(e.target.value)} placeholder="Template name…" aria-label="Template name" style={{ ...inputStyle, flex: 1 }} />
        <button onClick={save} disabled={saving || !name} style={primaryButtonStyle}>{saving ? "SAVING…" : "SAVE"}</button>
      </div>
      <Message text={message} />
      {templates.map(t => (
        <div key={t.name} style={{ display: "flex", justifyContent: "space-between", padding: "4px 0", fontSize: "11px", color: "var(--text-dim)" }}>
          <span>{t.name}</span>
          <button onClick={() => remove(t.name)} aria-label={`Delete template ${t.name}`} style={{ background: "transparent", border: "none", color: "#f87171", cursor: "pointer" }}>✕</button>
        </div>
      ))}
    </InfoCard>
  );
}
