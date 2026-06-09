# 3D PGN Player — Chess Shorts Content Creation Plan

## What Already Exists

The foundation is complete and production-quality. Before adding anything, understand what's already shipped:

| File | What it does |
|------|-------------|
| `src/game/replay.rs` | Full PGN replay engine: FEN snapshots, step/auto-play, speed slider, move list panel, eval sparkline, puzzle mode, Ctrl+S screenshot, 2D/3D toggle |
| `src/game/replay_shorts.rs` | Annotation system: arrows (green/orange/blue), square highlights, 3D mesh arrows + disc highlights, 2D egui board overlay, "Can you find it?" puzzle banner, screenshot-to-Pictures-folder |
| `crates/nimzovich_engine/src/pgn.rs` | `parse_pgn`, `san_to_move`, `PgnAssembler`, `ParsedPgnGame` with tag support |
| `src/rendering/` | Isometric 3D board, all piece meshes, check highlight, last-move highlight, move hints |
| `src/rendering/camera/camera_templeos.rs` | Camera system with view modes |

**What the plan does NOT rebuild** — none of the above is touched. All new systems add onto existing resources and schedules.

---

## Gap Analysis: Shorts Plan vs Current State

### Already done ✅
- 3D board + 2D egui board (toggle with one button)
- Step forward/back/start/end, auto-play with speed slider
- Arrow + square annotations in both 2D and 3D
- Puzzle mode + "Can you find the move?" + reveal
- Screenshot export (Ctrl+S or button)
- Move list with clickable moves, Lichess-style header
- FEN position loader
- Eval sparkline overlay

### Missing for chess-shorts ❌
1. **PGN annotation import** — parse `[%cal]`, `[%csl]`, `[%arrow]` from move comments and load them as `ReplayAnnotations` on each ply
2. **Move quality badges** — parse `!!`, `??`, `?!`, `!?`, `!`, `?` and NAG codes `$1–$6` from PGN; display as colored icons on the move list and as a full-screen badge flash
3. **Cinematic effects** — slow-motion piece tween, blunder flash (red), brilliant glow (gold), checkmate dramatic pause
4. **Hook text system** — overlay text (e.g. "99% of players miss this...") with configurable in/out timing per ply
5. **Camera drama modes** — zoom-to-square, orbit pan, POV tilt on key moves
6. **Content tier workflow** — UI mode selector: Puzzle / Blunder / Highlight / Trick Opening; sets appropriate cinematic preset
7. **Sequence capture mode** — auto-advance through a range of plies, capturing a screenshot at each step for external video assembly
8. **Audio beat markers** — mark plies as "beat N" so the creator knows where to cut/sync audio externally

---

## Architecture

```
                    ┌─────────────────────────────────────────────────────────┐
                    │  Bevy app — PgnReplay AppState                          │
                    │                                                         │
                    │  ParsedPgnGameResource (nimzovich_engine)               │
                    │       ├── moves[]       (SAN)                           │
                    │       ├── annotations[] (per-ply: arrows, highlights,   │
                    │       │                  quality badge, hook text)       │
                    │       └── tags          (White, Black, Event, …)        │
                    │                                                         │
                    │  PgnReplayState         (existing)                      │
                    │       ├── fen_snapshots[]                               │
                    │       ├── current_ply                                   │
                    │       └── speed / timer                                 │
                    │                                                         │
                    │  ShortsState            (new resource)                  │
                    │       ├── content_tier  (Puzzle | Blunder | Highlight)  │
                    │       ├── cinematic_preset                              │
                    │       ├── hook_text[]   (per-ply text + timing)        │
                    │       ├── beat_markers[]                                │
                    │       └── capture_mode                                  │
                    │                                                         │
                    │  Rendering layer (all existing, extended with):         │
                    │       ├── PieceTweenState  — smooth move animation      │
                    │       ├── CinematicEffect  — flash, glow, slow-mo       │
                    │       └── CameraDirector   — zoom + orbit automation    │
                    └─────────────────────────────────────────────────────────┘
```

### Backend route (Phase 3 only — optional for offline use)

`POST /api/pgn/parse`  
Request: `{ pgn: string }`  
Response: `{ moves: string[], fen_snapshots: string[], annotations: PerPlyAnnotation[], tags: { [key]: string } }`

This route uses `nimzovich_engine::parse_pgn` + `san_to_move` to pre-compute snapshots server-side and return them to the web frontend (for a future web PGN viewer). **The in-game 3D viewer does this locally inside the Bevy process — no backend call needed.**

