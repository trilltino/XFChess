# State reconciliation contract: Solana / Ephemeral Rollup / durable event log

## Context

A live XFChess game has its state represented in three places at once, each
updated independently and at different speeds:

1. **Solana base-layer state** — the `Game` PDA. Slow (finalizes on normal
   Solana block times), but the ultimate source of truth for money movement
   (wager escrow, ELO updates, treasury fees) once a game is finalized.
2. **Ephemeral Rollup (ER) state** — the same `Game` PDA, delegated to
   MagicBlock's ER for sub-second move recording during live play.
   Fast, but provisional: it only becomes real (reflected back on Solana)
   once undelegation completes.
3. **The durable event log** — `backend/src/signing/routes/game_log.rs`,
   backed by the `game_event_log` SQLite table. A causally-ordered record of
   moves/resign/draw/chat/session-info events, served over Braid-HTTP 209.
   It's the backend-side fallback/catch-up path when direct P2P gossip
   between clients doesn't land a message — not a replacement for gossip,
   and not itself a signing authority.

These three were never previously written down as having an explicit
priority order. This document names one, and the accompanying test in
`backend/src/signing/routes/main.rs`
(`settlement_deactivation_blocks_further_moves_even_with_a_live_session_on_file`)
enforces the one instance of it that's concretely testable today.

## The contract

**Solana finalized state is authoritative once undelegation completes.**
Once a game's `Game` PDA is back on Solana and shows `Finished` (with a
winner/draw recorded), that is the final word — no ER move, no durable-log
entry, no P2P gossip message can change the outcome after this point.
Concretely: `tasks::settlement_worker` observing this state calls
`SessionStore::deactivate(game_id)`, which is the actual enforcement
mechanism — every move-recording path
(`record_move`/`delegate_game`/`undelegate_game`/`finalize_game`) goes
through `resolve_move_signer`, which refuses to resolve a signer for a
deactivated session (`MoveSignerError::SessionInactive`) regardless of what
any other system still believes about the game.

**The durable event log is authoritative for ordering during live play.**
While a game is in progress (not yet finalized on Solana), the event log's
causal chain (`content_version`/`content_parent`, validated in
`GameLogState::put_event`) is what a client falls back to when direct P2P
delivery fails or a client needs to catch up after a reconnect. It answers
"what order did events happen in, as best either connected client and the
backend agree" — not "what is the legally final result." Its role ends the
moment finalization happens; nothing re-reads it to influence a finalized
outcome.

**ER state is authoritative for the current position during live play, but
provisional until undelegation.** Every move recorded via
`record_move`/`global_record_move` on the ER is real in the sense that it's
signed and lands on-chain (on the ER's chain), and the game's displayed
position follows it immediately. But it does not settle money, update ELO,
or close the wager escrow — that only happens once `undelegate_game` runs
and the account is back on Solana base layer, then `finalize_game` (or the
settlement worker's automatic equivalent) closes it out. A game stuck
mid-ER (delegation never completes, or the ER becomes unavailable) has no
finalized truth yet, by design — see
`docs/plans/networking-hardening-plan.md`'s ER-unavailability recovery path
for what happens when that stall needs to be forced.

## Recovery behavior when these three actually disagree

| Scenario | What happens |
|---|---|
| Event log has moves the ER never recorded (client only reached the durable-log fallback, ER submission failed) | Live position follows the ER/on-chain record, since that's what `record_move` actually persists on-chain — the event log is a relay aid, not a second ledger of truth. A client resyncs its local board from the ER account, not from the durable log. |
| ER shows the game still in progress, but Solana shows `Finished` (undelegation + finalize already ran) | `SessionStore.active` is already `false` (settlement worker's `deactivate` call). Any further `record_move` attempt — stale client, replay, or a race — is rejected with `SessionInactive` before it reaches the ER at all. This is the one case with a direct enforcement test today. |
| Durable event log has a move whose `content_parent` doesn't match the log's current head (a forked/out-of-order write) | Rejected outright by `GameLogState::put_event`'s causal-chain check (`PutEventError::ParentMismatch`), independent of Solana/ER state entirely — this is a same-layer integrity check, not a cross-layer reconciliation. |
| ER delegation stalls indefinitely (ER unavailable) | Neither Solana nor the event log can produce a finalized result on their own. Recovery is the self-serve ER-unavailability path (`docs/plans/networking-hardening-plan.md`) — a timeout-driven forced undelegation, not silently trusting the event log's last known position as final. |

## What's still open

- The event log's role is currently enforced by convention (nothing reads
  it as an input to `finalize_game`), not by an explicit assertion anywhere
  that finalization ignores it. A future refactor that added such a read
  path would need to re-derive this contract rather than violate it
  silently.
- There is no automated test exercising the ER-unavailability forced-timeout
  path against this contract specifically — it's covered by the existing
  settlement-worker stale-delegation metrics and manual admin recovery
  route, not a reconciliation test.
