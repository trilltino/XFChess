import { useEffect, useState } from "react";
import { apiClient, type TournamentDetail } from "../../../services/api";
import { resolveResult, swissResult, type ResultChoice, type Side } from "../../../services/adminFlows";
import { ghostButtonStyle, InfoCard, inputStyle, Message, PlayerTag, primaryButtonStyle, shorten, StatusPill } from "./ui";
import type { BracketData, MatchRecord } from "./useTournamentData";

/**
 * Games are created, played and settled automatically. This tab shows them and
 * keeps the manual result/forfeit controls folded away as emergency overrides.
 */
export default function MatchesTab({ tournament, bracket, onChanged }: {
  tournament: TournamentDetail;
  bracket: BracketData | null;
  onChanged: () => Promise<void>;
}) {
  const isSwiss = tournament.format === "Swiss";
  const sd = tournament.swiss_data;

  const byRound = new Map<number, MatchRecord[]>();
  for (const m of bracket?.matches ?? []) {
    if (m) byRound.set(m.round, [...(byRound.get(m.round) ?? []), m]);
  }
  const rounds = [...byRound.keys()].sort((a, b) => a - b);

  return (
    <div>
      {isSwiss && sd && sd.current_round > 0 && <SwissRoundBar tournament={tournament} byRound={byRound} onChanged={onChanged} />}

      {rounds.length === 0 && (
        <div style={{ textAlign: "center", padding: "4rem", color: "var(--text-dim)", border: "1px dashed var(--border)", borderRadius: "24px" }}>
          {isSwiss ? "No pairings yet — round 1 is paired when the tournament starts." : "The bracket is generated when the tournament starts."}
        </div>
      )}

      {rounds.map(r => {
        const matches = byRound.get(r)!;
        const byes = sd?.rounds.find(sr => sr.round === r + 1)?.byes ?? [];
        return (
          <div key={r} style={{ marginBottom: "2rem" }}>
            <div style={{ display: "flex", alignItems: "center", gap: "0.75rem", marginBottom: "1rem" }}>
              <div style={{ fontSize: "11px", fontWeight: "800", letterSpacing: "2px", color: "var(--primary)" }}>ROUND {r + 1}</div>
              {matches.every(m => m.status === "Completed") && (
                <span style={{ fontSize: "10px", backgroundColor: "rgba(74,222,128,0.1)", color: "#4ade80", padding: "2px 8px", borderRadius: "4px", fontWeight: "700" }}>COMPLETE</span>
              )}
              {byes.length > 0 && <span style={{ fontSize: "10px", color: "var(--text-dim)", fontStyle: "italic" }}>BYE: {byes.map(shorten).join(", ")} (+1 pt)</span>}
            </div>
            {matches.map(m => <MatchCard key={m.match_index} tournament={tournament} match={m} onChanged={onChanged} />)}
          </div>
        );
      })}

      {isSwiss && sd?.standings?.length ? <Standings standings={sd.standings} /> : null}
    </div>
  );
}

