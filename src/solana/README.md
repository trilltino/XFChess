# src/solana

Client-side Solana integration for the Bevy game. Compiled **only** with
`--features solana`; the default build must never import anything from here.
Program ID: `8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU` ([constants.rs](constants.rs)).

## Role in XFChess

For staked/ranked games the client records every move on-chain. This module holds
the constants, instruction encodings, and routing logic for that path; the higher
level flows (wallet sessions, lobby, tournaments) live in
[`src/multiplayer/solana/`](../multiplayer/solana/) and heavy transaction building is
delegated to the [`solana-chess-client`](../../crates/solana/solana-chess-client/) crate.

```
game/ (move made) ─► multiplayer/solana (session sign) ─► routing.rs ─► base RPC or Magic Router (ER)
```

## Key files

| File | Contents |
|------|----------|
| [constants.rs](constants.rs) | Program ID, PDA seeds (`b"game"`, `b"player"`), timeouts |
| [routing.rs](routing.rs) | `TxRoute::{Base, MagicRouter}` — where to send a write, based on delegation state |
| [program_interface/](program_interface/) | Instruction builders ([instructions.rs](program_interface/instructions.rs)) and account state mirrors ([state.rs](program_interface/state.rs)) |
| [session/](session/) | `SessionPlugin` — session-key lifecycle inside the ECS |
| [wallet/](wallet/) | Phantom deep-link signing ([phantom_sign.rs](wallet/phantom_sign.rs)) |
| [core/](core/) | Shared constants and error types |
| [multiplayer/](multiplayer/) | **Legacy, unused** — scheduled for removal (see [docs/legacy-cleanup-audit.md](../../docs/legacy-cleanup-audit.md)) |

## Example

```rust
// routing.rs — delegated games write to the ER via the Magic Router
pub fn route_for_game_write(is_delegated: bool) -> TxRoute {
    if is_delegated {
        TxRoute::MagicRouter
    } else {
        TxRoute::Base
    }
}
```

## Invariants

- Keep all Solana SDK imports behind the `solana` feature gate.
- The program ID here must match `declare_id!` in
  [programs/xfchess-game/src/lib.rs](../../programs/xfchess-game/src/lib.rs).
- Route writes through `route_for_game_write` — sending an ER write to base RPC (or
  vice versa) fails with owner mismatches. See [MAGICBLOCK.md](../../MAGICBLOCK.md).
