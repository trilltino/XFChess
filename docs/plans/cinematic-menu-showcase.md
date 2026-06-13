# Cinematic Chessboard Showcase — Design & Implementation Plan

A living main-menu background: the board sits in a master-level position and, every
10–15s, the menu cuts to a curated cinematic — fade to black, switch to a dramatic
camera angle, play one beautiful move over 2–3s, hold, fade back to the calm default
view. Curated, not procedural. Navigation is never blocked.

---

## 1. Goals & non-goals

**Goals**
- A repeating cinematic loop over a fixed set of camera angles and curated moments.
- Smooth fades, hard cuts between angles, eased single-move animation, a hold beat.
- Feels calm and "alive" — never grabs input, never blocks menu clicks.
- Curated library of positions + moves ("beautiful moments"), each tagged with an angle.

**Non-goals**
- No gameplay, no engine search, no networking.
- Not a replay viewer (that's `PGN Replay`).
- Doesn't change menu layout/logic — it only drives the 3D background + a fade overlay.

---

## 2. What already exists (build on this, don't reinvent)

| Piece | Where | Role |
|-------|-------|------|
| `MenuCameraOrbit` + `orbit_camera_system` | `src/states/main_menu/new_menu.rs` | The default slow orbit (perspective) / ortho (`V`) camera. Drives the `Camera3d` from `PersistentEguiCamera`. Looks at `BOARD_CENTER = (3.5, 0, 3.5)`. |
| `BoardAnimator` + `animate_menu_pieces` + `init_board_animator` | `src/states/main_menu/board_animation.rs` | Plays the Immortal Zugzwang PGN on the background board; slides pieces via `MenuBgPieceAnim` (cubic smoothstep + arc lift). Holds the live entity map `board[rank][file]`. |
| `MenuBg`, `MenuBgPiecePos`, `MenuBgPieceHome`, `MenuBgPieceAnim` | `board_animation.rs` | Per-piece components: current square, home square (for reset), in-flight slide. |
| `spawn_menu_bg_board`, `spawn_menu_bg_pieces` | `new_menu.rs` | Build the 8×8 board + 32 pieces. Squares at `Transform::from_xyz(7.0 - file, 0.0, rank)` → **world x = 7 − file, world z = rank**, y = 0. |
| `PieceMeshes` | `rendering::pieces` | Shared meshes; reuse for any pieces the cinematic spawns. |
| `GameState::MainMenu`, `EguiPrimaryContextPass` | core | State gate + egui render pass (where the fade overlay draws). |
| `nimzovich_engine` (`new_game`, `parse_pgn`, `san_to_move`, `do_move`) | crate | Already used to turn SAN → square coordinates for the menu board. Reuse for curated moves and for loading a position. |

**Coordinate convention (critical):** a board square `(file 0..7, rank 0..7)` maps to world
`Vec3::new(7.0 - file as f32, 0.0, rank as f32)`. Define one helper and use it everywhere:

```rust
#[inline]
pub fn square_to_world(file: u8, rank: u8) -> Vec3 {
    Vec3::new(7.0 - file as f32, 0.0, rank as f32)
}
```

Confirm which rank is White's home from `MenuBgPieceHome` on a known piece (e.g. the
white king) before hardcoding "behind White" camera positions — the doc assumes
**White on low ranks (rank 0–1), Black on high ranks (rank 6–7)**; flip the z sign in the
angle table if that's inverted.

---

## 3. Architecture overview

A small **state machine** (`MenuCinematic` resource) runs the loop. When it is *not*
idle it **takes over the camera** (and `orbit_camera_system` yields). A **director**
system advances phases on a timeline; a **camera** system positions the `Camera3d`
each frame from the active angle (with easing on cuts); a **fade** system draws a
full-screen black overlay in egui; the existing **`animate_menu_pieces`** does the
actual piece slide.

