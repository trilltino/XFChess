import { describe, expect, it } from "vitest";
import { normalizeTournamentDetail } from "./tournamentDetail";

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