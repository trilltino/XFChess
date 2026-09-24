# src/solana

Client-side Solana integration for the Bevy game. Compiled only with
`--features solana`; the default build must never import anything from here.

Program ID: `8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU`
([program_interface/instructions.rs](program_interface/instructions.rs)).

## Role In XFChess

For staked/ranked games the client records every move on-chain. This module holds
instruction encodings, PDA helpers, and the session-key bootstrap used by that
path. Higher level wallet, lobby, tournament, and recovery flows live in
[`src/multiplayer/solana/`](../multiplayer/solana/).

```
game/ (move made) -> multiplayer/rollup/bridge.rs -> vps_client (HTTP) -> backend picks base RPC or Magic Router
```

The native client does not pick between base RPC and the ER itself for gameplay
writes. It hands the move to the backend's VPS signing API
(`crate::multiplayer::vps_client::record_move`), which decides base vs. Magic
Router. See [MAGICBLOCK.md](../../MAGICBLOCK.md).

## Key Files

| File | Contents |
|------|----------|
| [mod.rs](mod.rs) | `SolanaPlugin` and stable `crate::solana::instructions::*` re-export |
| [program_interface/instructions.rs](program_interface/instructions.rs) | Instruction builders, PDA seeds, and `PROGRAM_ID` |
| [session/mod.rs](session/mod.rs) | `SessionPlugin` and session-key lifecycle inside ECS |

## Removed Legacy Files

The old `core/`, `multiplayer/`, `wallet/`, top-level `errors.rs`, top-level
`state.rs`, and `program_interface/state.rs` files were unused or empty and were
removed. Account-state mirrors should come from the active on-chain program or a
generated client instead of stale duplicate local enums.

## Invariants

- Keep all Solana SDK imports behind the `solana` feature gate.
- The program ID here must match `declare_id!` in
  [programs/xfchess-game/src/lib.rs](../../programs/xfchess-game/src/lib.rs).
- Do not send ER writes (moves, undelegate) to base RPC or vice versa. The
  backend owns that decision; see [MAGICBLOCK.md](../../MAGICBLOCK.md).
