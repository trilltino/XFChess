# chess-logic-on-chain

**no_std** chess move validation for the Solana program. This crate is the bridge
between `nimzovich_engine`'s allocation-light core and the on-chain `record_move`
instruction: with the program's `move-validation` feature enabled, every move is
verified legal on-chain within compute limits.

## Surface

A thin `#![no_std]` re-export layer over `nimzovich_engine`'s on-chain subset:

- `OnChainGame`, `CompactBoard` — the 68-byte packed board representation stored in
  the `Game` account.
- `validate_and_apply(game, uci)` — legality check + board mutation in one pass.
- `parse_uci` — `[u8; 5]` UCI decoding (4 chars + optional promotion piece).
- Piece/color constants (`PAWN_ID` … `KING_ID`, `Color`) and the `Game`/`Move` types.
- `validation` module — additional on-chain validation helpers.

## The one rule

**This crate must remain `no_std`.** It compiles into the Solana program; any
transitive `std` dependency breaks the on-chain build. Data layout goes through
`bytemuck`-style plain-old-data; nothing here may touch the allocator beyond what
`nimzovich_engine`'s `alloc` feature deliberately allows.

## Consumers

- `programs/xfchess-game` (behind its `move-validation` feature) — on-chain legality.
- Test suites (`programs/xfchess-game/tests/`) — reuse the same code as their oracle,
  so the test expectation can never diverge from the on-chain validator.