function SwissRoundBar({ tournament, byRound, onChanged }: {
  tournament: TournamentDetail;
  byRound: Map<number, MatchRecord[]>;
  onChanged: () => Promise<void>;
}) {
  const sd = tournament.swiss_data!;
  const id = tournament.tournament_id;
  // swiss_data.current_round is 1-based; bracket rounds are 0-based.
  const current = byRound.get(sd.current_round - 1) ?? [];
  const roundDone = current.length > 0 && current.every(m => m.status === "Completed");
  const canRecover = roundDone && sd.current_round < sd.total_rounds && tournament.status === "Active";
  const [minutes, setMinutes] = useState("60");
  const [message, setMessage] = useState<string | null>(null);
  const deadline = tournament.round_deadline_at ?? null;

  const setDeadline = async () => {
    const mins = parseInt(minutes, 10);
    if (!Number.isFinite(mins) || mins <= 0) return;
    const r = await apiClient.setRoundDeadline(id, Math.floor(Date.now() / 1000) + mins * 60);
    setMessage(r.ok ? `Round ${sd.current_round} must finish within ${mins} min.` : `Error: ${r.error?.message || r.error?.status}`);
    if (r.ok) await onChanged();
  };

  const advance = async () => {
    if (!window.confirm("Pair the next round now? Rounds normally advance on their own when the last game settles.")) return;
    const r = await apiClient.advanceRound(id);
    setMessage(r.ok ? `Round ${r.data?.new_round ?? "?"} paired.` : `Error: ${r.error?.message || "the current round still has open boards"}`);
    if (r.ok) await onChanged();
  };

  return (
    <div style={{ display: "flex", alignItems: "center", gap: "1rem", flexWrap: "wrap", marginBottom: "1.5rem", padding: "1rem 1.5rem", backgroundColor: "rgba(255,255,255,0.03)", borderRadius: "16px", border: "1px solid var(--border)" }}>
      <div style={{ flex: 1, fontSize: "13px", color: "#fff", fontWeight: 700 }}>
        Swiss round <span style={{ color: "var(--primary)" }}>{sd.current_round}</span> / {sd.total_rounds}
        <Message text={message} />
      </div>
      {deadline && deadline > Date.now() / 1000 && <RoundCountdown deadlineAt={deadline} />}
      <input type="number" min={1} max={480} value={minutes} onChange={e => setMinutes(e.target.value)} aria-label="Round time limit in minutes" style={{ ...inputStyle, width: "64px" }} />
      <button onClick={setDeadline} style={ghostButtonStyle}>SET ROUND DEADLINE (MIN)</button>
      {canRecover && <button onClick={advance} style={{ ...ghostButtonStyle, color: "var(--accent)" }}>PAIR NEXT ROUND NOW</button>}
    </div>
  );
}

