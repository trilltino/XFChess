import type { TournamentDetail } from "./tournamentDetail";

/** What the operator can do next. Each maps to one button in the lifecycle panel. */
export type LifecycleAction = "fund-prize" | "start" | "approve-release";

export interface NextStep {
  tone: "action" | "waiting" | "done" | "problem";
  title: string;
  detail: string;
  action?: LifecycleAction;
}

export interface LifecycleContext {
  /** Prize actually locked on-chain (from registration reconciliation), if known. */
  onChainPrizeLamports?: number | null;
}

/** Swiss may start at its minimum; the program only starts a bracket when full. */
export function playersNeededToStart(t: TournamentDetail): number {
  if (t.format === "SingleElimination") return t.max_players;
  return Math.max(2, t.min_players ?? Math.min(t.max_players, 8));
}

/**
 * The single next thing to do for `t`, in lifecycle order: fund a paid event's
 * prize, gather players, start, play (automatic), then pay out.
 */
export function nextStep(t: TournamentDetail, ctx: LifecycleContext = {}): NextStep {
  const status = t.status.toLowerCase();
  const registered = t.players.length;

  if (status === "cancelled") {
    return { tone: "done", title: "Cancelled", detail: "Entry fees and the guaranteed prize were returned on-chain." };
  }

  if (status === "completed") {
    if (t.prizes_distributed) {
      return { tone: "done", title: "Finished — prizes paid", detail: "Winners have been paid on-chain." };
    }
    return {
      tone: "action",
      title: "Finished — payout pending",
      detail: "Prizes are paid automatically after the anti-cheat hold. Large prize pools also need your approval.",
      action: t.prize_release_approved ? undefined : "approve-release",
    };
  }

  if (status === "active") {
    const round = t.format === "Swiss" && t.current_round
      ? ` Round ${t.current_round} of ${t.total_rounds ?? "?"}.`
      : "";
    return {
      tone: "waiting",
      title: "In progress",
      detail: `Games are created, played and settled automatically.${round} Use the Matches tab only for emergency overrides.`,
    };
  }

  // Registration.
  const prizeLocked = (ctx.onChainPrizeLamports ?? t.prize_pool ?? 0) > 0;
  if (t.entry_fee_lamports > 0 && !prizeLocked) {
    return {
      tone: "problem",
      title: "Fund the guaranteed prize",
      detail: "Paid tournaments reject every registration on-chain until a prize is locked.",
      action: "fund-prize",
    };
  }

  const needed = playersNeededToStart(t);
  if (registered < needed) {
    const how = t.format === "SingleElimination"
      ? "A bracket starts automatically the moment it is full."
      : `Swiss can start once ${needed} players have joined.`;
    return {
      tone: "waiting",
      title: `Waiting for players — ${registered}/${needed}`,
      detail: `${how} Share the Blink to fill it.`,
    };
  }

  return {
    tone: "action",
    title: t.format === "SingleElimination" ? "Bracket full — ready to start" : `Ready to start — ${registered} players`,
    detail: t.format === "SingleElimination"
      ? "It starts on its own; start now if it has not after a minute (this also retries a failed on-chain start)."
      : `Pairs round 1 of ${t.total_rounds ?? "?"} and starts the tournament on-chain.`,
    action: "start",
  };
}
