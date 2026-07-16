# src/bin

Auxiliary developer binaries that ship alongside the game target. All Solana-touching
tools have `required-features = ["solana"]` in the root [Cargo.toml](../../Cargo.toml).

## Binaries

| Binary | File | Purpose |
|--------|------|---------|
| `debugger` | [debugger.rs](debugger.rs) | Transaction/game-state inspector (no Solana feature needed) |
| `pda` | [pda.rs](pda.rs) | Derive and print program PDAs (game, profile, vaults) |
| `profile_pda` | [profile_pda.rs](profile_pda.rs) | Inspect a `PlayerProfile` PDA |
| `read_game` | [read_game.rs](read_game.rs) | Fetch and decode a `Game` account |
| `tournament_test` / `tournament_real_test` | [tournament_test.rs](tournament_test.rs), [tournament_real_test.rs](tournament_real_test.rs) | Drive tournament flows against devnet |
| `tournament_data_gen` | [tournament_data_gen.rs](tournament_data_gen.rs) | **Stale** — fails against current instruction signatures; removal candidate ([docs/legacy-cleanup-audit.md](../../docs/legacy-cleanup-audit.md)) |
| — | [on_chain_benchmark.rs](on_chain_benchmark.rs) | **Stale** — superseded by [crates/solana/er-cu-benchmark](../../crates/solana/er-cu-benchmark/); removal candidate |

## Example

```bash
cargo run --bin debugger
cargo run --bin pda --features solana
cargo run --bin read_game --features solana -- <GAME_ID>
```

## Gotchas

- These tools hit **devnet** by default via the same constants as the game client
  ([src/solana/constants.rs](../solana/constants.rs)); they do not read the backend's
  `.env`.
