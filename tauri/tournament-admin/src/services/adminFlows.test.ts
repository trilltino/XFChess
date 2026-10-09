import { describe, expect, it } from "vitest";
import { cancellationState, duplicateConfig, refundOutcome, resolveResult, swissResult } from "./adminFlows";
import type { TournamentDetail } from "./api";

describe("emergency results", () => {
  it("preserves a Swiss draw and explicit round/board, including board zero", () => {
    expect(swissResult(3, 0, resolveResult("Swiss", "draw"))).toEqual({ round: 3, board: 0, result: "draw" });
  });
  it.each([['white', 'black', 'forfeit-black'], ['black', 'white', 'forfeit-white']] as const)("awards the opponent when %s forfeits", (side, winner, result) => {
    expect(resolveResult("Swiss", null, side, "  absent  ")).toEqual({ winner, result, reason: "absent" });
  });
  it("rejects elimination draws, unselected results and blank forfeit reasons", () => {
    expect(() => resolveResult("SingleElimination", "draw")).toThrow();
    expect(() => resolveResult("Swiss", null)).toThrow();
    expect(() => resolveResult("Swiss", null, "white", "  ")).toThrow();
  });
  it.each([[0, 1], [1, undefined], [1, -1], [1.5, 0], [1, 65536]])("rejects invalid Swiss coordinates %s/%s", (round, board) => {
    expect(() => swissResult(round!, board, resolveResult("Swiss", "white"))).toThrow();
  });
});

describe("config duplication", () => {
  const source = { tournament_id: 123, name: "Old identity", format: "Swiss", max_players: 16,
    status: "Completed", players: ["player"], winner: "player", entry_fee_lamports: 100,
    platform_fee_lamports: 20, prize_shares: [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
    total_rounds: 5, scheduled_at: 100, kyc_required: true, password: "do-not-copy" } as TournamentDetail;
  it("copies only selected config and finances with a fresh identity and independent shares", () => {
    const copy = duplicateConfig(source, 456);
    expect(copy).toMatchObject({ tournament_id: 456, entry_fee_lamports: 100, platform_fee_lamports: 20, swiss_rounds: 5, kyc_required: true });
    for (const key of ["players", "winner", "status", "scheduled_at", "password"]) expect(copy).not.toHaveProperty(key);
    expect(copy.name).not.toBe(source.name);
    copy.prize_shares![0] = 0;
    expect(source.prize_shares[0]).toBe(6000);
  });
  it("rejects reuse of the source ID", () => expect(() => duplicateConfig(source, 123)).toThrow());
});

describe("refund execution evidence", () => {
  it("manual execution takes precedence over a stray signature", () => {
    const outcome = refundOutcome({ status: "awaiting_manual_execution", signature: "stale" });
    expect(outcome.signature).toBeNull();
    expect(outcome.message).toContain("no funds have been sent");
  });
  it("does not infer execution from HTTP success without a signature", () => {
    expect(refundOutcome({}).message).toContain("unconfirmed");
    expect(refundOutcome().signature).toBeNull();
  });
  it("reports submission, not confirmation, when a signature exists", () => {
    expect(refundOutcome({ signature: "tx" })).toEqual({ message: "Transaction submitted. Check its confirmation status.", signature: "tx" });
  });
});

describe("cancellation reconciliation", () => {
  it.each(["requested", "submitted", "pending_reconciliation", undefined] as const)("keeps %s pending", status => {
    expect(cancellationState(status).pending).toBe(true);
    expect(cancellationState(status).label).toContain("pending");
  });
  it("distinguishes failure from resolution", () => {
    expect(cancellationState("failed")).toEqual({ pending: false, label: "Cancellation failed: review recovery evidence" });
    expect(cancellationState("resolved")).toEqual({ pending: false, label: "Cancellation resolved" });
  });
});