```
            ┌────────────────────────── cinematic loop ──────────────────────────┐
 Idle(orbit) ─► FadeOut ─► Cut+Setup ─► AnimateMove ─► Hold ─► FadeIn ─► Idle(orbit)
   ~10–15s      0.5–1.0s     instant       2–3s        ~1.5s    0.5–1.0s
```

### 3.1 Phase state machine

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CinematicPhase {
    /// Default menu orbit; counting down to the next cinematic.
    Idle,
    /// Screen fading to black before the cut.
    FadeOut,
    /// One frame: snap camera to the new angle, set the board to the moment's position.
    Cut,
    /// Animating the curated move (pieces slide via MenuBgPieceAnim).
    AnimateMove,
    /// Holding on the resulting position.
    Hold,
    /// Fading back in (still on the cinematic angle, then handing back to orbit).
    FadeIn,
}

#[derive(Resource)]
pub struct MenuCinematic {
    pub phase: CinematicPhase,
    pub timer: f32,            // seconds remaining in the current phase
    pub idle_gap: f32,         // randomized 10–15s for the next Idle→FadeOut
    pub fade_alpha: f32,       // 0.0 = clear, 1.0 = black
    pub seq_index: usize,      // index into the curated SEQUENCES list (cycles)
    pub enabled: bool,         // master switch (settings toggle / reduced-motion)
}

impl Default for MenuCinematic {
    fn default() -> Self {
        Self {
            phase: CinematicPhase::Idle,
            timer: 0.0,
            idle_gap: 12.0,
            fade_alpha: 0.0,
            seq_index: 0,
            enabled: true,
        }
    }
}

impl MenuCinematic {
    /// True whenever the cinematic owns the camera (everything except Idle).
    pub fn active(&self) -> bool { self.phase != CinematicPhase::Idle }
}
```

### 3.2 Coexistence with `orbit_camera_system`

`orbit_camera_system` gets one guard at the top so the cinematic owns the camera while
active:

```rust
pub fn orbit_camera_system(/* … */, cinematic: Res<MenuCinematic>, /* … */) {
    if cinematic.active() { return; }   // cinematic camera system is driving
    // … existing orbit / ortho logic …
}
```

On `FadeIn → Idle`, snap `MenuCameraOrbit.angle` so the orbit resumes from the
cinematic camera's current bearing (no visible jump): compute `angle = atan2(z - cz, x - cx)`
from the last cinematic camera position before yielding.

---

## 4. Camera angle library

Each angle is a pure function `fn(focus: Vec3, t: f32) -> CameraShot`, where `focus` is the
featured square (or `BOARD_CENTER`) and `t∈[0,1]` lets an angle add a slow push-in over
the shot. `CameraShot` carries the transform, projection, and optional depth-of-field.

```rust
pub struct CameraShot {
    pub transform: Transform,           // position + looking_at(focus)
    pub projection: ProjectionKind,     // Perspective{fov} | Ortho{height}
    pub dof: Option<DofParams>,         // Some => enable DepthOfField focused on `focus`
}