---

## Implementation Phases

---

### Phase 1 — PGN Annotation Import (1–2 days)

**Goal:** When PGN move comments contain `[%cal]`, `[%csl]`, `[%arrow]`, or quality suffixes, automatically load them as annotations on each ply.

**Files touched:**
- `crates/nimzovich_engine/src/pgn.rs` — add `parse_pgn_annotated()` that preserves per-move comments
- `src/game/replay_shorts.rs` — add `PerPlyAnnotation` struct and load system

**New struct in `ParsedPgnGame`:**
```rust
pub struct PerPlyAnnotation {
    pub arrows:     Vec<(u8, u8, u8, u8, u8)>,   // (ff, fr, tf, tr, kind)
    pub highlights: Vec<(u8, u8, u8)>,             // (file, rank, kind)
    pub quality:    MoveQuality,
    pub comment:    Option<String>,
    pub hook_text:  Option<String>,                // set by content creator manually
}

pub enum MoveQuality {
    Brilliant, Good, Interesting, Dubious, Mistake, Blunder, Normal
}
```

**Annotation parsing logic** (in `pgn.rs`):
- Strip `{ comment }` blocks, scan for `[%cal Ge2e4,Re8e1]` → green/red/yellow/blue arrows
- Scan for `[%csl Gd4]` → colored square highlights
- Detect trailing `!!`, `!`, `?`, `??`, `!?`, `?!` or NAG `$1`–`$6` → `MoveQuality`

**Load system in `replay_shorts.rs`:**
```rust
// On every ply change, load the per-ply annotation into ReplayAnnotations
pub fn load_pgn_annotations_system(
    replay: Res<PgnReplayState>,
    pgn: Option<Res<ParsedPgnGameResource>>,
    mut annotations: ResMut<ReplayAnnotations>,
)
```

This system fires after `replay_apply_move_system`. It replaces whatever the user had drawn with the embedded annotations from the PGN. User-drawn annotations override them on the same ply.

---

### Phase 2 — Cinematic Effects (2–3 days)

**Goal:** Piece moves tween smoothly; blunders flash red; brilliant moves glow gold; checkmates slow-mo.

#### 2a — Smooth Piece Tweens

Currently `replay_spawn_pieces_system` hard-teleports pieces when ply changes. Replace with:

**New resource in `src/game/replay.rs`:**
```rust
#[derive(Resource)]
pub struct PieceTweenState {
    pub active: Vec<PieceTween>,
    pub slow_factor: f32,   // 1.0 = normal, 0.1 = 10x slow-mo
}

pub struct PieceTween {
    pub entity: Entity,
    pub from: Vec3,
    pub to: Vec3,
    pub elapsed: f32,
    pub duration: f32,      // seconds, modulated by slow_factor
}
```

**System:** `piece_tween_system` runs every frame, advances elapsed, applies `Transform` lerp, despawns when done.

**Trigger:** On ply advance, instead of respawning all pieces, diff the board: find the moved piece entity, create a `PieceTween` for it.

**Cinematic triggers** (driven by `MoveQuality`):
- `Blunder` or `Mistake` → `slow_factor = 0.5` for 1.5 seconds, schedule `BlunderFlash` event
- `Brilliant` or `Good` → `slow_factor = 0.4`, schedule `BrilliantGlow` event
- Checkmate (detected via `game_state == STATE_CHECKMATE`) → `slow_factor = 0.08`, camera zoom-in

#### 2b — Screen Flash Effects

**New resource:**
```rust
#[derive(Resource, Default)]
pub struct CinematicEffect {
    pub flash_color: Color,
    pub flash_alpha: f32,    // 0.0 = invisible, 1.0 = full
    pub flash_decay: f32,    // alpha per second
}
```

**Events:**
```rust
#[derive(Event)] pub struct BlunderFlash;
#[derive(Event)] pub struct BrilliantGlow;
#[derive(Event)] pub struct CheckmateFlash;
```

**System:** `cinematic_effect_system` ticks `flash_alpha` down each frame and renders a full-screen egui overlay rectangle with the flash color and current alpha.

| Event | Color | Peak alpha | Decay |
|-------|-------|------------|-------|
| `BlunderFlash` | Red `(220, 30, 30)` | 0.55 | 0.6/s |
| `BrilliantGlow` | Gold `(255, 200, 30)` | 0.45 | 0.5/s |
| `CheckmateFlash` | White `(255, 255, 255)` | 0.7 | 0.25/s (slow fade) |

#### 2c — Move Quality Badge

