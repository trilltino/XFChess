import { useEffect, useState } from "react";
import { apiClient, type OfflineTournamentSummary } from "../../services/api";
import { createOfflineTournament, type OfflineFormat } from "../../services/offlineTournament";
import OfflineEventDetail from "./OfflineEventDetail";
import { buttonStyle, cardStyle, inputStyle } from "./offlineStyles";

/** Off-chain ("normal") events: no escrow or on-chain state, results recorded here. */
export default function OfflineTournamentAdmin() {
  const [events, setEvents] = useState<OfflineTournamentSummary[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [format, setFormat] = useState<OfflineFormat>("single_elimination");
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
    if (!name.trim() || names.length < 2) {
      setMessage("Enter a name and at least two participant identities.");
      return;
    }
    let state;
    try {
      const id = `offline-${Date.now()}`;
      state = createOfflineTournament({
        id,
        name: name.trim(),
        format,
        participants: names.map(identity => ({ identity, name: identity })),
      });
    } catch (error) {
      setMessage(`Error: ${error instanceof Error ? error.message : "Invalid event"}`);
      return;
    }
    setBusy(true);
    setMessage(null);
    const result = await apiClient.createOfflineTournament({
      tournament_id: state.id,
      name: state.name,
      format: state.format,
      state,
    });
    setBusy(false);
    if (!result.ok) {
      setMessage(`Error: ${result.error?.message ?? "Create failed"}`);
      return;
    }
    setName("");
    setParticipants("");
    setMessage("Offline event created as draft.");
    await load();
  };

  if (selectedId) {
    return <OfflineEventDetail tournamentId={selectedId} onBack={() => { setSelectedId(null); void load(); }} />;
  }

  return <div style={{ padding: "1rem", maxWidth: "1100px" }}>
    <p style={{ color: "var(--text-dim)", fontSize: "12px", marginTop: 0 }}>
      Off-chain events have no entry fees, escrow or prizes. Use Create Tournament for on-chain events.
    </p>
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: "10px", marginBottom: "1.5rem" }}>
      <input value={name} onChange={e => setName(e.target.value)} placeholder="Event name" style={inputStyle} />
      <select value={format} onChange={e => setFormat(e.target.value as OfflineFormat)} style={inputStyle}>
        <option value="single_elimination">Single elimination</option>
        <option value="swiss">Swiss</option>
      </select>
      <input value={participants} onChange={e => setParticipants(e.target.value)} placeholder="Iroh/name identities, comma separated" style={{ ...inputStyle, gridColumn: "span 2" }} />
      <button disabled={busy} onClick={create} className="primary" style={{ borderRadius: "8px" }}>CREATE OFFLINE EVENT</button>
    </div>
    {message && <div style={{ color: message.startsWith("Error") ? "#f87171" : "#4ade80", fontSize: "12px", marginBottom: "1rem" }}>{message}</div>}
    <div style={{ display: "grid", gap: "10px" }}>
      {events.length === 0 && <div style={{ color: "var(--text-dim)" }}>No offline events yet.</div>}
      {events.map(event => (
        <div key={event.tournament_id} style={cardStyle}>
          <div>
            <strong>{event.name}</strong>
            <div style={{ color: "var(--text-dim)", fontSize: "11px", marginTop: "4px" }}>
              {event.format} · {event.status} · {event.participants.length} players · revision {event.revision}
            </div>
          </div>
          <button onClick={() => setSelectedId(event.tournament_id)} style={buttonStyle}>MANAGE</button>
        </div>
      ))}
    </div>
  </div>;
}
