import { describe, expect, it } from "vitest";
import { normalizeTournamentList } from "./api";
import { normalizeTournamentDetail } from "./tournamentDetail";

describe("normalizeTournamentList", () => {
  const row = {
    tournament_id: 1,
    name: "Open Cup",
    entry_fee_lamports: 0,
    prize_pool: 0,
    max_players: 2,
    registered: 0,
    status: "Registration",
  };

  it("accepts the backend's bare array response", () => {
    expect(normalizeTournamentList([row])).toEqual([row]);
  });

  it("accepts wrapped list responses from proxies or alternate serializers", () => {
    expect(normalizeTournamentList({ value: [row], Count: 1 })).toEqual([row]);
    expect(normalizeTournamentList({ Value: [row] })).toEqual([row]);
    expect(normalizeTournamentList({ tournaments: [row] })).toEqual([row]);
  });

  it("falls back to an empty list for unexpected shapes", () => {
    expect(normalizeTournamentList({ ok: true })).toEqual([]);
  });
});

describe("normalizeTournamentDetail", () => {
  it("normalizes the backend Swiss enum and nested round data", () => {
    const detail = normalizeTournamentDetail({
      format: { Swiss: { rounds: 5 } },
      swiss_data: {
        current_round: 2,
        total_rounds: 5,
        standings: [],
        rounds: [],
      },
    });

    expect(detail.format).toBe("Swiss");
    expect(detail.current_round).toBe(2);
    expect(detail.total_rounds).toBe(5);
  });

  it("preserves the existing single-elimination string shape", () => {
    const detail = normalizeTournamentDetail({ format: "single_elimination" });

    expect(detail.format).toBe("SingleElimination");
  });
});
