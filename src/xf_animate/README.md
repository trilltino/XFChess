# xf_animate — main-menu chess showcase

A self-contained mini chess animation rendered inside the LEARN box on the main menu:
famous games replayed on a small board as menu dressing.

## Isolation contract

The plugin is strictly scoped to `GameState::MainMenu`:

- Every spawned entity carries `DespawnOnExit(MainMenu)`.
- Every system runs behind `run_if(in_state(MainMenu))`.

Nothing from this module runs, allocates, or ticks during actual gameplay — it can be
modified freely without risk to the game loop.

## Contents

| File | Responsibility |
|------|----------------|
| `board.rs` | The miniature board spawn/layout |
| `pieces.rs` | Piece entities and their meshes for the showcase |
| `games/` | The scripted famous-game move sequences being replayed |
| `sequence.rs` | Sequencing/timing of the replay (advance, loop) |
| `animation.rs` | Per-move piece movement animation |
| `viewport.rs` | Camera/viewport handling so the showcase renders inside the menu box |
