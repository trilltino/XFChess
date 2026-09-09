import { useEffect, useState } from "react";
import { apiClient, type OfflineTournamentSummary } from "../../services/api";

const inputStyle = { background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "10px", fontSize: "12px" };

export default function OfflineTournamentAdmin() {
  const [events, setEvents] = useState<OfflineTournamentSummary[]>([]);
  const [name, setName] = useState("");
  const [format, setFormat] = useState<"single_elimination" | "swiss">("single_elimination");
  const [participants, setParticipants] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = async () => {
    const result = await apiClient.listOfflineTournaments();
    if (result.ok) setEvents(result.data ?? []);
    else setMessage(`Error: ${result.error?.message ?? "Unable to load offline events"}`);
  };
  useEffect(() => { void load(); }, []);

  const create = async () => {
    const names = participants.split(",").map(value => value.trim()).filter(Boolean);
    if (!name.trim() || names.length < 2) { setMessage("Enter a name and at least two participant identities."); return; }
    setBusy(true); setMessage(null);
    const id = `offline-${Date.now()}`;
    const result = await apiClient.createOfflineTournament({
      tournament_id: id,
      name: name.trim(),
      format,
      state: { id, name: name.trim(), format, participants: names.map(identity => ({ identity, name: identity })), status: "draft", revision: 0 },
    });
    setBusy(false);
    if (!result.ok) { setMessage(`Error: ${result.error?.message ?? "Create failed"}`); return; }
    setName(""); setParticipants(""); setMessage("Offline event created as draft."); await load();
  };

  const transition = async (event: OfflineTournamentSummary, status: string) => {
    setBusy(true); setMessage(null);
    const detail = await apiClient.getOfflineTournament(event.tournament_id);
    if (!detail.ok || !detail.data) { setMessage(`Error: ${detail.error?.message ?? "Event not found"}`); setBusy(false); return; }
    const result = await apiClient.updateOfflineTournament(event.tournament_id, { status, state: { ...detail.data.state, status }, revision: detail.data.revision + 1 });
    setBusy(false);
    setMessage(result.ok ? `Event ${status}.` : `Error: ${result.error?.message ?? "Update failed"}`);
    if (result.ok) await load();
  };

  return <div style={{ padding: "1rem", maxWidth: "1100px" }}>
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: "10px", marginBottom: "1.5rem" }}>
      <input value={name} onChange={e => setName(e.target.value)} placeholder="Event name" style={inputStyle} />
      <select value={format} onChange={e => setFormat(e.target.value as typeof format)} style={inputStyle}><option value="single_elimination">Single elimination</option><option value="swiss">Swiss</option></select>
      <input value={participants} onChange={e => setParticipants(e.target.value)} placeholder="Iroh/name identities, comma separated" style={{ ...inputStyle, gridColumn: "span 2" }} />
      <button disabled={busy} onClick={create} className="primary" style={{ borderRadius: "8px" }}>CREATE OFFLINE EVENT</button>
    </div>
    {message && <div style={{ color: message.startsWith("Error") ? "#f87171" : "#4ade80", fontSize: "12px", marginBottom: "1rem" }}>{message}</div>}
    <div style={{ display: "grid", gap: "10px" }}>
      {events.length === 0 && <div style={{ color: "var(--text-dim)" }}>No published offline events.</div>}
      {events.map(event => <OfflineEventCard key={event.tournament_id} event={event} onTransition={transition} />)}
    </div>
  </div>;
}

function OfflineEventCard({ event, onTransition }: { event: OfflineTournamentSummary; onTransition: (event: OfflineTournamentSummary, status: string) => void }) {
  return <div style={{ background: "var(--surface)", border: "1px solid var(--border)", borderRadius: "12px", padding: "1rem", display: "flex", justifyContent: "space-between", gap: "1rem", alignItems: "center" }}>
    <div><strong>{event.name}</strong><div style={{ color: "var(--text-dim)", fontSize: "11px", marginTop: "4px" }}>{event.format} · {event.status} · revision {event.revision}</div></div>
    <div style={{ display: "flex", gap: "6px" }}>{event.status === "draft" && <button onClick={() => onTransition(event, "published")} style={buttonStyle}>PUBLISH</button>}{event.status === "published" && <button onClick={() => onTransition(event, "active")} style={buttonStyle}>START</button>}{event.status !== "completed" && event.status !== "cancelled" && <button onClick={() => onTransition(event, "cancelled")} style={{ ...buttonStyle, color: "#f87171" }}>CANCEL</button>}</div>
  </div>;
}

const buttonStyle = { padding: "6px 10px", borderRadius: "6px", background: "rgba(255,255,255,0.07)", color: "var(--primary)", border: "1px solid var(--border)", fontSize: "10px", fontWeight: 800, cursor: "pointer" };
