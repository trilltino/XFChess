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
game/ (move made) ─► multiplayer/rollup/bridge.rs ─► vps_client (HTTP) ─► backend picks base RPC or Magic Router
```

The native client does not pick between base RPC and the ER itself for gameplay
writes — it hands the move to the backend's VPS signing API
(`crate::multiplayer::vps_client::record_move`), which is what actually decides
base vs. Magic Router (see [MAGICBLOCK.md](../../MAGICBLOCK.md)). This module's
own [multiplayer/rollup/magicblock.rs](../multiplayer/rollup/magicblock.rs)
`MagicBlockResolver` is used only to build the `delegate_game` instruction
(signed directly by the wallet) and to track local delegation status for the UI.

## Key files

| File | Contents |
|------|----------|
| [constants.rs](constants.rs) | Program ID, PDA seeds (`b"game"`, `b"player"`), timeouts |
| [program_interface/](program_interface/) | Instruction builders ([instructions.rs](program_interface/instructions.rs)) and account state mirrors ([state.rs](program_interface/state.rs)) |
| [session/](session/) | `SessionPlugin` — session-key lifecycle inside the ECS |
| [wallet/](wallet/) | Phantom deep-link signing ([phantom_sign.rs](wallet/phantom_sign.rs)) |
| [core/](core/) | Shared constants and error types |
| [multiplayer/](multiplayer/) | **Legacy, unused** — scheduled for removal (see [docs/legacy-cleanup-audit.md](../../docs/legacy-cleanup-audit.md)) |

## Invariants

- Keep all Solana SDK imports behind the `solana` feature gate.
- The program ID here must match `declare_id!` in
  [programs/xfchess-game/src/lib.rs](../../programs/xfchess-game/src/lib.rs).
- Do not send ER writes (moves, undelegate) to base RPC or vice versa — this
  fails with owner mismatches. The backend owns that decision; see
  [MAGICBLOCK.md](../../MAGICBLOCK.md).
