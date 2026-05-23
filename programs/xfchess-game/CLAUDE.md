# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

The `xfchess-game` Anchor 0.31 program is the on-chain authority for all game state. It stores game boards as packed FEN (`[u8; 68]`) and moves as UCI bytes (`[u8; 5]`). Tournament prize escrow and ELO ratings also live here.

**Program ID**: `8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU` (localnet + devnet)

## Build

```bash
# From repo root — size-optimized (opt-level="z")
scripts\build_program.bat
# or
anchor build

# Run all tests
cargo test -p xfchess-game

# Single test file
cargo test -p xfchess-game --test smoke_tests
cargo test -p xfchess-game --test security_tests
cargo test -p xfchess-game --test fee_model_tests
cargo test -p xfchess-game --test game_tests
```

## Instruction groups

Each group is a subdirectory under `src/`. Instruction handlers are thin — they call into the module's `handler` function. `lib.rs` routes all Anchor `#[program]` entry points and re-exports account structs at the crate root (required by Anchor's macro codegen).

| Module | What it does |
|--------|-------------|
| `account_ix/` | Profile init, fee vault, ELO update, session key CRUD |
| `game_ix/` | create / join / cancel / resign / timeout / finalize |
| `moves_ix/` | `record_move` — the hot path, runs on Ephemeral Rollups |
| `delegation_ix/` | Delegate game accounts to ER; session key auth; undelegation callback |
| `tournament_ix/` | Full lifecycle: initialize → shards → escrow → register → start → match results → prizes; also tournament-scoped sessions |
| `governance_ix/` | Dispute, resolve, claim stale dispute |
| `crank_ix/` | MagicBlock-scheduled time checks (feature `cranks`) |
| `elo/` | Glicko-2 rating math |
| `state/` | Account structs: `Game`, `PlayerProfile`, `Tournament`, `SessionDelegation`, etc. |

## Ephemeral Rollups (ER) lifecycle

1. **Delegate**: `delegate_game` moves the `Game` PDA to MagicBlock ER. After this point the account lives off-chain on the EU devnet rollup.
2. **Play**: `record_move` is called on the ER (sub-second latency). Move validation runs via `chess-logic-on-chain` if the `move-validation` feature is active.
3. **Undelegate**: `undelegate_game` schedules undelegation. The ER infrastructure calls `process_undelegation` automatically, which calls the `undelegate_account` CPI from `ephemeral-rollups-sdk`.
4. **Finalize**: `finalize_game` settles wagers and updates ELO.

Never call `record_move` on mainnet Solana — it must go through the ER RPC endpoint.

## Feature flags

| Feature | Effect |
|---------|--------|
| `default` | `cranks` + `move-validation` |
| `move-validation` | Pulls in `chess-logic-on-chain` to validate moves on-chain |
| `cranks` | Enables `crank_ix/` + MagicBlock crank API |

Disable `move-validation` only for local testing where you want to inject arbitrary board states.

## Anchor codegen quirk

Anchor 0.32 generates `pub use crate::__client_accounts_<snake>::*` at the crate root for every instruction accounts struct. Because the derive macro produces `pub(crate)` modules inside submodules, they can't be directly re-exported. `lib.rs` contains `pub mod __client_accounts_*` thin wrappers that re-export via `pub use` — do not remove them.

## Adding a new instruction

1. Create `src/<group>_ix/<name>.rs` with accounts struct + handler.
2. Re-export the accounts struct in `lib.rs` both in the `pub use` block and as a `pub mod __client_accounts_<snake>` wrapper.
3. Add the entry point in the `#[program]` block in `lib.rs`.
4. Add a corresponding test in `tests/`.

## Size constraint

The program uses `opt-level = "z"` in the workspace profile. Avoid pulling in large dependencies. Prefer `bytemuck` for zero-copy deserialization over anything that allocates. The deploy binary must fit within Solana's program size limit (~1.2 MB post-strip).
