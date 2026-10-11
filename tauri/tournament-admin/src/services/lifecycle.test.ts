import { describe, expect, it } from "vitest";
import { nextStep, playersNeededToStart } from "./lifecycle";
import type { TournamentDetail } from "./tournamentDetail";

const base = (over: Partial<TournamentDetail> = {}): TournamentDetail => ({
  tournament_id: 1, name: "T", status: "Registration", max_players: 4, entry_fee_lamports: 0,
  players: [], prize_shares: [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0], format: "SingleElimination",
  ...over,
});

describe("nextStep", () => {
  it("asks to fund a paid event before anything else", () => {
    const step = nextStep(base({ entry_fee_lamports: 1_000_000 }), { onChainPrizeLamports: 0 });
    expect(step.action).toBe("fund-prize");
    expect(nextStep(base({ entry_fee_lamports: 1_000_000 }), { onChainPrizeLamports: 5 }).action).not.toBe("fund-prize");
  });

  it("brackets wait until full, then offer start", () => {
    expect(nextStep(base({ players: ["a", "b", "c"] })).title).toMatch("3/4");
    expect(nextStep(base({ players: ["a", "b", "c"] })).action).toBeUndefined();
    expect(nextStep(base({ players: ["a", "b", "c", "d"] })).action).toBe("start");
  });

  it("Swiss starts at its minimum, not at capacity", () => {
    const swiss = base({ format: "Swiss", max_players: 16, min_players: 4, total_rounds: 5 });
    expect(playersNeededToStart(swiss)).toBe(4);
    expect(nextStep({ ...swiss, players: ["a", "b", "c"] }).action).toBeUndefined();
    expect(nextStep({ ...swiss, players: ["a", "b", "c", "d"] }).action).toBe("start");
  });

  it("active events run themselves; completed ones lead to payout", () => {
    expect(nextStep(base({ status: "Active" })).tone).toBe("waiting");
    expect(nextStep(base({ status: "Completed" })).action).toBe("approve-release");
    expect(nextStep(base({ status: "Completed", prize_release_approved: true })).action).toBeUndefined();
    expect(nextStep(base({ status: "Completed", prizes_distributed: true })).tone).toBe("done");
    expect(nextStep(base({ status: "Cancelled" })).tone).toBe("done");
  });
});