pub enum ProjectionKind { Perspective { fov_deg: f32 }, Ortho { height: f32 } }
pub struct DofParams { pub focal_distance: f32, pub aperture_f_stops: f32 }

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CameraAngle {
    KingsideLow, QueensideLow, WhitePlayerView, BlackPlayerView,
    TopDownTilted, CornerCloseup, HeroShot,
}
```

Reference values (relative to `BOARD_CENTER = (3.5,0,3.5)`; tune in-engine). `dist` is XZ
distance from focus, `height` is camera Y, `fov` is vertical FOV.

| Angle | Position (relative to focus) | Height | FOV / Proj | DoF | Feel |
|-------|------------------------------|--------|-----------|-----|------|
| **KingsideLow** | toward White kingside corner (`+x`,`−z`), `dist≈9` | `2.5` | `fov 45°` | off | front-line, pieces large |
| **QueensideLow** | White queenside corner (`−x`,`−z`), `dist≈9` | `2.5` | `fov 45°` | off | flank, long diagonals |
| **WhitePlayerView** | straight behind White (`z = focus.z − dist`), `dist≈10` | `7.0` | `fov 40°` | off | familiar "your move" |
| **BlackPlayerView** | straight behind Black (`z = focus.z + dist`), `dist≈10` | `7.0` | `fov 40°` | off | opponent's eyes |
| **TopDownTilted** | small XZ offset, `dist≈6` | `15` | `Ortho{height:11}` | off | grandmaster analysis |
| **CornerCloseup** | nearest corner, `dist≈6`, slight yaw so foreground pieces frame the shot | `3.5` | `fov 50°` | light | spectator leaning in |
| **HeroShot** | very low, close to the featured piece, `dist≈4` | `1.4` | `fov 35°` | **strong** (focus on piece) | movie-poster |

Push-in: lerp `dist` by `~10%` and `height` slightly over the shot using `t` for subtle life.

**Depth of field** (HeroShot / CornerCloseup) uses Bevy's
`bevy::core_pipeline::dof::DepthOfField` component on the camera, with
`focal_distance = camera→focus distance` and a small `aperture_f_stops`. Add/remove the
component on cut; clamp focal distance each frame as the move animates so the moving piece
stays sharp.

---

## 5. The curated "beautiful moments" library

A moment = a **position to set up** + **one move to play** + **the angle** + **timing** +
**the focus square** (what the camera frames; usually the move's destination).

```rust
pub struct MenuSequence {
    pub name: &'static str,
    pub angle: CameraAngle,
    /// Starting position for this moment. FEN so positions are exact & curated.
    pub fen: &'static str,
    /// The move to animate, in UCI (src/dst[/promo]) — unambiguous, no SAN parsing edge cases.
    pub mv: &'static str,
    /// Move animation duration (seconds).
    pub move_secs: f32,
    /// Hold after the move (seconds).
    pub hold_secs: f32,
    /// Square the camera frames; default = move destination.
    pub focus: Option<(u8, u8)>,
}

