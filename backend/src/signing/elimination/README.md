# backend/src/signing/elimination

Single-elimination brackets. Swiss events live in [`../swiss`](../swiss/); both are
driven by the shared [`../orchestrator.rs`](../orchestrator.rs).

## Files

| File | Contents |
|------|----------|
| [bracket.rs](bracket.rs) | `EliminationState`: the draw, the rules for applying a game or a decision, and the queries (assignments, open games, game ids) |
| [service.rs](service.rs) | Applies those rules to a stored tournament and fills the placings |
| [chain.rs](chain.rs) | Creates the bracket's match accounts at start and mirrors decided matches on-chain |

## Flow

1. The scheduler's start draws the whole bracket (`TournamentRecord::start`): the
   best seed plays the worst, and every round-one game is scheduled at once.
   `chain::initialize_matches` creates the same matches on-chain.
2. Each game id is deterministic:
   `TournamentStore::bracket_game_id(tournament, match, attempt)`, where `attempt` is
   the number of drawn games before it. `my-status` hands it to both players.
3. When the game settles, the orchestrator calls `EliminationService::record_game`
   with the Game's players and outcome:
   - a win decides the match and seats the winner in their next match;
   - a draw is replayed under a new game id with colours reversed;
   - once `MAX_REPLAYS` replays have also been drawn, the higher seed goes through
     (`ResultSource::SeedTiebreak`).
4. The final completes the event; the semifinal losers take third and fourth.
5. The orchestrator's reconcile pass runs `chain::reconcile` for every decided match
   that is not `chain_synced`: `record_result`, then `advance_winner`.

## Invariants

- Results are decided off-chain first. The chain mirror only reads them, and each
  step reads the on-chain match before sending, so a retry never sends a landed
  transaction twice. A match is `chain_synced` only once both steps have landed.
- Forfeits and admin results go through `EliminationService::record_decision`, never
  straight to the chain.
- Rounds and match indices are 0-based, as on-chain.
- The stored bracket is never redrawn once drawn; `TournamentRecord::start` refuses.
