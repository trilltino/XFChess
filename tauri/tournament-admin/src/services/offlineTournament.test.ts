import { describe, expect, it } from "vitest";
import {
  createOfflineTournament,
  publishOfflineTournament,
  recordOfflineMatch,
  recordOfflineSwissResult,
  startOfflineTournament,
} from "./offlineTournament";

const players = ["a", "b", "c", "d"].map(identity => ({ identity, name: identity }));

describe("offline tournaments", () => {
  it("creates and advances a single-elimination bracket", () => {
    let tournament = createOfflineTournament({ id: "local-1", name: "Local Cup", format: "single_elimination", participants: players });
    tournament = startOfflineTournament(publishOfflineTournament(tournament));
    tournament = recordOfflineMatch(tournament, 0, "white");
    tournament = recordOfflineMatch(tournament, 1, "black");
    expect(tournament.matches.find(match => match.round === 2)?.white).toBe("a");
    expect(tournament.matches.find(match => match.round === 2)?.black).toBe("d");
    expect(tournament.revision).toBeGreaterThan(0);
  });

  it("scores Swiss results and rejects duplicate pairings", () => {
    let tournament = createOfflineTournament({ id: "local-2", name: "Swiss Night", format: "swiss", participants: players, totalRounds: 2 });
    tournament = startOfflineTournament(publishOfflineTournament(tournament));
    tournament = recordOfflineSwissResult(tournament, "a", "b", "white");
    expect(tournament.standings.find(s => s.identity === "a")?.score).toBe(1);
    expect(() => recordOfflineSwissResult(tournament, "a", "b", "draw")).toThrow("Invalid Swiss pairing");
  });

  it("rejects duplicate identities", () => {
    expect(() => createOfflineTournament({ id: "bad", name: "Bad", format: "swiss", participants: [players[0], players[0]] })).toThrow("unique");
  });
});
