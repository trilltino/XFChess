# Game client (`src/`)

The XFChess game client: a Bevy 0.20 ECS application with a 3D (and 2D top-down)
chess board, AI, puzzles, multiplayer (Iroh P2P + Braid), and optional Solana
integration. `main.rs` is the binary entry, `lib.rs` assembles the app, and the repo
root's `build.rs` copies assets and injects the backend URL at compile time.

## Module map

| Module | Responsibility |
|--------|----------------|
| [`core/`](core/) | `GameState`/`GameMode`, frame order (`schedule.rs`), window, crash reports, settings, update checks, background I/O (`net_task.rs`) |
| [`states/`](states/) | One plugin per screen: main menu (`main_menu/menu`, `main_menu/screens`), pause, game over, settings, tournament menu |
| [`game/`](game/) | Turn flow, move execution, clocks, game-over detection, AI (`ai/`), PGN replay (`replay/`), board sync |
| [`engine/`](engine/) | Board position resource over the `nimzovich_engine` crate |
| [`input/`](input/) | Board picking observers (`board.rs`), hover cursor (`pointer.rs`), in-game keys |
| [`rendering/`](rendering/) | 3D board, pieces, cameras, lighting, move feedback (reads game state only) |
| [`presentation/`](presentation/) | Audio volume and mute |
| [`ui/`](ui/) | Egui widgets: in-game HUD (`game/game_ui`, `game/game_2d`), menus and popups, styles |
| [`puzzle/`](puzzle/) | Server-checked puzzles: rated, daily, Lichess-linked and SOL bounties |
| [`multiplayer/`](multiplayer/) | Backend client (`network/vps`), P2P mesh (`systems/`), Braid transport, spectating, resume; Solana flows under `solana/` and `rollup/` |
| [`solana/`](solana/) | Program instruction builders and session keys — **only with `--features solana`** |
| [`xf_animate/`](xf_animate/) | The animated mini board in the menu's LEARN box |
| [`assets/`](assets/) | Asset preloading and loading progress |
| [`android/`](android/) | Android entry point and platform glue |
| [`bin/`](bin/) | Developer tools (`pda`, `debugger`, …) |

Every module root has a `//!` header describing what it owns; READMEs cover
cross-cutting details.

## Frame order

`core::schedule` configures `Update` once; every system belongs to one of these
phases, and systems inside a phase are chained:

```
NetworkSet::Receive   Transport → Invites → Relay → Game → Feeds
NetworkSet::Session   Wallet → Lobby → Match → Tournament → Rollup → Delegation → Social → Puzzle
GameSystems           Input → Validation (MoveSource: Replay → Resume → Spectator → Puzzle)
                      → Execution → Visual                                   (InGame only)
MenuSystems           Main → LearnBoard                                       (MainMenu only)
ApplySettings         settings → audio, cameras, lights, themes
NetworkSet::Send      Moves → Session → Ack → RollupFeed → Settlement
```

A remote move received this frame is applied this frame, and outgoing reports see
the board after it. Egui drawing happens later, in `EguiPrimaryContextPass`. Debug
builds warn at startup about any two `Update` systems that share data without a
defined order — there are none, so a warning means a new system needs a phase.

## State lifetimes

- `GameState`: `MainMenu → InGame → GameOver`; `InGameplay` (computed) covers
  `InGame` and `GameOver` and only fires `OnEnter`/`OnExit` when gameplay really
  starts or ends.
- Menu entities carry Bevy's `DespawnOnExit(GameState::MainMenu)`; the game scene
  carries `core::game_scene()` (kept behind the game-over popup, rebuilt on re-entry).
- Systems are gated by their phase's state or by Bevy run conditions
  (`resource_changed`, `on_message`, `input_just_pressed`, `on_timer`) rather than
  early returns.

## Conventions

- **Blocking HTTP** (`reqwest::blocking`) never runs in a system or on Bevy's task
  pools: use `core::net_task::{spawn_detached, Pending}`, or the multiplayer Tokio
  runtime for async work.
- **Run conditions:** prefer Bevy's built-ins (`in_state`, `resource_equals`,
  `resource_changed`) over hand-written closures.
- **Feature flag `solana`** gates all on-chain code. Never import Solana types
  outside gated code; the default build must compile without the Solana SDK.

## Running

```bash
cargo run                     # offline / AI / casual play / puzzles
cargo run --features solana   # with on-chain integration
```