pub static SEQUENCES: &[MenuSequence] = &[
    MenuSequence { name: "Knight fork",   angle: CameraAngle::HeroShot,
        fen: "r1bqk2r/ppp2ppp/2n5/3np3/2B5/5N2/PPPP1PPP/RNBQ1RK1 w kq - 0 1",
        mv: "f3g5", move_secs: 2.5, hold_secs: 1.5, focus: None },
    MenuSequence { name: "Queen slide",   angle: CameraAngle::TopDownTilted,
        fen: "…", mv: "d1h5", move_secs: 3.0, hold_secs: 1.5, focus: None },
    MenuSequence { name: "Pawn promotion",angle: CameraAngle::HeroShot,
        fen: "…", mv: "e7e8q", move_secs: 2.5, hold_secs: 2.0, focus: None },
    MenuSequence { name: "Castling",      angle: CameraAngle::KingsideLow,
        fen: "…", mv: "e1g1", move_secs: 2.0, hold_secs: 1.5, focus: None },
    MenuSequence { name: "Rook lift",     angle: CameraAngle::QueensideLow,    fen: "…", mv: "…", move_secs: 2.5, hold_secs: 1.5, focus: None },
    MenuSequence { name: "Bishop skewer", angle: CameraAngle::CornerCloseup,   fen: "…", mv: "…", move_secs: 2.5, hold_secs: 1.5, focus: None },
    MenuSequence { name: "Checkmate",     angle: CameraAngle::HeroShot,        fen: "…", mv: "…", move_secs: 2.5, hold_secs: 2.5, focus: None },
];
```

Why **UCI for the move** even though the engine has `san_to_move`: SAN needs the full
position to disambiguate and to know it's legal; UCI src/dst is exact and trivially maps to
square coordinates with no parsing branches. Validate each curated move once in a unit test
(`validate_and_apply` from the on-chain crate, or engine move-gen) so a typo can't ship.

The loop cycles `seq_index = (seq_index + 1) % SEQUENCES.len()`.

### 5.1 Game-sourced moments (Immortal Games) — reuse the PGN replay

The ambient background already plays the **Immortal Zugzwang Game** (Sämisch–Nimzowitsch,
Copenhagen 1923) via `BoardAnimator` + `ZUGZWANG_PGN` in `board_animation.rs`, and
`precompute_plies()` there already turns a PGN into per-ply `(src,dst)` movements (handling
castling, en passant, promotion). We can spotlight dramatic plies straight from full famous
games — no hand-authored FENs, and every position is guaranteed legal because it comes from
a real game.

A game-sourced highlight = a PGN + the ply to feature + presentation:

```rust
pub struct MenuGameHighlight {
    pub game: &'static str,     // label, e.g. "Immortal Game — Be7#"
    pub pgn: &'static str,      // full game PGN
    pub ply: usize,             // 0-based half-move to animate; board is replayed to it first
    pub angle: CameraAngle,
    pub move_secs: f32,
    pub hold_secs: f32,
    pub focus: Option<(u8, u8)>,// default = that ply's destination square
}
```

**Setup at `Cut`:** call `precompute_plies(pgn)` (already exists), apply plies `0..ply`
instantly to position the board *during the black frame*, then animate ply `ply` exactly
like a curated move (§7). This reuses `set_menu_position`'s entity mapping but feeds it the
replayed position instead of a parsed FEN.

```rust
pub static GAME_HIGHLIGHTS: &[MenuGameHighlight] = &[
    // The original Immortal Game (Anderssen–Kieseritzky, London 1851).
    MenuGameHighlight { game: "Immortal Game — Bishop takes the rook (Bxg1)",
        pgn: IMMORTAL_GAME_PGN, ply: 35, angle: CameraAngle::TopDownTilted,
        move_secs: 3.0, hold_secs: 1.5, focus: None },
    MenuGameHighlight { game: "Immortal Game — Be7# (the finish)",
        pgn: IMMORTAL_GAME_PGN, ply: 44, angle: CameraAngle::HeroShot,
        move_secs: 2.5, hold_secs: 3.0, focus: None },
    // The Immortal Zugzwang Game — the SAME game the ambient board plays, so the
    // cinematic zooms into a moment the player has been watching unfold.
    MenuGameHighlight { game: "Immortal Zugzwang — 25…h6, the cage closes",
        pgn: super::board_animation::ZUGZWANG_PGN, ply: 49,
        angle: CameraAngle::CornerCloseup, move_secs: 2.5, hold_secs: 2.5, focus: None },
];

const IMMORTAL_GAME_PGN: &str = "
1. e4 e5 2. f4 exf4 3. Bc4 Qh4+ 4. Kf1 b5 5. Bxb5 Nf6 6. Nf3 Qh6
7. d3 Nh5 8. Nh4 Qg5 9. Nf5 c6 10. g4 Nf6 11. Rg1 cxb5 12. h4 Qg6
13. h5 Qg5 14. Qf3 Ng8 15. Bxf4 Qf6 16. Nc3 Bc5 17. Nd5 Qxb2 18. Bd6 Bxg1
19. e5 Qxa1+ 20. Ke2 Na6 21. Nxg7+ Kd8 22. Qf6+ Nxf6 23. Be7# 1-0
";
```

Ply indices are 0-based half-moves and **must be confirmed against `precompute_plies`
output** (count plies: move `n` White = ply `2(n-1)`, Black = `2(n-1)+1`). Examples above:
`18…Bxg1` = ply 35; `23. Be7#` (White's 23rd) = ply 44; Zugzwang `25…h6` = ply 49. Make
`ZUGZWANG_PGN` `pub` so the cinematic can reference it.

**Unify the two libraries** behind one rotation so the director doesn't care about the
source:

```rust
pub enum MenuMoment {
    Fen(&'static MenuSequence),       // §5 hand-curated one-offs
    Game(&'static MenuGameHighlight), // §5.1 PGN replay
}
// The director resolves either into: (position, animated_move, angle, move_secs, hold_secs, focus).
```

