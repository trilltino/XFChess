# backend/src/signing/swiss

Swiss-system tournament orchestration over the
[`swiss-pairing`](../../../../crates/shared/swiss-pairing/) crate (FIDE Dutch
algorithm). Single elimination lives in [`../elimination`](../elimination/); both
are driven by the shared [`../orchestrator.rs`](../orchestrator.rs).

## Files

| File | Contents |
|------|----------|
| [state.rs](state.rs) | `SwissStorageData` (the event's stored state) and the queries the rest of the backend asks of it: assignments, open games, game ids, bracket view |
| [service.rs](service.rs) | Every Swiss state transition: pair a round, record a result, auto-pair the next round, complete the event |
| [chain.rs](chain.rs) | Mirrors results on-chain, signed by the tournament authority |
| [handlers.rs](handlers.rs) | HTTP endpoints |
| [SCORING.md](SCORING.md) | Scoring and tiebreak rules — read this before touching results math |

## Flow

1. The scheduler's start (or `initialize-swiss`) pairs round 1
   (`SwissService::start_round`).
2. Each board's game id is deterministic:
   `TournamentStore::swiss_game_id(tournament, round, board)`. `my-status` hands it
   to both players as `MatchReady`, exactly like a bracket match.
3. When the game settles on-chain, the settlement worker sends
   `OrchestratorEvent::GameEnded`. The orchestrator calls
   `SwissService::record_game`, which decodes `(round, board)` from the game id and
   checks the Game's players against the pairing.
4. The last result of a round pairs the next round; the last result of the final
   round completes the event and writes the top-10 places.
5. The orchestrator's reconcile pass (after every event, and every 30 s) runs
   `chain::reconcile`: `record_swiss_result` per board (a bye is an extra board
   after the real ones), `advance_round` once all of a round's boards are in, then
   `complete_swiss_tournament` with the off-chain top 10.

## Invariants

- Pairing logic lives in the `swiss-pairing` crate; algorithm fixes go there, with tests.
- The orchestrator keeps no state of its own. Chain progress is
  `SwissStorageData::chain_boards_recorded` / `chain_completed` plus the on-chain
  `Tournament.current_round`, so restarts and failed transactions are retried by the
  next reconcile.
- Stored rounds and boards are 1-based, as the pairing engine numbers them. Rounds
  reported outside this module (`my-status`, `my-match`, the bracket view) are
  0-based like bracket rounds; the program's are 0-based too. Only
  `chain::mirror_to_chain` converts for the chain.
- Each board takes one result through `record_result`/`record_game`;
  `override_result` corrects one off-chain only, so use it before the round closes
  on-chain.
