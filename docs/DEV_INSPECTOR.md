# Dev Inspector

bevy-inspector-egui 0.36 integration for XFChess debug builds.

## Activation

The inspector is **compiled in only on debug builds** (`cargo build`, `just dev`).
It is never present in release builds.

**Keyboard shortcut:** `F12` — toggles the full Bevy world inspector (all entities, components, resources).

The chess-specific panel (top-right corner) is **always visible** while in a game.

---

## Chess Panel — Live Variables

The panel updates every frame. All edits in the **Lighting**, **Board Colors**, and **Graphics** sections take effect immediately — no restart needed.

It has seven collapsible sections:

### Game State

| Variable | Source resource | What it tells you |
|----------|----------------|-------------------|
| `Turn` | `CurrentTurn.color` | Whose move it is right now (`White` / `Black`) |
| `Move #` | `CurrentTurn.move_number` | Full-move counter (increments after Black plays) |
| `Phase` | `CurrentGamePhase.0` | `Playing`, `Check`, `Checkmate`, or `Stalemate` |
| `Pending advance` | `PendingTurnAdvance.is_pending()` | Whether a turn flip is queued but not yet flushed. Should be `no` every frame except the one where a move lands |
| `View mode` | `ViewMode` | `Standard3D`, `Standard2D`, or `TempleOS` |

### Clocks

| Variable | Source | What it tells you |
|----------|--------|-------------------|
| `White` / `Black` | `GameTimer.white_time_left` / `black_time_left` | Remaining time in `m:ss` |
| `Increment` | `GameTimer.increment` | Fischer increment per move (seconds) |
| `Running` | `GameTimer.is_running` | Clock is actively counting down |
| `Preset` | `ActiveTimeControl.control` | Time control label (e.g. `5+3`) — only shown when resource exists |
| `AI game` | `ActiveTimeControl.ai_game` | Only human clock ticks when `yes` |

### AI Engine

| Variable | Source | What it tells you |
|----------|--------|-------------------|
| `Mode` | `ChessAIResource.mode` | `VS AI (Black/White)`, `Multiplayer`, or `Ranked` |
| `Engine` | `ChessAIResource.engine` | `XFChess` (built-in Rust engine) or `Stockfish` (external process) |
| `Difficulty` | `ChessAIResource.difficulty` | Level + ELO estimate, e.g. `Club (1300 ELO)` |
| `Think time` | `AIDifficulty::stockfish_movetime_ms()` | Budget per move in milliseconds |
| `Score (cp)` | `AIStatistics.last_score` | Last search eval in centipawns. Positive = White advantage |
| `Depth` | `AIStatistics.last_depth` | How many plies the last search reached |
| `Nodes` | `AIStatistics.last_nodes` | Positions evaluated in the last search |
| `Time` | `AIStatistics.thinking_time` | Wall-clock ms the last search took |

**Useful combos:**
- `Depth = 1` + `Nodes = 0` usually means Stockfish process didn't start — check `STOCKFISH_PATH`.
- Score swinging wildly between moves = horizon effect at low depth. Raise difficulty level.
- `Think time` much lower than `Time` = iterative deepening finishing early (good); higher = search timed out (expected at max difficulty).

### Lighting *(editable)*

Shows every `AmbientLight`, `DirectionalLight`, and `PointLight` in the scene (the check-highlight pulse light is excluded — it's driven by code).

| Control | Effect |
|---------|--------|
| Ambient brightness slider (0–600) | Overall scene fill light |
| Ambient color picker | Tint the ambient |
| Directional illuminance slider (0–50 000) | Sun/key light strength |
| Directional color picker | Warm/cool the key light |
| Point light intensity (logarithmic, 0–2 M) | Fill/overhead light power |
| Point light range slider | How far the point light reaches |
| Point light color picker | Tint the fill |

Lights are shown by their `Name` component (e.g. `"Main Directional Light"`, `"Fill Light"`).

### Board Colors *(editable)*

| Control | Effect |
|---------|--------|
| Light squares color picker | Cream squares (RGB, no alpha) |
| Dark squares color picker | Green squares (RGB, no alpha) |
| Move hints color picker | Hint dots — includes alpha slider |
| Selection color picker | Border overlay on selected piece |

Changes go directly into `Assets<StandardMaterial>` and update every board square in the same frame.

### Graphics *(editable)*

| Control | Effect |
|---------|--------|
| Quality dropdown (Low/Medium/High/Ultra) | Bloom + SSAO preset |
| Board theme dropdown (0–4) | Classic/Green/Blue/Purple/Dark |
| Move hints checkbox | Toggle hint dots globally |
| Last move checkbox | Toggle last-move highlight arrow |
| Eval bar checkbox | Toggle centipawn eval bar |
| Dyn. lights checkbox | Enable/disable orbital point lights |
| Light count slider | How many orbital lights orbit the board |
| Orbit radius slider | How far out orbital lights sit |
| Orbit speed slider | Rotation speed of orbital lights |

### Selection

| Variable | Source | What it tells you |
|----------|--------|-------------------|
| `Entity` | `Selection.selected_entity` | Bevy entity ID of the selected piece (`—` = nothing selected) |
| `Position` | `Selection.selected_position` | Square in algebraic notation, e.g. `e2` |
| `Legal moves` | `Selection.possible_moves.len()` | Count of valid destination squares |
| `Dragging` | `Selection.is_dragging` | Drag-move in progress |
| Destination list | `Selection.possible_moves` | All legal target squares as `a1 b3 c4 …` |

**Useful combos:**
- `Legal moves = 0` after clicking a piece = move generator returned nothing. The piece is pinned, in checkmate, or there's a bug in the position.
- `Position` shows the square in standard chess notation — useful to cross-check against the FEN or engine output.

---

## World Inspector (F12)

Opens the full Bevy world inspector showing:

- **Entities** — every entity in the world with all components. Filter by name or component type.
- **Resources** — all reflected resources. Chess-relevant ones:

| Resource | Registered by |
|----------|---------------|
| `CurrentTurn` | `GamePlugin` |
| `CurrentGamePhase` | `GamePlugin` |
| `GameTimer` | `GamePlugin` |
| `PendingTurnAdvance` | `GamePlugin` |
| `Selection` | `DevInspectorPlugin` |
| `ViewMode` | `GamePlugin` |
| `ChessAIResource` | `AIPlugin` |
| `AIStatistics` | `AIPlugin` |

You can **edit resource values live** in the world inspector — useful for:
- Forcing a turn switch by changing `CurrentTurn.color`
- Simulating low time by setting `GameTimer.white_time_left = 5.0`
- Changing AI difficulty mid-game via `ChessAIResource.difficulty`
- Injecting a selection by writing to `Selection.selected_position`

---

## Extending the Inspector

To add a new chess variable to the panel, edit [src/dev_tools/mod.rs](../src/dev_tools/mod.rs):

1. Add the resource as a system parameter in `chess_inspector_panel`.
2. Add a row in the appropriate `egui::Grid` block.

To make a resource editable via the world inspector (F12), add `#[derive(Reflect)]` + `#[reflect(Resource)]` to the struct and call `app.register_type::<MyResource>()` anywhere a `Plugin` builds.