The checkmate highlights (`Be7#`) are natural `HeroShot` finishers; the queen-sac/zugzwang
moments suit `TopDownTilted`/`CornerCloseup` so the whole combination reads.

---

## 6. Setting the board to a position (FEN → menu pieces)

The ambient `BoardAnimator` keeps the calm Zugzwang game running during `Idle`. When a
cinematic starts, the board must snap to the moment's FEN. Two implementation options:

- **Option A — Reset & reposition (recommended).** Keep the existing 32 `MenuBg` piece
  entities. Add a function `set_menu_position(fen, …)` that:
  1. Parses the FEN with `nimzovich_engine` into a `[[Option<(PieceColor,PieceType)>;8];8]`.
  2. For each square, ensures the right piece entity is present at the right world pos:
     reuse home entities where the type matches; hide (move far below / `Visibility::Hidden`)
     pieces not in this position; show & reposition the rest.
  3. Rebuilds `BoardAnimator.board[rank][file]` so `animate_menu_pieces` & the move step work.

  This avoids per-cinematic despawn/spawn churn. The mapping "which entity goes where" can be
  greedy by `(color, piece_type)`; extra promoted pieces (e.g. two queens) are handled by
  keeping a small pool of hidden spares or spawning on demand.

- **Option B — Despawn & respawn.** On `Cut`, despawn all `MenuBg` pieces and spawn exactly
  the FEN's pieces from `PieceMeshes`. Simpler to reason about, but flickers and churns the
  entity map; only use if Option A proves fiddly.

Either way, set pieces **instantly** during the black frame (`Cut` phase) so the snap is
hidden by the fade.

After the cinematic, on `FadeIn → Idle`, restore the ambient game: reset pieces to
`MenuBgPieceHome` (the existing "loop back" reset path in `board_animation.rs`) and let
`BoardAnimator` resume from `ply_index = 0` (or a remembered index).

---

## 7. Playing the curated move

Reuse the existing slide animation — don't write a new one:

1. Parse `mv` (UCI) → `(from_file, from_rank, to_file, to_rank)` + promo.
2. Look up the entity at `from` in `BoardAnimator.board`.
3. If the destination has an enemy piece, hide it (capture) — or animate a quick sink/fade.
4. Insert `MenuBgPieceAnim { start: square_to_world(from), end: square_to_world(to),
   elapsed: 0.0, duration: move_secs }` on the moving entity; update its `MenuBgPiecePos`
   and the `board[][]` map. Castling adds a second `MenuBgPieceAnim` for the rook;
   promotion swaps the pawn mesh for the promoted piece at the end (or hides pawn / shows queen).
5. `animate_menu_pieces` (already registered) advances it. The director's `AnimateMove`
   phase timer = `move_secs`; when it elapses, go to `Hold`.

The featured-piece arc/lift already exists (`arc = sin(π t) * 0.28`) — for HeroShot you may
scale the arc up slightly for drama via a per-sequence multiplier.

---

## 8. Fade overlay

A full-screen black rectangle drawn in egui at the very end of the menu pass, alpha =
`cinematic.fade_alpha`:

```rust
pub fn cinematic_fade_overlay(mut contexts: EguiContexts, cine: Res<MenuCinematic>) {
    if cine.fade_alpha <= 0.001 { return; }
    let Ok(ctx) = contexts.ctx_mut() else { return; };
    let a = (cine.fade_alpha.clamp(0.0, 1.0) * 255.0) as u8;
    egui::Area::new("cinematic_fade".into())
        .order(egui::Order::Foreground)            // above the 3D board, below nothing important
        .fixed_pos(egui::pos2(0.0, 0.0))
        .interactable(false)                        // never eat menu clicks
        .show(ctx, |ui| {
            let r = ctx.screen_rect();
            ui.painter().rect_filled(r, 0.0, egui::Color32::from_black_alpha(a));
        });
}
```

