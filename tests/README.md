# Game-client tests (`xfchess` crate)

Integration tests for the Bevy game client. These compile against the real
`xfchess` crate API — **a test belongs in the crate it exercises**: backend tests
live in `backend/`, program tests under `programs/`, protocol/pairing tests in
their `crates/` crate. Do not import `backend`, `shared`, `shakmaty`, etc. here.

## Layout

```
tests/
├── lib.rs              # aggregator: runs the components/ and resources/ module tests
│                       #   (Cargo does not compile subdirectories as targets on their own)
├── components/         # game component usage (HasMoved, SelectedPiece, GamePhase, MoveRecord)
├── resources/          # game resources (ChessEngine moves, CapturedPieces, CurrentTurn, history)
├── core_tests.rs       # GameState state-machine transitions (needs StatesPlugin under Bevy 0.18)
├── protocol_roundtrip_tests.rs  # multiplayer wire-protocol serialization round-trips
├── systems_tests.rs    # ECS system test: reset_game_resources
├── types_tests.rs      # File / Rank / Square / Centipawns
└── integration_rollup.rs  # #![cfg(feature = "solana")] — only built with --features solana
```

## Running

```bash
cargo test -p xfchess                     # game-client tests (default features)
cargo test -p xfchess --features solana   # also builds the solana-gated targets
cargo test --workspace                    # everything

cargo test -p xfchess --test core_tests   # a single integration target
cargo test -p xfchess --lib short_label   # a single inline #[cfg(test)] unit test
```

## Writing a test here

- Use the real crate path: `use xfchess::game::...`, `use xfchess::engine::...`.
- Coordinates are `(file, rank)`, 0-indexed (e2 = `(4, 1)`), matching the codebase.
- A headless Bevy `App` that calls `init_state` must add `StatesPlugin` after
  `MinimalPlugins` (Bevy 0.18 dropped it from `MinimalPlugins`).
- Engine move queries need `engine.rebuild_legal_move_cache()` first —
  `ChessEngine::default()` starts with an empty cache.
- Keep tests deterministic: no wall-clock, network, or unseeded RNG.

## Real examples in this tree

- State machine: [`core_tests.rs`](core_tests.rs) — `test_state_transition_*`.
- Legal move generation: [`resources/engine_tests.rs`](resources/engine_tests.rs).
- Material counting: [`resources/captured_tests.rs`](resources/captured_tests.rs).
- ECS system reset: [`systems_tests.rs`](systems_tests.rs).
