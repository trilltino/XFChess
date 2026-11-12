import { useState } from "react";
import { apiClient, type TournamentDetail } from "../../../services/api";
import { dangerButtonStyle, ghostButtonStyle, InfoCard, inputStyle, Message, primaryButtonStyle, shorten } from "./ui";

type Result = { ok: boolean; error?: { message?: string; status?: number } };

export default function PlayersTab({ tournament, onChanged }: { tournament: TournamentDetail; onChanged: () => Promise<void> }) {
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const act = async (confirmText: string | null, call: () => Promise<Result>, done: string) => {
    if (confirmText && !window.confirm(confirmText)) return;
    setBusy(true);
    setMessage(null);
    const r = await call();
    setMessage(r.ok ? done : `Error: ${r.error?.message || r.error?.status || "request failed"}`);
    setBusy(false);
    if (r.ok) await onChanged();
  };

  const swissActive = tournament.format === "Swiss" && tournament.status === "Active";
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      <Message text={message} />
      <PlayerRoster tournament={tournament} busy={busy} act={act} swissActive={swissActive} />
      {swissActive && <PairingRules tournament={tournament} busy={busy} act={act} />}
    </div>
  );
}

function PlayerRoster({ tournament, busy, act, swissActive }: {
  tournament: TournamentDetail;
  busy: boolean;
  swissActive: boolean;
  act: (confirmText: string | null, call: () => Promise<Result>, done: string) => Promise<void>;
}) {
  const id = tournament.tournament_id;
  const sd = tournament.swiss_data;
  const round = sd?.current_round ?? 0;
  const currentRound = sd?.rounds.find(r => r.round === round);
  const [reseedOrder, setReseedOrder] = useState<string[] | null>(null);
  const list = reseedOrder ?? tournament.players;
  const canReseed = tournament.status === "Registration" && tournament.format === "SingleElimination";

  const isPaired = (p: string) => Boolean(currentRound?.pairings.some(x => x.white === p || x.black === p));
  const status = (p: string) =>
    sd?.withdrawn_players?.includes(p) ? "WITHDRAWN" : sd?.absent_players?.includes(p) ? "ABSENT" : null;

  return (
    <InfoCard title={`${tournament.players.length} PLAYERS`}>
      {canReseed && (
        <div style={{ display: "flex", gap: "8px", marginBottom: "1rem" }}>
          {reseedOrder == null
            ? <button onClick={() => setReseedOrder([...tournament.players])} style={ghostButtonStyle}>RESEED (DRAG TO REORDER)</button>
            : <>
              <button disabled={busy} style={primaryButtonStyle}
                onClick={() => act("Apply this seeding? Seed 1 meets the last seed in round 1.", () => apiClient.reseedPlayers(id, reseedOrder), "Players reseeded.").then(() => setReseedOrder(null))}>
                APPLY SEEDING
              </button>
              <button onClick={() => setReseedOrder(null)} style={ghostButtonStyle}>DISCARD</button>
            </>}
        </div>
      )}
      <div style={{ maxHeight: "420px", overflow: "auto" }}>
        {list.map((p, i) => (
          <div key={p} draggable={reseedOrder != null}
            onDragStart={e => e.dataTransfer.setData("text/plain", p)}
            onDragOver={e => e.preventDefault()}
            onDrop={e => {
              if (!reseedOrder) return;
              const dragged = e.dataTransfer.getData("text/plain");
              const next = reseedOrder.filter(x => x !== dragged);
              next.splice(i, 0, dragged);
              setReseedOrder(next);
            }}
            style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "0.75rem 1rem", borderRadius: "10px", cursor: reseedOrder ? "grab" : "default" }}>
            <div style={{ color: "#fff", fontSize: "13px", fontFamily: "monospace" }}>
              <span style={{ color: "var(--primary)", marginRight: "1rem", opacity: 0.5 }}>{String(i + 1).padStart(2, "0")}</span>
              {p}
            </div>
            <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
              {tournament.player_elos?.[tournament.players.indexOf(p)] ? (
                <span style={{ color: "var(--accent)", fontSize: "11px", fontWeight: "bold" }}>ELO {tournament.player_elos[tournament.players.indexOf(p)]}</span>
              ) : null}
              {swissActive && (() => {
                const s = status(p);
                if (s === "WITHDRAWN") return <span style={{ color: "var(--text-dim)", fontSize: "10px", fontWeight: 800 }}>WITHDRAWN</span>;
                return <>
                  {s === "ABSENT"
                    ? <button disabled={busy} style={ghostButtonStyle} onClick={() => act(null, () => apiClient.rejoinSwissPlayer(id, p), `${shorten(p)} is back in the pairings.`)}>REJOIN</button>
                    : isPaired(p) && <button disabled={busy} style={dangerButtonStyle}
                        onClick={() => act(`Mark ${shorten(p)} absent for round ${round}? Their board is forfeited.`, () => apiClient.markSwissAbsent(id, p, round), `${shorten(p)} forfeits round ${round}.`)}>ABSENT</button>}
                  <button disabled={busy} style={dangerButtonStyle}
                    onClick={() => act(`Withdraw ${shorten(p)}? Their open board is forfeited and they are not paired again.`, () => apiClient.withdrawSwissPlayer(id, p), `Withdrew ${shorten(p)}.`)}>WITHDRAW</button>
                </>;
              })()}
            </div>
          </div>
        ))}
      </div>
    </InfoCard>
  );
}

