# xf_animate - main-menu chess showcase

A self-contained mini chess animation rendered inside the LEARN box on the main menu:
famous games replayed on a small board as menu dressing.

## Isolation contract

The plugin is strictly scoped to `GameState::MainMenu`:

- Every spawned entity carries `DespawnOnExit(MainMenu)`.
- Every system runs behind `run_if(in_state(MainMenu))`.

Nothing from this module runs, allocates, or ticks during actual gameplay - it can be
modified freely without risk to the game loop.

## Contents

| File | Responsibility |
|------|----------------|
| `../xf_animate.rs` | Plugin wiring and public root module exports |
| `animation/mod.rs` | Public facade for movement/fade components and animation systems |
| `animation/components.rs` | Per-piece animation components |
| `animation/systems.rs` | Movement, capture fade, and idle float systems |
| `board/mod.rs` | Public facade for board geometry, squares, and lighting |
| `board/coordinates.rs` | Board constants and square-to-world conversion |
| `board/squares.rs` | The miniature board spawn/layout |
| `board/lighting.rs` | Mini-board lighting setup |
| `pieces/mod.rs` | Public facade for piece assets, components, setup, and spawning |
| `pieces/assets.rs` | Mesh/material handles for the showcase pieces |
| `pieces/components.rs` | Piece marker/data components |
| `pieces/setup.rs` | Starting-position setup systems |
| `pieces/spawn.rs` | Single-piece entity construction |
| `games/` | The scripted famous-game move sequences being replayed |
| `sequence/mod.rs` | Public facade for replay model and playback systems |
| `sequence/model.rs` | Move data types shared by games and playback |
| `sequence/playback.rs` | Sequencing/timing of the replay: advance, loop, reset |
| `viewport/mod.rs` | Public facade for viewport state, conversion, camera, and sync |
| `viewport/resource.rs` | Viewport resource and render layer constant |
| `viewport/rect.rs` | Egui rectangle to physical-pixel conversion |
| `viewport/camera.rs` | Mini showcase camera spawn |
| `viewport/sync.rs` | Camera viewport activation and clamping |