function RoundCountdown({ deadlineAt }: { deadlineAt: number }) {
  const [remaining, setRemaining] = useState(deadlineAt - Math.floor(Date.now() / 1000));
  useEffect(() => {
    const t = setInterval(() => setRemaining(deadlineAt - Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(t);
  }, [deadlineAt]);
  if (remaining <= 0) return <span style={{ color: "#f87171", fontWeight: 700, fontSize: "13px" }}>ROUND EXPIRED</span>;
  const pad = (n: number) => String(n).padStart(2, "0");
  const color = remaining < 300 ? "#f87171" : remaining < 900 ? "#fbbf24" : "#4ade80";
  return (
    <span style={{ fontFamily: "monospace", fontSize: "14px", fontWeight: 700, color }}>
      {remaining >= 3600 ? `${pad(Math.floor(remaining / 3600))}:` : ""}{pad(Math.floor((remaining % 3600) / 60))}:{pad(remaining % 60)}
    </span>
  );
}

function MatchCard({ tournament, match, onChanged }: { tournament: TournamentDetail; match: MatchRecord; onChanged: () => Promise<void> }) {
  const white = match.player_white ?? "TBD";
  const black = match.player_black ?? "TBD";
  const done = match.status === "Completed";
  const isSwiss = tournament.format === "Swiss";
  const [choice, setChoice] = useState<ResultChoice | null>(null);
  const [forfeitSide, setForfeitSide] = useState<Side | "">("");
  const [forfeitReason, setForfeitReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const submit = async (forfeit: boolean) => {
    try {
      const outcome = resolveResult(tournament.format, forfeit ? null : choice, forfeit ? forfeitSide || undefined : undefined, forfeitReason);
      if (!window.confirm("Record this result by hand? Normally the settled game records it.")) return;
      setBusy(true);
      setMessage(null);
      const r = isSwiss
        // Bracket rounds are 0-based; the Swiss result API numbers rounds from 1.
        ? await apiClient.recordSwissResult(tournament.tournament_id, swissResult(match.round + 1, match.board, outcome))
        : await apiClient.recordResult(
            tournament.tournament_id, match.match_index,
            outcome.winner === "white" ? white : black,
            outcome.winner === "white" ? black : white,
            outcome.reason);
      setMessage(r.ok ? (forfeit ? "Forfeit recorded." : "Result recorded.") : `Error: ${r.error?.message || "failed"}`);
      if (r.ok) { setChoice(null); await onChanged(); }
    } catch (error) {
      setMessage(`Error: ${error instanceof Error ? error.message : "Result submission failed."}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div style={{ backgroundColor: "rgba(255,255,255,0.03)", border: `1px solid ${done ? "rgba(59,130,246,0.3)" : "var(--border)"}`, borderRadius: "16px", padding: "1.25rem 1.5rem", marginBottom: "10px" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.75rem" }}>
        <span style={{ fontSize: "10px", color: "var(--text-dim)", fontWeight: "800", letterSpacing: "1px" }}>
          {match.board != null ? `BOARD ${match.board}` : `MATCH ${match.match_index + 1}`}
        </span>
        <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
          {!done && match.game_id != null && match.player_white && match.player_black && (
            <button onClick={() => navigator.clipboard.writeText(`xfchess://spectate/${match.game_id}`).catch(() => {})} style={ghostButtonStyle}>
              COPY SPECTATE LINK
            </button>
          )}
          <StatusPill status={match.status} />
        </div>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
        <PlayerTag color="white" name={white} isWinner={done && match.winner === white} />
        <span style={{ color: "var(--text-dim)", fontSize: "11px" }}>vs</span>
        <PlayerTag color="black" name={black} isWinner={done && match.winner === black} />
      </div>
      {done && !match.winner && isSwiss && <div style={{ fontSize: "11px", color: "var(--text-dim)", marginTop: "6px" }}>DRAW</div>}

      {!done && match.player_white && match.player_black && (
        <details style={{ marginTop: "10px" }}>
          <summary style={{ cursor: "pointer", color: "#fbbf24", fontSize: "10px", fontWeight: 800 }}>EMERGENCY OVERRIDE</summary>
          <div style={{ display: "grid", gap: "6px", marginTop: "8px" }}>
            <div style={{ display: "flex", gap: "6px" }}>
              {((isSwiss ? ["white", "black", "draw"] : ["white", "black"]) as ResultChoice[]).map(opt => (
                <button key={opt} onClick={() => setChoice(c => (c === opt ? null : opt))}
                  style={{ ...ghostButtonStyle, flex: 1, color: choice === opt ? "#fff" : "var(--text-dim)", borderColor: choice === opt ? "#fff" : "var(--border)" }}>
                  {opt === "draw" ? "DRAW" : `${opt.toUpperCase()} WINS`}
                </button>
              ))}
              <button disabled={!choice || busy} onClick={() => submit(false)} style={primaryButtonStyle}>{busy ? "…" : "RECORD"}</button>
            </div>
            <div style={{ display: "flex", gap: "6px" }}>
              <select aria-label="Forfeiting side" value={forfeitSide} onChange={e => setForfeitSide(e.target.value as Side | "")} style={inputStyle}>
                <option value="">Forfeit by…</option><option value="white">White</option><option value="black">Black</option>
              </select>
              <input aria-label="Forfeit reason" placeholder="Reason (required)" value={forfeitReason} onChange={e => setForfeitReason(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
              <button disabled={busy || !forfeitSide || !forfeitReason.trim()} onClick={() => submit(true)} style={{ ...ghostButtonStyle, color: "#f87171" }}>FORFEIT</button>
            </div>
            <Message text={message} />
          </div>
        </details>
      )}
    </div>
  );
}

function Standings({ standings }: { standings: NonNullable<TournamentDetail["swiss_data"]>["standings"] }) {
  return (
    <InfoCard title="STANDINGS">
      <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "12px" }}>
        <thead>
          <tr>{["#", "PLAYER", "PTS", "BUCHHOLZ", "S-B", "ELO"].map(h => (
            <th key={h} style={{ textAlign: "left", padding: "6px 10px", color: "var(--text-dim)", fontWeight: "800", fontSize: "10px", borderBottom: "1px solid var(--border)" }}>{h}</th>
          ))}</tr>
        </thead>
        <tbody>
          {standings.map(s => (
            <tr key={s.player_id}>
              <td style={{ padding: "8px 10px", color: s.rank <= 3 ? "var(--accent)" : "var(--text-dim)", fontWeight: "800" }}>{s.rank}</td>
              <td style={{ padding: "8px 10px", fontFamily: "monospace", color: "#fff" }}>{shorten(s.player_id)}</td>
              <td style={{ padding: "8px 10px", color: "var(--primary)", fontWeight: "800" }}>{s.score}</td>
              <td style={{ padding: "8px 10px", color: "var(--text-dim)" }}>{s.buchholz.toFixed(1)}</td>
              <td style={{ padding: "8px 10px", color: "var(--text-dim)" }}>{s.sonneborn.toFixed(1)}</td>
              <td style={{ padding: "8px 10px", color: "var(--text-dim)" }}>{s.rating}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </InfoCard>
  );
}
