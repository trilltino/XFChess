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
    // Seeds pair 1 v 4 and 2 v 3; black wins board 2, so seed 3 (c) advances.
    expect(tournament.matches.find(match => match.round === 2)?.black).toBe("c");
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

describe("offline brackets short of a power of two", () => {
  it("gives the top seeds byes that advance immediately", () => {
    const six = ["a", "b", "c", "d", "e", "f"].map((identity, i) => ({ identity, name: identity, rating: 2000 - i }));
    const t = createOfflineTournament({ id: "six", name: "Six", format: "single_elimination", participants: six });
    const round1 = t.matches.filter(m => m.round === 1);
    expect(round1.map(m => [m.white, m.black])).toEqual([["a", null], ["b", null], ["c", "f"], ["d", "e"]]);
    const round2 = t.matches.filter(m => m.round === 2);
    expect(round2[0]).toMatchObject({ white: "a", black: "b" });
  });
});

describe("offline lifecycle", () => {
  it("completes active events, cancels unfinished ones, and upgrades bare drafts", async () => {
    const { completeOfflineTournament, cancelOfflineTournament, loadOfflineTournament } = await import("./offlineTournament");
    const draft = createOfflineTournament({ id: "s", name: "S", format: "swiss", participants: players });
    expect(() => completeOfflineTournament(draft)).toThrow("active");
    const active = startOfflineTournament(draft);
    expect(completeOfflineTournament(active).status).toBe("completed");
    expect(() => cancelOfflineTournament(completeOfflineTournament(active))).toThrow("finished");
    expect(cancelOfflineTournament(draft).status).toBe("cancelled");

    const upgraded = loadOfflineTournament({ id: "x", name: "X", format: "single_elimination", status: "draft", revision: 0,
      participants: [{ identity: "a", name: "a" }, { identity: "b", name: "b" }] });
    expect(upgraded.matches).toHaveLength(1);
    expect(() => loadOfflineTournament({ id: "y", name: "Y", format: "swiss", status: "active", participants: [] })).toThrow("past draft");
  });
});
