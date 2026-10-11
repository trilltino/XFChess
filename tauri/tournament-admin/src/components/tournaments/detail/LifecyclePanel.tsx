import { useState } from "react";
import { apiClient, type TournamentDetail } from "../../../services/api";
import { cancellationState } from "../../../services/adminFlows";
import { nextStep } from "../../../services/lifecycle";
import { ghostButtonStyle, inputStyle, Message, primaryButtonStyle } from "./ui";

const toneColor = { action: "var(--primary)", waiting: "var(--accent)", done: "#4ade80", problem: "#f87171" } as const;

/**
 * The one card that tells the operator what happens next and holds the button for
 * it, so the normal flow never needs the emergency controls further down.
 */
export default function LifecyclePanel({ tournament, onChainPrizeLamports, onChanged }: {
  tournament: TournamentDetail;
  onChainPrizeLamports?: number | null;
  onChanged: () => Promise<void>;
}) {
  const step = nextStep(tournament, { onChainPrizeLamports });
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [prizeSol, setPrizeSol] = useState("");
  const [copied, setCopied] = useState(false);
  const id = tournament.tournament_id;
  const status = tournament.status.toLowerCase();

  const run = async (label: string, call: () => Promise<{ ok: boolean; error?: { message?: string; status?: number } }>, done: string) => {
    setBusy(true);
    setMessage(null);
    try {
      const r = await call();
      setMessage(r.ok ? done : `Error: ${r.error?.message || r.error?.status || `${label} failed`}`);
      if (r.ok) await onChanged();
    } finally {
      setBusy(false);
    }
  };

  const fundPrize = () => {
    const sol = parseFloat(prizeSol);
    if (!Number.isFinite(sol) || sol <= 0) { setMessage("Error: enter a prize in SOL."); return; }
    if (!window.confirm(`Lock ${sol} SOL as the guaranteed on-chain prize?`)) return;
    void run("Funding", () => apiClient.fundTournamentPrize(id, Math.round(sol * 1e9)), `Locked ${sol} SOL — registration is open.`);
  };

  const start = () => {
    if (!window.confirm(`Start "${tournament.name}" now with ${tournament.players.length} players?`)) return;
    void run("Start", () => apiClient.startTournament(id), "Start submitted — confirming on-chain.");
  };

  const approve = () => {
    if (!window.confirm(`Approve prize release for "${tournament.name}"?`)) return;
    void run("Approval", () => apiClient.approvePrizeRelease(id), "Prize release approved — payout runs automatically.");
  };

  const cancel = async () => {
    if (!window.confirm(`Cancel "${tournament.name}"? Entry fees are refunded and the guaranteed prize returns to the operator.`)) return;
    setBusy(true);
    setMessage(null);
    const r = await apiClient.cancelTournament(id);
    setMessage(r.ok ? cancellationState(r.data?.status).label : `Error: ${r.error?.message || "Cancellation failed"}`);
    setBusy(false);
    await onChanged();
  };

  const blinkUrl = `https://dial.to/?action=solana-action:${apiClient.getBaseUrl().replace("http://", "https://")}/api/actions/tournament/${id}`;
  const copyBlink = async () => {
    await navigator.clipboard.writeText(blinkUrl).catch(() => {});
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div style={{ marginBottom: "2rem", padding: "1.5rem", borderRadius: "20px", background: "var(--surface)", border: `1px solid ${toneColor[step.tone]}` }}>
      <div style={{ fontSize: "10px", fontWeight: 800, letterSpacing: "2px", color: toneColor[step.tone] }}>NEXT STEP</div>
      <div style={{ fontSize: "20px", fontWeight: 900, color: "#fff", margin: "6px 0" }}>{step.title}</div>
      <div style={{ fontSize: "13px", color: "var(--text-dim)", marginBottom: "1rem" }}>{step.detail}</div>

      <div style={{ display: "flex", gap: "8px", flexWrap: "wrap", alignItems: "center" }}>
        {step.action === "fund-prize" && <>
          <input type="number" step="0.0001" min="0" value={prizeSol} onChange={e => setPrizeSol(e.target.value)}
            placeholder="Guaranteed prize (SOL)" aria-label="Guaranteed prize in SOL" style={{ ...inputStyle, width: "200px" }} />
          <button disabled={busy || !prizeSol} onClick={fundPrize} style={primaryButtonStyle}>{busy ? "LOCKING…" : "FUND PRIZE"}</button>
        </>}
        {step.action === "start" && <button disabled={busy} onClick={start} style={primaryButtonStyle}>{busy ? "STARTING…" : "START NOW"}</button>}
        {step.action === "approve-release" && <button disabled={busy} onClick={approve} style={primaryButtonStyle}>{busy ? "APPROVING…" : "APPROVE PRIZE RELEASE"}</button>}
        {status === "registration" && (
          <button onClick={copyBlink} style={ghostButtonStyle}>{copied ? "BLINK COPIED" : "COPY REGISTRATION BLINK"}</button>
        )}
        {(status === "registration" || status === "active") && (
          <button disabled={busy} onClick={cancel} style={{ ...ghostButtonStyle, color: "#f87171" }}>CANCEL & REFUND</button>
        )}
      </div>
      <Message text={message} />
    </div>
  );
}
