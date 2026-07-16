# src/game

Core chess gameplay: turn flow, move execution, time controls, AI hookup, replays,
and board synchronization. `GamePlugin` ([plugin.rs](plugin.rs)) wires everything into
ordered system sets.

## Role in XFChess

This is the hub between input and output: [`input/`](../input/) selects pieces here,
[`engine/`](../engine/) answers legality questions, `multiplayer/` and `solana/`
inject remote moves through [systems/network_move.rs](systems/network_move.rs), and
[`rendering/`](../rendering/) + [`presentation/`](../presentation/) draw the results.

## Layout

| Path | Contents |
|------|----------|
| [plugin.rs](plugin.rs) / [system_sets.rs](system_sets.rs) | `GamePlugin`; `GameSystems` sets ordered `Input → Validation → Execution → Visual` |
| [systems/](systems/) | Move input, game init/logic, promotion, network-move apply, spectator sync, camera |
| [components/](components/) | `Piece` markers, `GamePhase`, `MoveRecord`, animation components |
| [resources/](resources/) | Turn state ([turn/](resources/turn/)), selection/promotion ([player/](resources/player/)), move history ([history/](resources/history/)), sounds, time control |
| [ai/](ai/) | AI opponent plugin — drives `nimzovich_engine` search off the main thread |
| [sync/](sync/) | `GameSyncPlugin` — keeps ECS pieces and `ChessEngine` FEN consistent |
| [replay.rs](replay.rs) / [replay_braid.rs](replay_braid.rs) / [replay_shorts.rs](replay_shorts.rs) | PGN replay, Braid-stream replay, and short-clip capture |
| [time_control.rs](time_control.rs) | Clock + Fischer increment logic |
| [events.rs](events.rs) | `GameStartedEvent`, `GameEndedEvent`, move events |

## Example

```rust
// plugin.rs — system ordering is explicit; new gameplay systems join a set
app.configure_sets(
    Update,
    (
        GameSystems::Input,      // camera, piece selection
        GameSystems::Validation, // validate moves, sync board state
        GameSystems::Execution,  // execute moves, update game state
        GameSystems::Visual,     // highlights, animations
    )
        .chain()
        .run_if(in_state(GameState::InGame)),
);
```

## Gotchas

- Never mutate board state outside the `Execution` set — `sync/` assumes ECS and
  `ChessEngine` only diverge inside one frame.
- Remote (network) moves must enter via `systems/network_move.rs` so they run the
  same validation as local input.