`interactable(false)` is essential — the overlay must not block the menu. Note the menu
panel window is drawn separately; ensure ordering puts the menu text either above the fade
(so menus stay readable) or accept that during the brief full-black frames the menu dims too
(matches "fade to black"). Recommended: fade only dims the **board** area, leaving the menu
panel legible — draw the overlay behind the menu window (`Order::Middle`) so navigation never
visually drops out. Pick one and note it; the spec's "fade to black" implies whole-screen, so
default to whole-screen but keep the menu window `Order::Foreground` above it.

---

## 9. Director system (the timeline)

```rust
pub fn cinematic_director(
    time: Res<Time>,
    mut cine: ResMut<MenuCinematic>,
    mut board: ResMut<BoardAnimator>,
    // + queries/commands needed to set position & spawn MenuBgPieceAnim
) {
    if !cine.enabled { return; }
    let dt = time.delta_secs();
    cine.timer -= dt;

    match cine.phase {
        CinematicPhase::Idle => {
            // ambient orbit + Zugzwang game run normally
            if cine.timer <= 0.0 {
                cine.phase = CinematicPhase::FadeOut;
                cine.timer = 0.8; // fade-out seconds
            }
        }
        CinematicPhase::FadeOut => {
            cine.fade_alpha = (1.0 - cine.timer / 0.8).clamp(0.0, 1.0);
            if cine.timer <= 0.0 { cine.fade_alpha = 1.0; cine.phase = CinematicPhase::Cut; cine.timer = 0.0; }
        }
        CinematicPhase::Cut => {
            let seq = &SEQUENCES[cine.seq_index];
            // set_menu_position(seq.fen, …); cut camera to seq.angle; (all while black)
            // queue the move's MenuBgPieceAnim with duration seq.move_secs
            cine.phase = CinematicPhase::AnimateMove;
            cine.timer = seq.move_secs;
            // start fading back in immediately so the move is visible
        }
        CinematicPhase::AnimateMove => {
            // fade from black to clear over the first ~0.6s of the move
            cine.fade_alpha = (1.0 - (SEQUENCES[cine.seq_index].move_secs - cine.timer) / 0.6).clamp(0.0, 1.0);
            if cine.timer <= 0.0 { cine.phase = CinematicPhase::Hold; cine.timer = SEQUENCES[cine.seq_index].hold_secs; }
        }
        CinematicPhase::Hold => {
            if cine.timer <= 0.0 { cine.phase = CinematicPhase::FadeIn; cine.timer = 0.8; }
        }
        CinematicPhase::FadeIn => {
            cine.fade_alpha = (cine.timer / 0.8).clamp(0.0, 1.0); // black → clear handled by orbit handoff
            if cine.timer <= 0.0 {
                cine.fade_alpha = 0.0;
                cine.phase = CinematicPhase::Idle;
                cine.idle_gap = rand_range(10.0, 15.0);
                cine.timer = cine.idle_gap;
                cine.seq_index = (cine.seq_index + 1) % SEQUENCES.len();
                // reset ambient board to home, resume BoardAnimator
            }
        }
    }
}
```