On the egui layer, when `MoveQuality != Normal`, show a centered badge for 1.8 seconds:

```
┌─────────────┐
│  !!         │   brilliant (gold)
│  ??         │   blunder   (red)
│  ?!         │   dubious   (orange)
│  !?         │   interesting (purple)
└─────────────┘
```

Badge fades in (0.2s), holds (1.4s), fades out (0.2s). Drawn by `quality_badge_system` using the egui painter on top of the board.

---

### Phase 3 — Camera Drama (1–2 days)

**Goal:** Camera automatically zooms to the destination square on significant moves, then returns to overview.

**New resource:**
```rust
#[derive(Resource, Default)]
pub struct CameraDirector {
    pub mode: CameraMode,
    pub target_square: Option<(u8, u8)>,
    pub zoom_level: f32,     // 1.0 = default, 2.5 = close-up
    pub orbit_angle: f32,
    pub cinematic_timer: f32,
}

pub enum CameraMode {
    Overview,
    ZoomToSquare { from: Vec3, to: Vec3, t: f32 },
    OrbitPan { center: Vec3, speed: f32, elapsed: f32 },
    PovTilt,
}
```

**Trigger rules** (fire from the ply-change handler):
- `Blunder` → `ZoomToSquare` on the blundering king's square, then return to overview after 2s
- `Checkmate` → `OrbitPan` around the mated king, slow orbit for 4s
- `Brilliant` → `ZoomToSquare` on the destination square, hold 1.5s
- Hook ply (ply 0) → `PovTilt` (slight low-angle "cinematic" view) for the first 3 seconds

**System:** `camera_director_system` reads `CameraDirector`, modifies the active `Camera3d` entity's `Transform` each frame via lerp/slerp. Hooks into the existing `camera_templeos.rs` by reading the base orbit position and applying a multiplied offset.

---

### Phase 4 — Hook Text Overlay + Content Tiers (1 day)

**Goal:** Creator can type hook text per-ply and select a content tier; the UI adapts to it.

#### 4a — Hook Text

**New field in `ShortsState`:**
```rust
pub hook_texts: HashMap<usize, HookText>,   // keyed by ply

pub struct HookText {
    pub text: String,
    pub style: HookStyle,
    pub duration_secs: f32,
}

pub enum HookStyle {
    TopBold,     // big white bold text at top — "99% miss this..."
    BottomCaption,  // subtitle bar at bottom
    CenterDramatic, // centered, large, dark background
}
```

**Rendering:** `hook_text_system` draws via egui on the appropriate area. Text fades in over 0.3s, holds, fades out over 0.3s.

**Creator workflow:** In the right-side panel, a text field for hook text on the current ply. Stored in-memory and serialized to a sidecar `.shorts.json` file next to the `.pgn` file.

#### 4b — Content Tier Selector

Top of the right panel gets a tab row:

```
[ 🧩 Puzzle ]  [ ⚡ Blunder ]  [ 🏆 Highlight ]  [ 🎣 Opening Trap ]
```

Each tier applies a preset:
- **Puzzle** — enable puzzle mode at the current ply, set hook text "Can you find the move?", auto-advance speed 1.5s, pause at puzzle ply
- **Blunder** — set hook text "This move lost the game.", auto-reveal blunder move with `BlunderFlash`, slow-mo on the blunder ply
- **Highlight** — set hook text with player names, full auto-play at 0.8s speed, `BrilliantGlow` on the key move
- **Opening Trap** — set hook text "This trick wins in 4 moves.", auto-play at 1.0s speed, `BrilliantGlow` on every key ply

---

### Phase 5 — Sequence Capture Mode (1 day)

**Goal:** Auto-advance through a ply range, capture a screenshot at each step for external video assembly tools.

**New fields in `ShortsState`:**
```rust
pub capture_mode: Option<CaptureSequence>,

pub struct CaptureSequence {
    pub from_ply: usize,
    pub to_ply:   usize,
    pub current:  usize,
    pub delay_secs: f32,
    pub timer:    Timer,
    pub output_dir: PathBuf,
}
```

**System:** `capture_sequence_system` — when capture mode is active:
1. Wait for `delay_secs` (defaults to `replay.speed + 0.15s` for tween to settle)
2. Fire `ScreenshotRequested` → saves PNG named `capture_ply_NNN.png`
3. Advance `current_ply` by 1
4. Stop when `current == to_ply`

Output folder defaults to `Pictures/xfchess_sequence_<timestamp>/`.

