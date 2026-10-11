# src/bin

Auxiliary developer binaries that ship alongside the game target. All Solana-touching
tools have `required-features = ["solana"]` in the root [Cargo.toml](../../Cargo.toml).

## Binaries

| Binary | File | Purpose |
|--------|------|---------|
| `debugger` | [debugger.rs](debugger.rs) | Local log/debug stream inspector |
| `pda` | [pda.rs](pda.rs) | Derive and print program PDAs (game, profile, vaults) |
| `profile_pda` | [profile_pda.rs](profile_pda.rs) | Inspect a `PlayerProfile` PDA |
| `read_game` | [read_game.rs](read_game.rs) | Fetch and decode a `Game` account |
| `tournament_test` | [tournament_test.rs](tournament_test.rs) | Drive tournament flows against devnet |

## Example

```bash
cargo run --bin debugger
cargo run --bin pda --features solana
cargo run --bin read_game --features solana -- <GAME_ID>
```

## Gotchas

- Check each tool's endpoint, program ID and key-loading behavior before use;
  do not assume it uses the backend environment.
- Maintained cost/capacity workflows live in [cu_benchmark](../../crates/cu_benchmark/README.md).
