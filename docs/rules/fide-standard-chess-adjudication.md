# Standard chess adjudication: implementation record

Status: **not release ready**. This record accompanies the implementation branch;
it does not certify the existing product as FIDE compliant.

## Sources and baseline (2026-10-31)

- Application: `trilltino/XFChess`, freshly fetched `main`,
  `40e78cb4308303b4f07f991d835de5205d6e2dc8`.
- Engine: `trilltino/xfchess-contracts`, freshly cloned default head,
  `d8dea0a9d41c0a401add75cb22b580128360700b`. Application engine pin:
  `08e430699297f7d13f94fae32128fc0bafedf778`.
- The contracts README identifies XFChess as canonical for the program and
  extracted clients/rules. The engine is canonical in contracts. Do not overwrite
  the newer contracts security changes with the application's old engine pin.
- [FIDE Laws effective 1 January 2023](https://handbook.fide.com/chapter/e012023),
  rechecked on the date above: Articles 3–6 and 9 govern this change.
- Selected dependencies: shakmaty 0.30.2, Anchor libraries 1.2.1, solana-program
  5.1.0, solana-sdk 5.0.0, Agave client/test 4.3.2, ephemeral-rollups-sdk 0.17.3,
  MagicBlock magic-program API 1.1.1. Preserve these pins.
- Host AI uses Stockfish through `xfchess-stockfish`; Nimzovich full search is
  excluded from application dependencies by `scripts/ci/check_engine_dependencies.ps1`.

## Authority and change inventory

The initial inventory searched all tracked source, scripts, manifests and IDLs
for the assignment's result, history, move, timeout and settlement terms.

| Mode / surface | Current writers and readers | Required authority |
| --- | --- | --- |
| Human/AI and offline play | `src/game/ai/systems/mod.rs`, `src/input/board.rs`, `src/game/systems/shared.rs`, `src/game/systems/game_logic.rs` | Session-scoped legal history and clocks; no network claim requirement |
| Casual online | `braid_chess/src/message.rs`, `src/multiplayer/network/{protocol,braid_transport}.rs`, `braid_transport/draws.rs`, `network_move.rs` | Authenticated seats, causal move log and durable claim decision |
| Rated / wagered | `src/multiplayer/solana/{chain_actions,lobby/game_actions}.rs`, `rollup/bridge/{move_relay,settlement}.rs` | Confirmed Game account; client evaluation is advisory |
| ER / move sessions | `programs/xfchess-game/src/moves_ix/{record,global_record,apply}.rs`, `game_ix/record.rs`, delegation and magicblock modules | Validated atomic program transition, then commit and undelegate |
| Swiss / elimination | `programs/xfchess-game/src/tournament_ix/`, `crates/tournament/`, backend tournament routes, Tauri admin | Canonical result feeds score, progression and prizes exactly once |
| Spectator | `src/multiplayer/spectator.rs`, `game/systems/spectate_sync.rs`, backend `routes/spectate.rs` | Read-only verified history and final result |
| Replay / archive | `src/game/replay/`, popup PGN, backend `game_pgn.rs`, rules `pgn.rs`, tournament admin | Read-only reconstruction, typed termination metadata |
| Puzzles / tutorial | `src/puzzle/`, scripted demo and training consumers | Live claims and wager controls not applicable; visual loops are not accepted moves |

### Accepted move and recovery boundaries

Local input, promotion completion, premoves, AI and validated remote moves
converge on `src/game/systems/shared.rs::execute_move`. Its ECS mutations,
`MoveHistory`, FEN refresh and `MoveMadeEvent` precede animation completion.
Remote `network_move.rs` validates turn/legality/FEN and acknowledges applied
versions through Braid. Render frames, `ChessEngine::refresh_position`, ECS
resynchronization and lost-ACK redelivery must not append positions.

The rollup bridge stages/batches moves, advances nonces and reconciles canonical
inclusion. Chain repetition may advance only inside `apply_recorded_move`,
after signer/session, turn, nonce, legality and next-board validation. Local
predictions cannot finalize a payable result.

`game_init.rs`, `board_snapshot.rs`, `session_restore.rs`, `resume.rs`,
`casual_resume.rs`, replay seek and spectator catch-up are reconstruction/reset
boundaries. A final FEN or checkpoint alone proves no earlier occurrences.
Recover history only by legal ordered replay with checked FENs and game identity.
Missing history must disable historical claims, especially wagered claims.
The AI currently sends start-position move history when it reconstructs the
current FEN, and falls back to current FEN otherwise; that fallback loses
repetition knowledge and needs an explicit incomplete-history state.

### Serialization and consumers

`state/game.rs::Game` is an Anchor `InitSpace` account, allocated by normal,
global, tournament match and tournament-session constructors. It has no version,
repetition history, per-player balances, claim sequence or termination reason.
`GameResult` is `None`, `Winner(Pubkey)`, `Draw`; preserve discriminants.

`services/backend/src/signing/solana/game_account.rs` manually decodes variable Borsh
fields and offsets. Handwritten clients include
`src/solana/program_interface/instructions.rs`, benchmark builders, Tauri wallet
transaction descriptions and the website JSON IDL. Contract ABI, client-layout
and instruction-layout tests and the contracts extraction must agree.
An appended Rust field alone is not a migration. Existing undersized accounts
and active delegated accounts require an explicit versioned lifecycle.

`lifecycle/settlement.rs`, backend `tasks/settlement_worker.rs`, Elo/profile,
tournament scoring, archive, spectator and UI consumers must consume the same
confirmed outcome. Keep termination reason separate from payout outcome.
`services/backend/src/signing/routes/game_log.rs` uses a SQLite durable event log and a
participant-kind allowlist; claims and adjudication events need explicit gates,
causal versions and idempotency. A POST or peer terminal message cannot decide
chain settlement.

## Strict rules and online procedure

Three appearances allow a claim by the side to move, including a declared legal
intended move; five appearances end play automatically. At 100 reversible plies
there is a claim, not automatic termination; at 150 there is automatic termination
unless the last move checkmates. Current and intended-move claims are distinct
from mutual agreement. Both players must have moved before agreement can end play.

Online deliberate move submission is the Article 9.4 commitment boundary.
Selection, hover, drag preview and animation do not waive a claim. A formal
intended move is irrevocable. Authenticated adjudication replaces calling an
arbiter; its durable receipt pauses authoritative clocks. A correct claim ends
the game on the current board. An incorrect substantive claim adds 120 seconds
to the opponent and binds a declared legal move. Malformed, unauthorized, stale,
duplicate and transport-failed submissions are not new penalizable claims.
Claims also preserve the draw-offer semantics of Article 9.1.2.3.

Submitting a move atomically completes the digital move and switches clocks;
an attached offer implements the physical offer-before-clock-press procedure.
An out-of-turn offer remains an offer, subject to abuse controls. Durable signed
receipts replace scoresheet entries; PGN carries the outcome and reason.

## Timing and release risks

The current contract has **no per-side clock balances**. `lifecycle/clock.rs`
uses base time plus theoretical increments, with a 90-second minimum. This is
an inactivity bound, not a FIDE chessclock. Displayed Bevy time, signed move
timestamps, backend receipt time and Solana `Clock` cannot be interchanged.
Claims require an explicit authenticated clock authority, remaining-time storage,
increment accounting, monotonic timestamps, pause/penalty and restoration proof.

| Risk / correction | Evidence and next gate |
| --- | --- |
| Incorrect automatic 100-ply draw | `moves_ix/apply.rs`, `game_logic.rs`, test `hundred_quiet_halfmoves_end_the_game_as_the_contract_does`; replace in coordinated upgraded paths |
| No authoritative repetition | Game has only current compact board; add bounded canonical history and exact equality, not search hash |
| Unsafe promotion aliases | Engine `parse_uci` treats unknown suffix as absent and internal apply defaults queen; add rejection and differential tests |
| One-sided mate impossibility | Resign and timeout always award winner; existing tests use empty fake boards; introduce legal asymmetric fixtures |
| General dead positions | Material tests are sound only for a subset; exotic locked positions require verified proof strategy; strict release blocker until covered |
| Early draw agreement | `finish_by_draw_agreement` permits zero/one-ply completion; test Article 5.2.3 |
| Layout / ER growth | No schema version today; measure CU, stack/heap, bytes, rent and delegation before selecting history storage |
| Peer / result forgery | Do not treat unverified GameOverState or game-log write as rated/financial authority |
| Reconnect and duplicate application | Test legal replay, nonce/claim sequence, lost ACK, stale game, crash and claim/move races |

Implementation order: engine legality and canonical identities, shared bounded
adjudication primitives and fixtures; measured SBF storage and versioned migration;
authoritative program claims/clocks; host/network/backend integration; UI/PGN;
recovery, settlement and end-to-end proof. No merge, deployment, funding or use
of wallet key material is part of this work.