**UI:** Button "🎬 Capture Sequence" in the control bar. Opens a small dialog to set ply range and output folder.

---

### Phase 6 — Audio Beat Markers (0.5 day)

**Goal:** Creator marks plies that should land on audio beats; exported as metadata for video editing.

**New field:**
```rust
pub beat_markers: BTreeMap<usize, String>,  // ply → label ("beat_1", "drop", etc.)
```

**UI:** Right-click on a move in the move list → "Mark as beat". Beat plies shown with a small ♩ icon in the move list.

**Export:** `File → Export Beat Sheet` writes a JSON file:
```json
{
  "game": "Magnus vs Nepo, 2023",
  "beats": [
    { "ply": 0, "label": "hook_text", "fen": "..." },
    { "ply": 12, "label": "blunder", "fen": "..." },
    { "ply": 13, "label": "reveal", "fen": "..." }
  ]
}
```

This is pasted into CapCut / DaVinci to sync cuts manually.

---

## File Map

```
src/
├── game/
│   ├── replay.rs              (existing — add PieceTweenState, tween trigger on ply change)
│   ├── replay_shorts.rs       (existing — add: load_pgn_annotations_system, quality_badge_system,
│   │                                               cinematic_effect_system, hook_text_system,
│   │                                               capture_sequence_system, beat_marker_export)
│   └── shorts_state.rs        (NEW — ShortsState, ContentTier, CaptureSequence, HookText)
│
├── rendering/
│   ├── camera/
│   │   ├── camera_templeos.rs (existing — extend with CameraDirector offset hook)
│   │   └── camera_director.rs (NEW — CameraDirector resource + camera_director_system)
│   └── effects/
│       └── cinematic.rs       (NEW — BlunderFlash/BrilliantGlow/CheckmateFlash events + system)
│
└── ui/
    └── shorts_panel.rs        (NEW — content tier selector, hook text editor, beat marker UI)

crates/
└── nimzovich_engine/src/
    └── pgn.rs                 (existing — add parse_pgn_annotated() + PerPlyAnnotation)
```

---

## Shorts Workflow — End to End

1. **Load PGN** — drag `.pgn` onto the window or paste via File → Open PGN
2. **Choose content tier** — select Puzzle / Blunder / Highlight / Opening Trap
3. **Set hook text** — type the first-frame text (e.g. "Magnus didn't see this...")
4. **Navigate** — step through with `←`/`→` or click moves; draw arrows manually or let PGN annotations load automatically
5. **Preview cinematic** — press Play; see slow-mo, flash effects, camera drama
6. **Mark beats** — right-click moves to mark audio sync points
7. **Capture sequence** — "🎬 Capture Sequence" saves one PNG per ply to `Pictures/xfchess_sequence_*/`
8. **Export beat sheet** — copy JSON into CapCut timeline for audio sync

Total turnaround: ~3 minutes from PGN to a folder of frames ready for video assembly.

---

## Formula Coverage Check

| Shorts formula step | Feature covering it |
|--------------------|---------------------|
| Hook 0–3s: visual shock | Hook text overlay + `PovTilt` camera mode |
| Hook 0–3s: bold text | `HookStyle::TopBold` |
| Tension / puzzle setup 3–12s | Puzzle mode, auto-play, per-ply annotations |
| "Can you find the move?" | PuzzleOverlay (already exists) |
| Dramatic reveal 12–19s | Reveal button → `BrilliantGlow` + camera zoom |
| Blunder moment | `BlunderFlash` + slow-mo + red flash |
| Checkmate slow-mo | `CheckmateFlash` + 0.08x slow factor + `OrbitPan` |
| Sync to audio | Beat markers + beat-sheet JSON export |
| Sequence capture | `CaptureSequence` system |
| Quality badges !! ?? | `quality_badge_system` |
| CTA / follow prompt | Add as hook text at final ply |

---

## Build Order

| Phase | Effort | Dependency |
|-------|--------|------------|
| 1 — PGN annotation import | 1–2 days | None |
| 2 — Cinematic effects | 2–3 days | Phase 1 (needs quality badges) |
| 3 — Camera drama | 1–2 days | Phase 2 (needs slow_factor) |
| 4 — Hook text + content tiers | 1 day | Phase 2 |
| 5 — Sequence capture | 1 day | Phase 2 (tween must settle) |
| 6 — Audio beat markers | 0.5 day | Phase 4 |

**Total: ~7–10 days of Bevy/Rust work.**

Phases 1–2 deliver the highest impact (visual quality of content). Phases 3–6 are workflow acceleration for creators.
