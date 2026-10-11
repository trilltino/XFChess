import { useEffect, useState } from "react";
import { apiClient } from "../../services/api";
import {
  cancelOfflineTournament,
  completeOfflineTournament,
  loadOfflineTournament,
  publishOfflineTournament,
  recordOfflineMatch,
  recordOfflineSwissResult,
  startOfflineTournament,
  type OfflineTournament,
} from "../../services/offlineTournament";
import { buttonStyle, cardStyle, inputStyle } from "./offlineStyles";

type SwissOutcome = "white" | "black" | "draw";

/**
 * Manages one off-chain event. Every change is a pure transition from
 * `offlineTournament.ts`, saved with the stored revision + 1 so a concurrent edit
 * from another admin is rejected (409) instead of overwritten.
 */
export default function OfflineEventDetail({ tournamentId, onBack }: { tournamentId: string; onBack: () => void }) {
  const [event, setEvent] = useState<OfflineTournament | null>(null);
  const [revision, setRevision] = useState(0);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [pair, setPair] = useState<{ white: string; black: string; result: SwissOutcome }>({ white: "", black: "", result: "white" });

  const load = async () => {
    const result = await apiClient.getOfflineTournament(tournamentId);
    if (!result.ok || !result.data) {
      setMessage(`Error: ${result.error?.message ?? "Event not found"}`);
      return;
    }
    try {
      setEvent(loadOfflineTournament(result.data.state));
      setRevision(result.data.revision);
    } catch (error) {
      setMessage(`Error: ${error instanceof Error ? error.message : "Unreadable event"}`);
    }
  };
  useEffect(() => { void load(); }, [tournamentId]);

  const apply = async (transition: (t: OfflineTournament) => OfflineTournament, done: string) => {
    if (!event) return;
    let next: OfflineTournament;
    try {
      next = transition(event);
    } catch (error) {
      setMessage(`Error: ${error instanceof Error ? error.message : "Invalid change"}`);
      return;
    }
    setBusy(true);
    setMessage(null);
    const result = await apiClient.updateOfflineTournament(tournamentId, { status: next.status, state: next, revision: revision + 1 });
    setBusy(false);
    if (!result.ok) {
      setMessage(result.error?.status === 409
        ? "Error: someone else changed this event — reloaded, please retry."
        : `Error: ${result.error?.message ?? "Save failed"}`);
      await load();
      return;
    }
    setMessage(done);
    await load();
  };

  if (!event) {
    return <div style={{ padding: "1rem" }}>
      <button onClick={onBack} style={buttonStyle}>← BACK</button>
      <p role={message ? "alert" : undefined}>{message ?? "Loading…"}</p>
    </div>;
  }

  const active = event.status === "active";
  const identities = event.participants.map(p => p.identity);

  return <div style={{ padding: "1rem", maxWidth: "1100px", display: "grid", gap: "1rem" }}>
    <div style={cardStyle}>
      <div>
        <button onClick={onBack} style={{ ...buttonStyle, marginRight: "10px" }}>← BACK</button>
        <strong>{event.name}</strong>
        <span style={{ color: "var(--text-dim)", fontSize: "11px", marginLeft: "8px" }}>{event.format} · {event.status} · revision {revision}</span>
      </div>
      <div style={{ display: "flex", gap: "6px" }}>
        {event.status === "draft" && <button disabled={busy} onClick={() => apply(publishOfflineTournament, "Published.")} style={buttonStyle}>PUBLISH</button>}
        {(event.status === "draft" || event.status === "published") && <button disabled={busy} onClick={() => apply(startOfflineTournament, "Started.")} style={buttonStyle}>START</button>}
        {active && event.format === "swiss" && <button disabled={busy} onClick={() => apply(completeOfflineTournament, "Completed.")} style={buttonStyle}>COMPLETE</button>}
        {event.status !== "completed" && event.status !== "cancelled" && (
          <button disabled={busy} onClick={() => window.confirm("Cancel this event?") && apply(cancelOfflineTournament, "Cancelled.")} style={{ ...buttonStyle, color: "#f87171" }}>CANCEL</button>
        )}
      </div>
    </div>
    {message && <div role={message.startsWith("Error") ? "alert" : "status"} style={{ color: message.startsWith("Error") ? "#f87171" : "#4ade80", fontSize: "12px" }}>{message}</div>}

    {event.format === "single_elimination" ? (
      <div style={{ display: "grid", gap: "8px" }}>
        {event.matches.map(match => (
          <div key={match.index} style={cardStyle}>
            <span style={{ fontSize: "12px" }}>
              R{match.round} · B{match.board}: {match.white ?? "TBD"} vs {match.black ?? (match.round === 1 ? "BYE" : "TBD")}
              {match.result && <strong style={{ color: "#4ade80", marginLeft: "8px" }}>→ {match.result === "white" ? match.white : match.black}</strong>}
            </span>
            {active && !match.result && match.white && match.black && (
              <div style={{ display: "flex", gap: "6px" }}>
                <button disabled={busy} onClick={() => apply(t => recordOfflineMatch(t, match.index, "white"), "Result recorded.")} style={buttonStyle}>WHITE WINS</button>
                <button disabled={busy} onClick={() => apply(t => recordOfflineMatch(t, match.index, "black"), "Result recorded.")} style={buttonStyle}>BLACK WINS</button>
              </div>
            )}
          </div>
        ))}
      </div>
    ) : (
      <div style={{ display: "grid", gap: "10px" }}>
        {active && (
          <div style={{ ...cardStyle, justifyContent: "flex-start", flexWrap: "wrap" }}>
            <select aria-label="White" value={pair.white} onChange={e => setPair(p => ({ ...p, white: e.target.value }))} style={inputStyle}>
              <option value="">White…</option>{identities.map(id => <option key={id} value={id}>{id}</option>)}
            </select>
            <select aria-label="Black" value={pair.black} onChange={e => setPair(p => ({ ...p, black: e.target.value }))} style={inputStyle}>
              <option value="">Black…</option>{identities.map(id => <option key={id} value={id}>{id}</option>)}
            </select>
            <select aria-label="Result" value={pair.result} onChange={e => setPair(p => ({ ...p, result: e.target.value as SwissOutcome }))} style={inputStyle}>
              <option value="white">White wins</option><option value="black">Black wins</option><option value="draw">Draw</option>
            </select>
            <button disabled={busy || !pair.white || !pair.black}
              onClick={() => apply(t => recordOfflineSwissResult(t, pair.white, pair.black, pair.result), "Result recorded.")}
              className="primary" style={{ borderRadius: "8px" }}>RECORD GAME</button>
          </div>
        )}
        <table style={{ width: "100%", fontSize: "12px", borderCollapse: "collapse" }}>
          <thead><tr>{["#", "PLAYER", "PTS", "OPPONENTS"].map(h => <th key={h} style={{ textAlign: "left", padding: "6px", color: "var(--text-dim)" }}>{h}</th>)}</tr></thead>
          <tbody>
            {event.standings.map(s => (
              <tr key={s.identity}><td style={{ padding: "6px" }}>{s.rank}</td><td>{s.identity}</td><td>{s.score}</td><td style={{ color: "var(--text-dim)" }}>{s.opponents.join(", ") || "—"}</td></tr>
            ))}
          </tbody>
        </table>
      </div>
    )}
  </div>;
}
