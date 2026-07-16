# src/multiplayer

Online play for the Bevy client: WebSocket auth against the backend, Iroh QUIC P2P
transport, Braid HTTP-209 state subscriptions, and (behind `--features solana`) the
on-chain wager/session flows. `MultiplayerPlugin` in [mod.rs](mod.rs) owns a shared
Tokio runtime and registers all sub-plugins.

## Role in XFChess

```
game/ (local move) ─► systems.rs ─► network/ ── Iroh QUIC ──► opponent client
                                     │        ── Braid 209 ──► backend relay (state sync)
                                     └─► network/vps/ ──HTTP──► backend (auth, matchmaking, move record)
solana feature:      rollup/ + solana/ ──► backend /session/* ──► MagicBlock ER
```

WebSockets carry auth/signaling; Braid subscriptions carry board/tournament state —
keep the two distinct (see [crates/zarathustra_net/](../../crates/zarathustra_net/README.md)).

## Submodules

| Module | Responsibility |
|--------|----------------|
| [network/](network/) | Transports: Iroh P2P ([p2p.rs](network/p2p.rs)), live session ([online_game_session.rs](network/online_game_session.rs)), relay bridge, backend HTTP client ([vps/](network/vps/)), wire protocol ([protocol.rs](network/protocol.rs)) |
| [rollup/](rollup/) *(solana)* | MagicBlock ER client side: delegation bridge, session keys |
| [solana/](solana/) *(solana)* | Wallet + session-key managers, lobby, tournaments, Tauri signer bridge |
| [wager_state/](wager_state/) *(solana)* | Escrow/payout state machine + UI |
| [tournament/](tournament/) | Tournament client + events |
| [auth_ws.rs](auth_ws.rs) | WebSocket auth handshake with the backend |
| [social.rs](social.rs) / [spectator.rs](spectator.rs) | Friends/chat, spectating |
| [join_link.rs](join_link.rs) | Shareable game join links |
| [ui/](ui/) | Spectator overlay, transaction debugger |

## Example

```rust
// mod.rs — older call sites use the vps_client alias for the backend HTTP client
pub mod vps_client {
    pub use super::network::vps::*;
}

// network/vps/game.rs et al. drive the backend endpoints:
// POST /session/create, /move/record, /game/undelegate, /game/finalize
```

## Gotchas

- All async work goes through the shared `TokioRuntime` resource — don't create
  per-system runtimes.
- `network/braid.rs` is a legacy shell slated for removal
  ([docs/legacy-cleanup-audit.md](../../docs/legacy-cleanup-audit.md)); live transport
  is `OnlineGameSession`.
- `rollup/`, `solana/`, and `wager_state/` are `#[cfg(feature = "solana")]` — never
  import them unconditionally.
