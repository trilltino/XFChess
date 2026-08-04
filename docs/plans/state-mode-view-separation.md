# State / Mode / View Separation & Cleanup — E2E Plan

**Date:** 2026-07-16 (updated 2026-07-18)
**Status:** Phase 0 + Phase 1 landed & verified; Phases 2–4 pending.
Also landed 2026-07-18: **menu visual unification** — the main-menu board now
uses the same lit Cream/Green materials, the in-game "Angel Light" + a
camera-following fill (shared `CameraFollowLight` + `update_board_fill_light`),
and matched ambient, so the menu renders identically to a live game.
**Trigger:** Field bugs from one play session: a new game started with an "extra
white bishop"; toggling 2D → 3D view made all pieces disappear; after quitting,
game-piece artifacts (flat 2D picking quads) appeared on the *menu* board; a dead
black band sat right of the in-game sidebar.

All four are symptoms of one architectural problem: **entity lifetime and
visibility are governed by several parallel, unsynchronized mechanisms**, so
every state/mode/view transition is a chance for them to disagree.

---

## 1. Current architecture (inventory)

### 1.1 State axes (who says "where are we?")

| Axis | Type | Values | Written by |
|------|------|--------|-----------|
| `GameState` | Bevy `States` | `Auth`, `MainMenu`, `InGame`, `Paused`, `GameOver` | many |
| `MenuState` | Bevy `States` (sub) | `Main`, `ModeSelect`, `BraidLobby`, `LobbySelection`, … | menu UI |
| `GameMode` | **plain `Resource`** | `SinglePlayer`, `MultiplayerLocal`, `MultiplayerCompetitive`, `OnlineMultiplayer`, `Spectator`, `PgnReplay` | menu flows, matchmaking, replay |
| `ViewMode` | **plain `Resource`** | `Standard3D`, `Standard2D` (+`TempleOS`) | sidebar button, `N`-key system |
| `PlayerViewPreferences.local_view` | **second copy of ViewMode** | same | same writers **plus `game_over.rs`** |
| `CameraViewMode` | plain `Resource` | normal / cinematic | game-over cinematic |

Problems visible in the table alone:

- `GameMode` is a resource, so there are **no OnEnter/OnExit hooks per mode** —
  every mode-specific setup/teardown is ad-hoc `run_if(in_mode(...))` checks.
- The view mode has **two sources of truth**. Both toggle sites sync them
  manually; `game_over.rs::setup_cinematic_camera` wrote only one of them
  (fixed 2026-07-16). Any future writer can silently desync them again.
- The piece-visibility system runs on `resource_changed::<ViewMode>` — a
  **missed write is unrecoverable**: nothing ever re-applies the correct
  visibility (this is the "all pieces disappeared" family of bugs).

### 1.2 Entity domains (who owns which pieces?)

| Domain | Markers | Scope mechanism | Spawned by |
|--------|---------|-----------------|-----------|
| Game pieces | `Piece` (+children `Piece3DVisual`, `Piece2DVisual`, `PiecePickingProxy2D`) | custom `DespawnOnExit(InGame)` | `create_pieces` / `spawn_pieces_from_fen` |
| Menu ambient pieces | `MenuBg` + `MenuBgPieceHome` | custom `DespawnOnExit(MainMenu)` | `spawn_menu_bg_pieces` (self-healing, world-keyed) |
| Cinematic pieces | `MenuBg` + `CinematicPiece` | custom `DespawnOnExit(MainMenu)` | `cinematic::spawn_piece` |
| Replay pieces | `Piece` | `DespawnOnExit(InGame)` + `cleanup_replay` | `spawn_piece_at_replay` |
| LEARN-box mini showcase | `MINI_LAYER` render layer | own plugin | `xf_animate` |
| Board, lights, HUD | various | mix of `DespawnOnExit` + per-plugin `OnExit` systems | several plugins |

### 1.3 Cleanup mechanisms (four in parallel — this is the core problem)

1. **Custom `DespawnOnExit<GameState>`** + hand-rolled per-state cleanup systems
   (`state_lifecycle.rs`: `cleanup_main_menu`, `cleanup_in_game`, …) registered
   on each `OnExit`. This predates / duplicates Bevy's built-in `StateScoped`.
2. **Per-plugin `OnExit` systems**: `reset_pieces_spawned`, `reset_game_camera`,
   `clear_puzzle_board`, `teardown_sky`, `despawn_orbital_lights`, …
3. **Self-healing spawns**: `spawn_menu_bg_pieces` re-spawns whenever no
   `MenuBgPieceHome` exists (good pattern — world-keyed).
4. **Boolean flags**: `PiecesSpawned`, `MenuBgPiecesSpawned`. The comment in
   `new_menu.rs` itself admits bool flags "can desync from reality" — yet game
   pieces still use one.

Visibility is a fifth, informal axis: parent `Visibility` propagates to
children, while `view_mode_rendering_toggle_system` toggles **child** visuals —
two systems fighting over the same result with no single owner.

---

## 2. Defects found (root-caused this session)

