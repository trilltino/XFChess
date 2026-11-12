# Standard chess adjudication primitives

This allocation-free `no_std` crate is the canonical shared definition for the
FIDE rules being integrated into XFChess. It is **not yet a live-game feature**.
It has no signing, network, account, wallet, UI or settlement authority.

- `PositionKey` v1 is an exact 34-byte encoding, with no fingerprint collisions.
  The optional `shakmaty` feature supplies the host adapter. The Nimzovich
  `on_chain_rules` adapter validates compact boards and normalizes legal EP.
- `RepetitionTracker` borrows caller-owned bounded storage. The starting position
  counts. Pawn moves, captures and lost castling rights invalidate older
  repetition history; only pawn moves/captures reset the halfmove clock. Rejected
  updates are atomic. Restore accepts an authenticated checkpoint, not peer proof.
- Threefold and 100 reversible halfmoves are claim eligibility. Fivefold and 150
  reversible halfmoves are automatic; checkmate takes precedence at 150.
- `AdjudicatedClock` uses integer milliseconds from an authenticated monotonic
  source chosen by the mode adapter. Browser timestamps are not authoritative.
- `ClaimProcedure` follows durable formal receipt: pause, consume a replay nonce,
  then resolve once. An incorrect claim adds 120 seconds and binds any legal
  intended move. A claim is also an offer. Malformed/stale envelopes do not
  change state. There is no withdrawal operation.

The mode adapter must authenticate the claimant's seat, bind the exact game and
ply, reject automatic/finished positions, validate the intended move before
receipt, derive eligibility from its own legal history, and atomically persist
the procedure, clocks, board and result. A successful prospective claim does
not play the intended move. On an incorrect one, enforce the obligation before
accepting the next move and preserve the claim's draw offer for the opponent.
Checkpoints and all historical starting positions must come from trusted
creation/replay; a lone FEN cannot prove earlier repetitions.

`nimzovich_engine::on_chain_replay::record_verified_move` supplies a transaction
for the chess position/history portion of that adapter, checking each legal move
and exact compact post-board. It does not authenticate sessions or touch funds.

The engine's directional mating test proves only sound material cases and
returns `Unproven` otherwise. The locked bishop/pawn-wall fixture demonstrates
why this is not a complete dead-position solver. Treat that gap as a strict-FIDE
release blocker. Never interpret `Unproven` as evidence that a timeout win is
correct.

The maximum reversible key window is 151 * 34 = 5,134 bytes. The API borrows this
storage to avoid constructing a large stack temporary. This is **not a selected
Game account layout**: serialization metadata, rent, SBF runtime CU/heap, migration
and ER delegation measurements remain required. No deployed ABI has changed.

Run:

```text
cargo test -p nimzovich_engine -p xfchess-adjudication --locked
cargo check -p nimzovich_engine -p xfchess-adjudication --no-default-features --locked
cargo clippy -p xfchess-adjudication --all-targets --all-features --locked -- -D warnings
cargo fmt -p nimzovich_engine -p xfchess-adjudication -- --check
```

The engine's search TT now includes castling rights and raw EP in its search
hash, including make/unmake and null moves. Raw EP can conservatively split
otherwise equivalent search positions; it is not the authoritative key. Search
twofold pruning remains a heuristic and never awards a draw.
