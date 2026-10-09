import type { CreateTournamentRequest, TournamentDetail } from "./api";

export type ResultChoice = "white" | "black" | "draw";
export type Side = "white" | "black";
export type OperationStatus = "requested" | "submitted" | "pending_reconciliation" | "resolved" | "failed";
export interface TournamentOperation {
  id: string; tournament_id: string; actor: string; reason: string;
  status: OperationStatus; signature: string | null; last_error: string | null;
  created_at: number; updated_at: number;
}
export interface CancellationResponse {
  ok: boolean; status: OperationStatus; signature: string | null; operation: TournamentOperation;
}
export function cancellationState(status?: OperationStatus) {
  return { pending: status !== "resolved" && status !== "failed",
    label: status === "resolved" ? "Cancellation resolved" : status === "failed" ? "Cancellation failed: review recovery evidence" : `Cancellation pending (${status ?? "unconfirmed"})` };
}
export interface SwissResultRequest {
  round: number;
  board: number;
  result: "1-0" | "0-1" | "draw" | "forfeit-white" | "forfeit-black";
  reason?: string;
}

export function resolveResult(format: string, choice?: ResultChoice | null, forfeitingSide?: Side, reason?: string) {
  if (forfeitingSide) {
    if (!reason?.trim()) throw new Error("A forfeit reason is required.");
    return { winner: forfeitingSide === "white" ? "black" as const : "white" as const,
      result: forfeitingSide === "white" ? "forfeit-black" as const : "forfeit-white" as const,
      reason: reason.trim() };
  }
  if (!choice) throw new Error("Select a result or forfeiting side.");
  if (choice === "draw" && format !== "Swiss") throw new Error("Elimination matches cannot end in a draw.");
  return { winner: choice, result: choice === "white" ? "1-0" as const : choice === "black" ? "0-1" as const : "draw" as const };
}

export function swissResult(round: number, board: number | undefined, outcome: ReturnType<typeof resolveResult>): SwissResultRequest {
  if (!Number.isInteger(round) || round < 1 || round > 255 || board == null || !Number.isInteger(board) || board < 0 || board > 65535)
    throw new Error("Valid Swiss round and board are required.");
  return { round, board, result: outcome.result, ...(outcome.reason ? { reason: outcome.reason } : {}) };
}

export function duplicateConfig(source: TournamentDetail, newId: number): CreateTournamentRequest {
  if (!Number.isSafeInteger(newId) || newId <= 0 || newId === source.tournament_id) throw new Error("A new tournament ID is required.");
  if (source.format !== "Swiss" && source.format !== "SingleElimination") throw new Error("Unsupported format.");
  return {
    tournament_id: newId, name: `${source.max_players}-player ${source.format}`,
    format: source.format, max_players: source.max_players as CreateTournamentRequest["max_players"],
    entry_fee_lamports: source.entry_fee_lamports, platform_fee_lamports: source.platform_fee_lamports ?? 0,
    prize_shares: source.prize_shares ? [...source.prize_shares] : undefined,
    swiss_rounds: source.total_rounds, elo_min: source.elo_min, elo_max: source.elo_max,
    kyc_required: source.kyc_required,
  };
}

export function refundOutcome(data?: { status?: string; signature?: string }) {
  if (data?.status === "awaiting_manual_execution") return { message: "Request accepted. Awaiting manual execution on the isolated signing host; no funds have been sent.", signature: null };
  if (data?.signature?.trim()) return { message: "Transaction submitted. Check its confirmation status.", signature: data.signature };
  return { message: "Request accepted. Execution is unconfirmed; no transaction signature returned.", signature: null };
}