| # | Defect | Root cause | Status |
|---|--------|-----------|--------|
| D1 | New game starts with extra pieces on non-start squares | Menu ambient pieces leaked past the MainMenu→InGame transition; the 30 still on home squares hide under the fresh spawn, only ambient-animation-moved ones are visible | **Stopgap landed**: `purge_stale_board_visuals` (OnEnter(InGame), first in both chains) despawns `MenuBg`/`Piece` leftovers and `warn!`s each by name. Root transition hole still open — the warn log will name it next occurrence |
| D2 | 2D→3D toggle: all pieces disappear | View mode has two sources of truth keyed by change-detection; a missed/one-sided write leaves child visuals hidden forever. `game_over.rs` wrote prefs without the resource | **Partial fix landed** (game_over sync). Full fix = Phase 1 |
| D3 | After quitting, flat piece-blobs render on the menu board | InGame-scoped 2D picking quads survived into MainMenu **and/or** menu pieces spawned while the stale 2D view state applied; ambient-hide system (`main_menu.rs` hides `MenuBgPieceHome`) can also leave shadow-only pieces | Open — Phase 1 + Phase 2. The D1 purge stops re-entry contamination; the reverse direction (game→menu) still relies on `cleanup_in_game` |
| D4 | Dead black band right of the in-game sidebar | `update_game_viewport` shrank the camera viewport, but the same camera renders egui — panel moved with it; net effect was only the band | **Fixed** (system removed) |
| D5 | Dead/deprecated code confusing the picture | `toggle_3d_visibility` is an empty no-op; deprecated `.and`/`.or` run-condition combinators throughout | Open — Phase 1 cleanup |
| D6 | `PiecesSpawned` bool flag | Same footgun class the menu already fixed with world-keyed spawning | Open — Phase 2 |

---

## 3. Target architecture

**One rule: every spawned entity carries exactly one scope, and every piece of
"where are we" state has exactly one owner.**

1. **View mode becomes a Bevy `SubState` of `InGame`** (or minimally: one
   resource, with `PlayerViewPreferences` demoted to persistence-only, applied
   at one sync point on game entry). Visibility application becomes an
   `OnEnter(ViewMode::…)` transition system — idempotent, schedule-driven, and
   re-applied on game entry so a fresh spawn in 2D mode starts correctly.
   `on_piece_added` observer must apply the *current* mode to newly spawned
   pieces (today a piece spawned mid-2D-mode gets default visibility).
2. **`GameMode` becomes `SubStates` of `GameState::InGame`.** Per-mode
   setup/teardown moves to `OnEnter/OnExit(GameMode::…)` schedules instead of
   `run_if(in_mode(...))` sprinkled at call sites. `PgnReplay` stops being a
   pseudo-mode that aliases InGame.
3. **Replace custom `DespawnOnExit` with Bevy's built-in state-scoped
   despawning** (`StateScoped`/`DespawnOnExit` in Bevy 0.19). One engine-tested
   mechanism; the hand-rolled `state_lifecycle.rs` cleanup systems are deleted.
   Keep `audit_despawn_markers`/`periodic_entity_audit` as debug observers, and
   extend the audit to **flag any `Mesh3d`/`Sprite` entity with no scope
   marker** — unscoped renderables become a logged bug, not a mystery.
4. **World-keyed spawn guards everywhere**: drop `PiecesSpawned`; `create_pieces`
   runs when `Query<&Piece>` is empty and meshes are loaded (the menu already
   proved this pattern).
5. **Explicit domain markers**: `GameEntity`, `MenuEntity`, `ReplayEntity` —
   cheap, greppable, and lets purge/audit systems be exhaustive instead of
   enumerating today's marker zoo (`MenuBg`, `CinematicPiece`, `Piece`, …).

---

## 4. Phased execution plan

### Phase 0 — Stopgaps (landed 2026-07-16)

- [x] `purge_stale_board_visuals` OnEnter(InGame), first in both entry chains,
      with `warn!` breadcrumbs (`game/systems/game_init.rs`, `game/plugin.rs`)
- [x] Remove `update_game_viewport` viewport shrink (`game/systems/camera.rs`)
- [x] Sync `ViewMode` resource in `game_over.rs::setup_cinematic_camera`
- [x] Centering fixes: exit dialog, create-game, draw/rematch offers, game-over
      popup, tier-up label
- [x] **Camera framing invariant** (`cinematic.rs::cinematic_camera_system`):
      intro/attract cinematic shots must never crop the board. A framing guard
      runs after each shot's transform is computed: perspective shots dolly
      back along the view axis, orthographic shots widen `viewport_height`,
      until all 8 board bounding corners (x,z ∈ [-0.5, 7.5], up to 2.0 high)
      fit with a 0.90 margin. Applies automatically to any future
      `CameraAngle`/moment. "Press Enter to Start" prompt moved lower
      (CENTER_BOTTOM −34).

**Standing rule (applies to all phases): any camera that frames the board —
menu orbit, cinematic shots, game camera presets — must keep the entire board
inside the frustum. New shots/presets go through `board_fit_corners()`-style
verification rather than hand-tuned offsets.**

### Phase 1 — View-mode unification (small, high value) — ✅ DONE 2026-07-18

