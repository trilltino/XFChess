# Game client (`src/`)

The XFChess desktop game client: a Bevy 0.18 ECS application rendering an isometric 3D
chess board, with AI, multiplayer (Iroh P2P + Braid relay), and optional Solana
integration. Entry points are `main.rs` (binary) and `lib.rs` (module tree + app
assembly); `build.rs` at the repo root copies assets and injects the backend URL at
compile time.

## Module map

| Module | Responsibility |
|--------|----------------|
| [`core/`](core/) | App lifecycle, crash reporting, the `AppState` flow (`Splash → MainMenu → Game → Pause`) |
| [`states/`](states/) | State types and transition wiring |
| [`game/`](game/) | Board state, FEN management, move validation, check/checkmate detection |
| [`engine/`](engine/) | AI opponent — delegates to the `nimzovich_engine` crate |
| [`input/`](input/) | Picking, drag/drop, keyboard input |
| [`rendering/`](rendering/) | Isometric 3D board, piece meshes, `graphics_quality.rs` settings |
| [`presentation/`](presentation/) | Visual feedback layered on top of rendering (highlights, animations) |
| [`ui/`](ui/) | Menus, HUD, dialogs |
| [`multiplayer/`](multiplayer/) | WebSocket auth + Iroh P2P relay + Braid game-state subscriptions |
| [`puzzle/`](puzzle/) | Server-verified tactics puzzle mode |
| [`xf_animate/`](xf_animate/) | Self-contained chess showcase animation on the main menu |
| [`solana/`](solana/) | On-chain play — **only compiled with `--features solana`** |
| [`assets/`](assets/) | Asset manifest documentation (models, textures, audio) |
| [`bin/`](bin/) | Auxiliary binaries (`pda`, `debugger`, …) |

## Data flow

```
Player input ─► game/ (validate) ─► rendering/ + presentation/ (draw)
                    │
                    ├─► engine/ (AI reply, single-player)
                    ├─► multiplayer/ (P2P relay / Braid subscription, casual PvP)
                    └─► solana/ (backend API → on-chain record_move, staked PvP)
```

## Feature flags

- **`solana`** — gates all on-chain code. Never import Solana types outside
  `src/solana/`; the default build must compile without the Solana SDK.

## Running

```bash
cargo run                     # offline / AI / casual play
cargo run --features solana   # with on-chain integration
```

Each module's README covers its internals; start with [`core/`](core/) for how the
app boots and which plugins are registered.
