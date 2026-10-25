# Game and tournament state machines (2026-10-25)

Derived from `programs/xfchess-game/src` by grepping every `status =` write and
every status guard, then hand-checked. The Game transition matrix is enforced
by `tests/state_machine_tests.rs` (resign, claim_timeout, record_move,
offer_draw and accept_draw are rejected from every status outside their
allowed set, with no mutation). The remaining rows are covered by the test
named in the last column or are marked **untested**.

## Game (`GameStatus`)

`GamePhase` (`state/game.rs`) pairs a status with `is_delegated`; any pairing
not listed there returns `InvalidLifecycleTransition`.

| From | To | Instruction / path | Guard | Test |
|---|---|---|---|---|
| (none) | `WaitingForOpponent` | `create_game`, `global_create_game(_v2)`, `session_create_game_v2` via `init_game_fields` (`game_ix/common.rs:27`) | wager within `MIN/MAX_WAGER`, funded escrow | `global_create_game_tests` |
| (none) | `Pending` | struct literal in `global_create.rs:130`, `session_create_game.rs:191` | overwritten by `init_game_fields` in the same instruction | n/a, never persisted |
| `WaitingForOpponent` | `Active` | `join_game(_v2)`, `global_join_game_v2`, `session_join_game`, tournament `create_tournament_game` (`transitions::join_waiting_game`) | joiner is not white, fee payer matches | `lifecycle_race_tests` |
| `WaitingForOpponent` | `Cancelled` | `cancel_game` (creator only) | undelegated | `lifecycle_race_tests` |
| `WaitingForOpponent` | `Expired` | `withdraw_expired_wager` | creator, `created_at + 86400 < now` | `account_security_tests` |
| `Active` | `Finished` | `record_move` / `global_record_move` (checkmate, stalemate, insufficient material, 100 half-moves) | session of side to move, legal move, exact next board | `er_move_tests`, `move_authorization_tests` |
| `Active` | `Finished` | `resign`, `accept_draw`, `claim_timeout`, `crank_time_check` (`lifecycle/terminal.rs`) | player / expired clock | `claim_timeout_tests`, `lifecycle_race_tests` |
| `Active` | `Cancelled` | `claim_timeout` or `crank_time_check` with `move_count == 0`; `cancel_game` after 24 h idle or by a player | undelegated | `claim_timeout_tests`, `lifecycle_race_tests` |
| `Active` / `Inactive` | `Disputed` | `dispute_game` (`governance_ix/dispute.rs:39`) | challenger bond | `dispute_tests` |
| `Finished` | `Settled` then account closed | `finalize_game` (`settle_finished_game`) | undelegated, result set, escrow >= pot | `game_settlement_tests`, `global_session_settlement_tests` |
| `Cancelled` | `Settled` then account closed | `finalize_game` (`settle_cancelled_game`) | undelegated, result `None` | `global_session_settlement_tests` |
| `Disputed` | `Settled` | `resolve_dispute` (authority), `claim_stale_dispute` (after TTL) | dispute `Pending` | `dispute_tests` |
| any non-terminal | wiped | `force_undelegate_after_timeout` wipes the Game account (DLP resizes to 0) | stuck-delegation guard; recovery via `RecoveryIdentity` | `er_recovery_tests`, `rollback_recipient_tests` |

Observations:

- `GameStatus::Inactive` is accepted by `dispute_game` but no instruction ever
  writes it. Dead state, harmless.
- `Expired` is reachable only from `withdraw_expired_wager`; nothing reads it
  except `GamePhase`, which has no `Expired` phase, so `phase()` on an expired
  game errors. Callers must not call `phase()` on a withdrawn game.
- `Settled` is terminal. The `finalize_game` path closes the account, so the
  status is only visible to `resolve_dispute` / `claim_stale_dispute`.
- Threefold repetition is not enforced on-chain (no position history is
  stored). Draws by repetition require `offer_draw` / `accept_draw`.

### Recovery identity (`RecoveryStatus`)

`Waiting -> Ready -> Delegated -> Recovered`, plus `Waiting|Ready|Delegated ->
Closed` when an ordinary payout runs (`close_identity`). `recover` requires
`Delegated`; the new test `recovery_only_runs_for_a_delegated_identity_and_never_twice`
checks `Waiting`, `Ready`, `Closed` and `Recovered` are rejected without any
account change.

## Tournament (`TournamentStatus`)

| From | To | Instruction | Guard | Test |
|---|---|---|---|---|
| (none) | `Registration` | `initialize_tournament` | `sum(prize_shares) <= 10000`, valid player count | `tournament_registration_tests` |
| `Registration` | `Active` | `start_tournament` | `min_players` registered | `tournament_round_advancement_tests` |
| `Active` | `Completed` | `record_match_result` on the final match (elimination) | match participants verified | `tournament_match_tests` |
| `Active` | `Completed` | `complete_swiss_tournament` | all rounds played, places match standings, no duplicates | `tournament_swiss_completion_tests` |
| `Registration` / `Active` | `Cancelled` | `cancel_tournament` | authority + host treasury sign, all shards supplied, exact refund recipients | `tournament_cancel_tests` |
| `Completed` | `Closed` | `close_tournament` | no funded unclaimed place | `tournament_prize_tests` (indirect) |

Observations:

- `Cancelled` and `Closed` are terminal. `close_tournament` only flips the
  status; it does not close the account or sweep escrow remainder.
- Elimination brackets only populate places 1-4 (`record_result.rs:62-72`).
  `prize_shares` entries for places 5-10 are accepted at initialization but can
  never be paid in an elimination tournament, so those shares stay in escrow.
  Policy question, not changed.
- `initialize.rs:81` sums ten `u16` shares with `Iterator::sum`; with
  `overflow-checks = true` an overflowing input aborts the instruction rather
  than returning an error. No state change either way.