(The exact fade choreography — black during `Cut`, fade-in during the start of
`AnimateMove`, fade-out during `FadeIn` — matches the spec's "cut to angle, animate move,
hold, fade to black, return".)

The **cinematic camera system** runs after the director, reads `cine.phase`/`seq_index`,
and writes the `Camera3d` transform/projection/DoF from the angle table (with cut-snap +
optional ease/push-in). It only runs `if cine.active()`.

---

## 10. Module & schedule layout

New file: `src/states/main_menu/cinematic.rs`

- Resources: `MenuCinematic`.
- Data: `CameraAngle`, `CameraShot`, `MenuSequence`, `SEQUENCES`, angle functions.
- Systems:
  - `cinematic_director` (Update, `run_if(in_state(GameState::MainMenu))`).
  - `cinematic_camera_system` (Update, after director; `run_if(MainMenu)`).
  - `cinematic_fade_overlay` (`EguiPrimaryContextPass`, `run_if(MainMenu)`).
  - helpers: `set_menu_position`, `square_to_world`, `queue_curated_move`.
- Register in `MainMenuPlugin` (`src/states/main_menu.rs`) alongside the existing menu
  systems; add `.init_resource::<MenuCinematic>()`.
- One-line guard added to `orbit_camera_system`.

No changes to menu navigation, panels, or input handling.

---

## 11. Implementation phases (incremental, each shippable)

1. **Scaffolding & fade.** Add `MenuCinematic`, the director state machine, and the fade
   overlay. Wire timing so it cycles Idle→FadeOut→(black)→FadeIn→Idle with *no* camera/board
   change yet. Verify the loop + fade visually.
2. **Camera angles.** Implement `CameraAngle` functions + `cinematic_camera_system` + the
   `orbit_camera_system` guard + orbit handoff. Cycle through all 7 angles on a dummy
   position. Tune the angle table in-engine.
3. **Position setup.** Implement `set_menu_position(fen)` (Option A) and the ambient-reset
   path. Confirm snaps happen during black frames.
4. **Curated move playback.** Parse UCI, queue `MenuBgPieceAnim`, handle captures/castling/
   promotion. Add the `SEQUENCES` table (start with 2–3 moments).
4b. **Game-sourced highlights (§5.1).** Make `ZUGZWANG_PGN` `pub`; reuse `precompute_plies`
   to replay a PGN to ply `N` for the position, then animate ply `N`. Add `GAME_HIGHLIGHTS`
   (the Immortal Game finish `Be7#`, its queen sac, and a Zugzwang moment) and the
   `MenuMoment` enum unifying both libraries in one rotation.
5. **Depth of field + polish.** DoF for HeroShot/CornerCloseup, push-in easing, per-sequence
   arc multiplier, final timing pass. Fill out the full moment library.
6. **Settings + accessibility.** A "Cinematic menu" toggle (and honor a reduced-motion
   setting) that sets `MenuCinematic.enabled = false` → pure orbit.

---

## 12. Edge cases & constraints

- **Never block navigation.** Fade overlay is `interactable(false)`; the menu window stays
  above it (or only the board dims). Clicking "Play Online" mid-cinematic must work instantly.
- **Leaving MainMenu mid-cinematic.** On `OnExit(GameState::MainMenu)` reset
  `MenuCinematic` to default and clear `fade_alpha` so re-entry starts clean. Pieces are
  `DespawnOnExit(MainMenu)` already.
- **`V` ortho toggle.** Decide precedence: simplest is to suppress the cinematic while the
  user has forced ortho (treat as `enabled = false` until toggled back).
- **Performance.** No spawning during steady state (Option A reuses entities). DoF is the
  only added GPU cost and only during two angles.
- **Determinism / correctness.** Unit-test every `SEQUENCES` entry: FEN parses, the UCI move
  is legal in that FEN (`validate_and_apply`), and src has a piece of the side to move.
- **Camera handoff.** Recompute `MenuCameraOrbit.angle` from the final cinematic camera
  bearing on return to `Idle` to avoid an orbit jump.

---

## 13. Testing

- **Unit (`cinematic.rs` tests):** each `MenuSequence` FEN+move validates; `square_to_world`
  round-trips the known board square positions from `spawn_menu_bg_board`; angle functions
  produce a transform whose forward points at `focus`.
- **Manual:** run the menu, watch a full cycle through all angles; confirm fades, the move
  animates on the correct angle, hold beat reads, navigation stays responsive, and the orbit
  resumes without a jump. Toggle the settings switch off → pure orbit.

---

## 14. Future extensions

- Subtle ambient SFX per moment (whoosh on slide, soft thud on landing) reusing the audio
  setup from `MenuSounds`.
- Per-moment captions ("Immortal Game — Bxh7+") fading in bottom-center.
- Pull moments from famous games (Opera Game, Kasparov–Topalov) as a themed rotation.
- Let the cinematic feature the *current* ambient game's most spectacular ply instead of a
  fixed library (hybrid curated/procedural).