- [x] Removed `PlayerViewPreferences` entirely; `ViewMode` is now the single
      source of truth. `toggle_view()` became `ViewMode::toggle()`. All writers
      (V-key `view_mode_toggle_input_system`, in-game/replay UI toggles,
      `game_over` cinematic, TempleOS menu item) mutate only `ViewMode`; every
      reader already keyed off it. Desync is now structurally impossible.
- [x] Deleted the dead `view_mode_systems.rs` (empty `toggle_3d_visibility`
      no-op + unused `view_mode_is_3d/2d`) and its registration.
- [x] Made visibility application idempotent + also run on `OnEnter(InGame)`
      (in addition to `resource_changed::<ViewMode>` / `PiecesSpawned`), so a
      fresh game always applies the current mode.
- [x] Verified: builds clean, launches, runs without panics/recovered errors.
  (Deferred: the deprecated `.and`/`.or` combinator cleanup — cosmetic warnings,
  not part of the correctness fix; roll into Phase 2's touch of those files.)

### Phase 2 — Cleanup unification — PARTIAL (leak audit landed 2026-07-18)

Files: `core/states.rs`, `core/state_lifecycle.rs`, every `DespawnOnExit` call
site (mechanical), `rendering/pieces/pieces.rs`.

- [x] **Leak audit landed** — `audit_cross_state_leaks` (state_lifecycle.rs,
      registered in core/plugin.rs Update) warns (throttled) when `Piece`
      entities survive into MainMenu/Auth or `MenuBg` entities appear outside
      MainMenu — i.e. the exact D1/D3 signatures now name themselves in the log.
- [ ] **DEFERRED (deliberately)** — Swap `crate::core::DespawnOnExit` for Bevy's
      built-in state-scoped component + delete the four hand-rolled cleanup
      systems. This is a 71-call-site rename on a **working, centralised**
      mechanism; Bevy 0.19-rc.3's built-in is also named `DespawnOnExit`, and a
      missed `enable_state_scoped_entities` registration would silently disable
      ALL cleanup (worse than the bug we're fixing). High churn, low incremental
      value, runtime-verification-heavy → do as its own playtested pass, not
      blind. The custom mechanism stays until then.
- [ ] **DEFERRED** — Drop `PiecesSpawned`. It is load-bearing: the visibility
      re-apply keys off `resource_changed::<PiecesSpawned>`. Removing it needs
      the world-keyed spawn guard + a replacement re-apply trigger, tested at
      runtime. (Phase 1's `OnEnter(InGame)` apply covers the fresh-game case
      already, so this is lower-priority now.)
- [ ] Acceptance (for the deferred items): loop MainMenu→InGame→MainMenu 20× —
      entity count returns to baseline every cycle; zero audit warnings.

### Phase 3 — GameMode as substates

Files: `core/states.rs`, `game/plugin.rs`, multiplayer/matchmaking flows,
`game_over_popup.rs` (review flow), replay plugin.

- [ ] `#[derive(SubStates)] #[source(GameState = GameState::InGame)]` for
      `GameMode`; entry points set `NextState<GameMode>` instead of mutating a
      resource.
- [ ] Move mode-specific setup/teardown (P2P session, competitive sidebar
      state, spectator sync, replay resources) into per-mode OnEnter/OnExit.
- [ ] Disconnect/forfeit teardown for online modes gets its own exit path —
      today it is entangled with generic InGame exit.
- [ ] Acceptance: each mode entered and exited twice in one app run leaves no
      residue (audit clean); Review Game → replay → back works from every mode.

### Phase 4 — Regression harness

- [ ] Headless Bevy tests (`tests/`): drive `NextState` transitions through the
      full matrix and assert entity-count invariants per domain marker.
- [ ] Transition matrix to cover:

| From \ To | MainMenu | InGame(SP) | InGame(Local) | InGame(Online) | InGame(Competitive) | InGame(Replay) | GameOver |
|-----------|----------|-----------|---------------|----------------|--------------------|----------------|---------|
| MainMenu | — | ✓ | ✓ | ✓ | ✓ | ✓ | n/a |
| InGame(any) | ✓ (quit) | — | — | — | — | ✓ (review) | ✓ |
| GameOver | ✓ | ✓ (rematch) | — | ✓ (rematch) | ✓ | ✓ (review) | — |

  Each cell asserts: correct pieces on board, zero foreign-domain entities,
  view mode correctly applied, timers/HUD reset.

- [ ] Manual e2e script (10 min, run before releases): start each mode, toggle
      2D/3D twice, quit to menu, confirm menu board intact, start next mode.
      Watch the log for `[GAME_INIT] Purging stale board visual` — any hit is a
      regression to file.

---

## 5. How to verify today's stopgaps

1. `just dev` → start any game: board must have exactly 32 pieces. If the menu
   leak recurs, the log now prints the leaked entities by name — attach that to
   the issue.
2. In game: 2D View → 3D View repeatedly; finish a game (cinematic) and check
   pieces are visible afterwards.
3. Sidebar sits flush against the window's right edge (no black band).
4. ESC exit dialog, CREATE GAME, draw/rematch offers, game-over popup: button
   rows centered.
