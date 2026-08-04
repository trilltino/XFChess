# src/engine

Bevy-side wrapper around the [`nimzovich_engine`](../../crates/engine/nimzovich_engine/)
crate. Holds the authoritative board position as a FEN string inside an ECS resource
and answers move-generation / legality questions for the rest of the client.

## Role in XFChess

Every mode consults this module for rules: single-player asks it for legal moves and
AI replies, multiplayer validates incoming remote moves against it, and the Solana
path uses the same `nimzovich_engine` move representation that
`chess-logic-on-chain` validates on-chain — so client and program can never disagree
about legality.

## Key files

| File | Contents |
|------|----------|
| [board_state.rs](board_state.rs) | `ChessEngine` resource: FEN + internal `nimzovich_engine::Game`, per-turn legal-move cache, castling/en-passant/halfmove bookkeeping, ECS→engine sync |

## Example

```rust
use nimzovich_engine::{game_from_fen, generate_pseudo_legal_moves, is_legal_move};

// board_state.rs — ChessEngine keeps FEN and engine state in lockstep
#[derive(Resource)]
pub struct ChessEngine {
    pub fen: String,               // updated after every move
    game: Game,                    // nimzovich_engine internal state
    move_cache: HashMap<(u8, u8), Vec<(u8, u8)>>, // legal moves, rebuilt once per turn
    // …
}
```

## Gotchas

- The legal-move cache is only valid for the current turn; `synced_this_move` /
  `move_cache_valid` exist to stop `update_game_phase` from re-syncing every frame.
  If you mutate piece positions outside `execute_move`, invalidate the cache.
- AI search itself lives in `nimzovich_engine` (`std` + `search` features); this
  module is only the ECS-facing board state.