function PairingRules({ tournament, busy, act }: {
  tournament: TournamentDetail;
  busy: boolean;
  act: (confirmText: string | null, call: () => Promise<Result>, done: string) => Promise<void>;
}) {
  const id = tournament.tournament_id;
  const sd = tournament.swiss_data;
  const active = tournament.players.filter(p => !sd?.withdrawn_players?.includes(p));
  const [forbid, setForbid] = useState({ a: "", b: "" });
  const [manual, setManual] = useState({ white: "", black: "" });


  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(300px, 1fr))", gap: "1.5rem" }}>
      <InfoCard title="NEVER PAIR">
        <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: "0 0 12px" }}>Keeps two players apart (same club, household…) from the next round on.</p>
        <div style={{ display: "flex", gap: "6px" }}>
          <PlayerSelect players={active} label="Player A" value={forbid.a} onChange={a => setForbid(f => ({ ...f, a }))} />
          <PlayerSelect players={active} label="Player B" value={forbid.b} onChange={b => setForbid(f => ({ ...f, b }))} />
          <button disabled={busy || !forbid.a || !forbid.b || forbid.a === forbid.b} style={primaryButtonStyle}
            onClick={() => act(null, () => apiClient.addForbiddenPair(id, forbid.a, forbid.b), "Rule added.").then(() => setForbid({ a: "", b: "" }))}>ADD</button>
        </div>
        {(sd?.forbidden_pairs ?? []).map(([a, b]) => (
          <div key={`${a}-${b}`} style={{ display: "flex", justifyContent: "space-between", fontSize: "11px", padding: "6px 0", color: "var(--text-dim)" }}>
            <span>{shorten(a)} ✕ {shorten(b)}</span>
            <button disabled={busy} style={dangerButtonStyle} onClick={() => act(null, () => apiClient.removeForbiddenPair(id, a, b), "Rule removed.")}>REMOVE</button>
          </div>
        ))}
      </InfoCard>

      <InfoCard title="FORCE A PAIRING (NEXT ROUND)">
        <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: "0 0 12px" }}>Overrides the engine for one board of the next round; it is used once, then cleared.</p>
        <div style={{ display: "flex", gap: "6px" }}>
          <PlayerSelect players={active} label="White" value={manual.white} onChange={white => setManual(m => ({ ...m, white }))} />
          <PlayerSelect players={active} label="Black" value={manual.black} onChange={black => setManual(m => ({ ...m, black }))} />
          <button disabled={busy || !manual.white || !manual.black || manual.white === manual.black} style={primaryButtonStyle}
            onClick={() => act(null, () => apiClient.addManualPairing(id, manual.white, manual.black), "Pairing queued for next round.").then(() => setManual({ white: "", black: "" }))}>ADD</button>
        </div>
        {(sd?.manual_pairings_next_round ?? []).map(({ white, black }) => (
          <div key={`${white}-${black}`} style={{ display: "flex", justifyContent: "space-between", fontSize: "11px", padding: "6px 0", color: "var(--text-dim)" }}>
            <span>{shorten(white)} (W) vs {shorten(black)} (B)</span>
            <button disabled={busy} style={dangerButtonStyle} onClick={() => act(null, () => apiClient.removeManualPairing(id, white, black), "Pairing removed.")}>REMOVE</button>
          </div>
        ))}
      </InfoCard>
    </div>
  );
}

function PlayerSelect({ players, label, value, onChange }: { players: string[]; label: string; value: string; onChange: (v: string) => void }) {
  return (
    <select aria-label={label} value={value} onChange={e => onChange(e.target.value)} style={{ ...inputStyle, flex: 1 }}>
      <option value="">{label}…</option>
      {players.map(p => <option key={p} value={p}>{shorten(p)}</option>)}
    </select>
  );
}
