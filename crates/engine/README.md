# Engine crates

The chess AI used by the game client (single-player and hint generation) and the
backend (analysis), plus its UCI test harness.

| Crate | Purpose |
|-------|---------|
| [`nimzovich_engine/`](nimzovich_engine/) | The engine itself: bitboard move generation, alpha-beta search with transposition tables and iterative deepening, PeSTO-style evaluation, opening book, perft, and PGN utilities. Feature-gated so the same crate serves the full `std` search build and the `no_std` on-chain move-generation build. |
| [`nimzovich-uci/`](nimzovich-uci/) | A minimal synchronous UCI protocol adapter (`triton-bench`-style binary) so the engine can play under cutechess-cli or any UCI GUI for strength testing and regression matches. |

## Feature boundary (critical)

`nimzovich_engine` compiles in two modes and the boundary must stay clean:

- **`std` + `search`** — full engine: search, evaluation, book, PGN. Used by `src/engine/`
  in the game client and by backend analysis.
- **default / `no_std`** — move generation and board representation only
  (`on_chain.rs`, `on_chain_moves.rs`, `on_chain_attack.rs`). This is the subset that
  `chess-logic-on-chain` re-exports into the Solana program, so nothing in these
  modules may touch the allocator beyond what the `alloc` feature deliberately allows.
