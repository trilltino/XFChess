# Warning Remediation Plan

All warnings captured from a clean `just dev` build (2026-05-26).
Policy: **Remove** trivially dead code; **Integrate** warnings that point to a missing implementation.

---

## Legend

| Symbol | Meaning |
|--------|---------|
| RM | Delete the dead code |
| INT | Wire up the value — it was computed but never plugged in |
| FIX | Non-trivial change (deprecated API, wrong pattern, etc.) |

---

## 1. `crates/braid-core` — 3 warnings

`crates/braid-core/src/lib.rs` references feature flags `blob` and `fs` that are never declared in the crate's own `[features]` table, triggering `unexpected_cfg` errors.

| # | Line | Type | Action |
|---|------|------|--------|
| 1 | 16 | unexpected\_cfg `blob` | RM — add `blob = []` to `braid-core/Cargo.toml` `[features]` |
| 2 | 19 | unexpected\_cfg `fs` | RM — add `fs = []` to `braid-core/Cargo.toml` `[features]` |
| 3 | 40 | unexpected\_cfg `blob` (second site) | resolved by fix #1 |

**Single change:** in `crates/braid-core/Cargo.toml` add under `[features]`:
```toml
blob = []
fs   = []
```

---

## 2. `crates/xfchess-anticheat` — 5 warnings

All unused imports.

| # | File | Line | Item | Action |
|---|------|------|------|--------|
| 1 | `engine/stockfish.rs` | 4 | `tokio::time::timeout` | RM |
| 2 | `engine/stockfish.rs` | 5 | `warn` from `tracing` | RM — keep `debug` |
| 3 | `report/store.rs` | 3 | `warn` from `tracing` | RM — keep `info` |
| 4 | `lib.rs` | 12 | `std::sync::Arc` | RM |
| 5 | `lib.rs` | 22 | `Complexity` from import list | RM |

---

## 3. `crates/nimzovich_engine` — 18 warnings

### 3a. Unused import (1)

| # | File | Line | Item | Action |
|---|------|------|------|--------|
| 6 | `on_chain_moves.rs` | 12 | `CompactBoard` | RM from import list |

### 3b. Unused variables — stub implementations (2) — INTEGRATE

| # | File | Line | Variable | Action |
|---|------|------|----------|--------|
| 7 | `search/alphabeta.rs` | 67 | `old_alpha` | INT — use in `store_tt_entry` to classify TT bound type: `EXACT` when `old_alpha < best_score < beta`, `ALPHA` when `best_score <= old_alpha`, `BETA` when `best_score >= beta`. Add a `BoundType` enum to the hash module and thread it through. |
| 8 | `search/alphabeta.rs` | 170 | `rbeta` | INT — the ProbCut block (lines 166–173) computes `rbeta` then does nothing. Either: (a) implement the SEE-based probe loop using `rbeta`, or (b) remove the entire block. **Recommended: remove the stub block** since SEE is not yet available in this search path; ProbCut can be re-added when SEE is wired up. |

### 3c. Unused variables — truly dead (3)

| # | File | Line | Variable | Action |
|---|------|------|----------|--------|
| 9 | `on_chain_attack.rs` | 231 | `shift` param in `ray_attacks` | RM — rename to `_shift` or remove from signature if no caller depends on it by position |
| 10 | `on_chain_moves.rs` | 268 | `saved_kings` | RM — the function already copies `*g` into `tmp`; `saved_kings` is never restored |
| 11 | `on_chain_moves.rs` | 417,420,432 | `idx += 1` (3 dead assignments at end of blocks) | RM — remove the trailing `idx += 1` after the last `out[idx] = Some(...)` assignment in the castling and pawn-capture branches |

### 3d. Dead constants (4)

| # | File | Line | Item | Action |
|---|------|------|------|--------|
| 12 | `constants.rs` | 199 | `WR0` | RM |
| 13 | `constants.rs` | 201 | `WR7` | RM |
| 14 | `constants.rs` | 202 | `BR56` | RM |
| 15 | `constants.rs` | 204 | `BR63` | RM |

### 3e. Dead functions (2)

| # | File | Line | Item | Action |
|---|------|------|------|--------|
| 16 | `evaluation/pst.rs` | 97 | `get_pst_value` | RM — PST tables are accessed directly everywhere; the wrapper is never called |
| 17 | `move_gen/attack.rs` | 288 | `find_king` | RM — king square is already tracked in `Game::white_king` / `black_king`; this function duplicates a board scan nobody calls |

### 3f. Dead enum variant + dead methods (2 items, 3 symbols)

| # | File | Line | Item | Action |
|---|------|------|------|--------|
| 18 | `search/move_picker.rs` | 18 | `Stage::InitCaptures` variant | RM — `classify_moves` is called eagerly in `new()`; the variant was for a lazy path that was never wired up. Also remove the `_ => Stage::Done` wildcard arm in `next_move` once the enum is exhausted. |
| 19 | `search/move_picker.rs` | 146/153 | `len` / `is_empty` | RM — no caller exists anywhere in the crate |

### 3g. Dead `SearchParams` fields — INTEGRATE or REMOVE

`SearchParams` is a SPSA-tuned config struct; many fields are declared but not read by the search.

**Integrate (use SP field instead of hardcoded constant):**

| # | File | Current code | SP field to use | Action |
|---|------|-------------|-----------------|--------|
| 20 | `alphabeta.rs:227` | `mv.score < -500` | `SP.see_quiet_margin` | INT — replace `-500` with `SP.see_quiet_margin as i16` |
| 21 | `alphabeta.rs:232` | `mv.score < -2000` | `SP.see_nonquiet_margin` | INT — replace `-2000` with `SP.see_nonquiet_margin as i16` |

**Remove (no corresponding implementation; stubs carry tuning risk):**

| # | Field(s) | Reason to remove |
|---|----------|-----------------|
| 22 | `qsee_margin` | quiescence SEE pruning not implemented |
| 23 | `check_prune_margin` | check-extension pruning not implemented |
| 24 | `futility_hist_mul`, `futility_improving` | extended futility (history-scaled, improving-adjusted) not implemented; simple version uses only `futility_base`+`futility_mul*depth` |
| 25 | `iid_depth` | Internal Iterative Deepening not implemented |
| 26 | `chist_depth`, `chist1_margin`, `chist2_margin` | counter-move history pruning not implemented |
| 27 | `se_depth`, `se_depth_margin` | singular extension not implemented |
| 28 | `qhistory_mul`, `qhistory_base`, `qhpen_mul`, `qhpen_base` | quiet-history update uses raw `bonus`; scaling not wired up |
| 29 | `chistory_mul`, `chistory_base`, `chpen_mul`, `chpen_base` | continuation history scaling not wired up |
| 30 | `beta_bonus` | unused |
| 31 | `corrhist_grain`, `corrhist_weight`, `corrhist_max`, `corr_depth_base`, `corr_ch_weight`, `corr_np_weight`, `corr_pawn_weight`, `corr_kbn_weight`, `corr_kqr_weight` | correction history not implemented |
| 32 | `eval_scale` | eval output is not scaled |

Remove each field from the struct body **and** from `sarah_tuned()`.

### 3h. Dead utility function (1)

| # | File | Line | Item | Action |
|---|------|------|------|--------|
| 33 | `utils.rs` | 38 | `create_boxed_array` | RM |

---

## 4. `backend` — 12 warnings

### 4a. Unused imports (9)

| # | File | Line | Item | Action |
|---|------|------|------|--------|
| 34 | `signing/social/routes.rs` | 18 | `tracing::warn` | RM |
| 35 | `signing/routes/chat.rs` | 15 | `put` from `routing` | RM |
| 36 | `signing/routes/matchmaking/handlers.rs` | 20 | `SharedMatchmakingState` | RM |
| 37 | `signing/routes/archive.rs` | 4 | `IntoResponse` | RM |
| 38 | `signing/routes/admin.rs` | 13 | `Arc` from `std::sync` | RM — keep `Mutex` |
| 39 | `signing/routes/admin.rs` | 481 | local `use std::str::FromStr` | RM |
| 40 | `signing/routes/global_session.rs` | 32 | `error` from `tracing` | RM — keep `info`, `warn` |
| 41 | `tasks/archiver.rs` | 4 | `Seek`, `SeekFrom` from `std::io` | RM — keep `Write`, `Read` |
| 42 | `tasks/archiver.rs` | 7 | `warn` from `tracing` | RM — keep `info`, `error` |

### 4b. Unreachable pattern (1)

| # | File | Line | Item | Action |
|---|------|------|------|--------|
| 43 | `signing/routes/history.rs` | 91 | `_ => "unknown"` arm | RM — the previous `None => "unknown"` arm already covers this |

### 4c. Unused variables (2)

| # | File | Line | Variable | Action |
|---|------|------|----------|--------|
| 44 | `signing/social/friends.rs` | 210 | `to_node` in destructure | RM — replace with `_` in the tuple pattern |
| 45 | `signing/routes/external_elo.rs` | 111 | `state` extractor | RM — rename to `_state` |

---

## 5. `programs/xfchess-game` (Solana program) — 9 warnings

All are handler function parameters named `tournament_id` or `source_match_index` that are passed by the Anchor framework but not read in the body. The PDA seed already encodes the ID; reading it again is usually unnecessary.

| # | File | Line | Variable | Action |
|---|------|------|----------|--------|
| 46 | `crank_ix/crank_time_check.rs` | 47 | `timed_out_player` | RM — prefix with `_` |
| 47 | `tournament_ix/lifecycle/close_tournament.rs` | 34 | `tournament_id` param | RM — prefix with `_tournament_id` |
| 48 | `tournament_ix/lifecycle/start.rs` | 50 | `tournament_id` param | RM — prefix with `_tournament_id` |
| 49 | `tournament_ix/registration/register.rs` | 74 | `tournament_id` param | RM — prefix with `_tournament_id` |
| 50 | `tournament_ix/registration/leave.rs` | 54 | `tournament_id` param | RM — prefix with `_tournament_id` |
| 51 | `tournament_ix/matches/record_result.rs` | 33 | `tournament_id` param | RM — prefix with `_tournament_id` |
| 52 | `tournament_ix/matches/record_result.rs` | 106 | `source_match_index` param | RM — prefix with `_source_match_index` |
| 53 | `tournament_ix/matches/record_swiss_result.rs` | 61 | `tournament_id` param | RM — prefix with `_tournament_id` |
| 54 | `tournament_ix/prizes/fund_prize.rs` | 52 | `tournament_id` param | RM — prefix with `_tournament_id` |

---

## 6. `src/` (game client) — 20 warnings

### 6a. Unused imports (8)

| # | File | Line | Item | Action |
|---|------|------|------|--------|
| 55 | `engine/board_state.rs` | 13 | `Color` | RM |
| 56 | `game/systems/camera.rs` | 482 | `crate::rendering::pieces::PieceColor` (in test mod) | RM |
| 57 | `game/replay.rs` | 16–17 | `CapturedPieces`, `CurrentGamePhase`, `CurrentTurn`, `GameOverState`, `GameTimer`, `MoveHistory`, `PendingTurnAdvance`, `Selection`, `TurnStateContext` (all 9) | RM — entire import block |
| 58 | `multiplayer/solana/global_session_manager.rs` | 20 | `directories::ProjectDirs` | RM |
| 59 | `rendering/effects/check_highlight.rs` | 6 | `PieceColor` | RM — keep `Piece`, `PieceType` |
| 60 | `states/main_menu/new_menu.rs` | 13 | `GameMode as CoreGameMode` | RM |
| 61 | `states/main_menu/board_animation.rs` | 9 | `DespawnOnExit`, `GameState` | RM — entire `use crate::core::{...}` line |
| 62 | `ui/account/solana_panel.rs` | 10 | `GlobalSessionCheckPending` | RM |

### 6b. Deprecated API — FIX (3)

| # | File | Line | Old call | Replacement |
|---|------|------|----------|------------|
| 63 | `states/main_menu/screens.rs` | 1382 | `egui::Frame::none()` | `egui::Frame::NONE` |
| 64 | `states/main_menu.rs` | 826 | `egui_ctx.screen_rect()` | `egui_ctx.content_rect()` |
| 65 | `ui/game/game_ui.rs` | 1396 | `egui::Frame::none()` | `egui::Frame::NONE` |

### 6c. Unused variables (6)

| # | File | Line | Variable | Action |
|---|------|------|----------|--------|
| 66 | `game/systems/camera.rs` | 836 | `ai_config` system param | RM — prefix with `_ai_config` |
| 67 | `game/systems/camera.rs` | 1006 | `ai_config` system param (second system) | RM — prefix with `_ai_config` |
| 68 | `multiplayer/solana/lobby.rs` | 675 | `rpc_url` | RM — prefix with `_rpc_url` |
| 69 | `multiplayer/solana/lobby.rs` | 679 | `program_id` | RM — prefix with `_program_id` |
| 70 | `states/main_menu/screens.rs` | 763 | `is_free_game` | RM — `is_free_casual` and `is_free_rated` already cover every code path; the combined variable adds nothing |
| 71 | `ui/game/game_ui.rs` | 14 | `b1c3` (and adjacent `b8c6`) | RM — entire line of dead test coordinates |
| 72 | `ui/game/game_2d.rs` | 400 | `ai_config` system param | RM — prefix with `_ai_config` |

### 6d. Unnecessary parentheses (1)

| # | File | Line | Action |
|---|------|------|--------|
| 73 | `ui/menus/game_over_popup.rs` | 431 | RM — `match (elapsed % 3)` → `match elapsed % 3` |

### 6e. Dead field (1)

| # | File | Line | Item | Action |
|---|------|------|------|--------|
| 74 | `multiplayer/rollup/bridge.rs` | 85 | `is_delegated` field on `PendingFinalization` | INT — the field is set when the struct is created but never read in the finalization logic. Read it in the finalize handler: when `is_delegated` is `true`, ensure the undelegation step is triggered before calling `finalize_game`. If undelegation tracking is handled elsewhere, RM the field. |

---

## 7. `tauri/` — 16 warnings

### 7a. Unused imports (10)

| # | File | Line | Item | Action |
|---|------|------|------|--------|
| 75 | `main.rs` | 30 | `tauri_plugin_deep_link::DeepLinkExt` | RM |
| 76 | `main.rs` | 46 | `AppError`, `AppResult` | RM — `error.rs` stays (has tests); just remove the dead imports |
| 77 | `main.rs` | 47 | `services::auth::AuthState` | RM |
| 78 | `main.rs` | 48 | `get_admin_api_key`, `get_wallet_port` | RM |
| 79 | `main.rs` | 50 | `windows::tournament_admin::TournamentAdminWindow` | RM |
| 80 | `services/config.rs` | 16 | `std::env` | RM |
| 81 | `services/notification_poller.rs` | 9 | `Manager` | RM — keep `AppHandle` |
| 82 | `services/notification_poller.rs` | 11 | `warn` | RM — keep `error`, `info` |
| 83 | `utils/logging.rs` | 2 | `debug` | RM |
| 84 | `windows/wallet.rs` | 4 | `WindowCommands` | RM — keep `IpcServer` |

### 7b. Deprecated API — FIX (2)

| # | File | Line | Old call | Action |
|---|------|------|----------|--------|
| 85 | `services/ipc.rs` | 113 | `app.shell().open(&url, None)` | FIX — add `tauri-plugin-opener` to `tauri/Cargo.toml` + `tauri.conf.json` permissions, then use `app.opener().open_url(&url, None::<&str>)` |
| 86 | `main.rs` | 661 | `.menu_on_left_click(true)` | FIX — rename to `.show_menu_on_left_click(true)` |

### 7c. Unused variable (1)

| # | File | Line | Variable | Action |
|---|------|------|----------|--------|
| 87 | `main.rs` | 588 | `handle` | RM — prefix with `_handle` |

### 7d. Dead function (1)

| # | File | Line | Item | Action |
|---|------|------|------|--------|
| 88 | `main.rs` | 101 | `load_persisted_wallet` | RM — entire function body + signature |

---

## Summary by effort

| Effort | Count | Items |
|--------|-------|-------|
| Trivial (remove import / prefix var) | 62 | #1–6, #9–10, #12–15, #19, #34–45, #46–54, #55–62, #66–73, #75–84, #87 |
| Small (delete dead fn/variant/field) | 8 | #11, #16–18, #70–71, #74, #88 |
| Medium (use SP field, fix deprecated API) | 5 | #20–21, #63–65, #85–86 |
| Complex (integrate into search) | 3 | #7 (`old_alpha` TT bounds), #8 (remove ProbCut stub), #74 (`is_delegated`) |
| Config-only | 1 | #1–3 (braid-core Cargo.toml features) |
| Bulk struct field removal | 11 fields | #22–32 (`SearchParams` dead fields) |

**Total tracked warnings: 88 items (covering all 86 compiler warning lines).**

---

## Execution order

1. **Config** — braid-core Cargo.toml features (#1–3)
2. **Crate-level dead code** — nimzovich\_engine constants/fns/variant/struct fields (#12–33)
3. **Search integration** — `old_alpha` TT bounds, ProbCut stub (#7–8)
4. **Bulk unused imports** — all crates (#34–42, #55–62, #75–84)
5. **Backend cleanup** — unreachable pattern, unused vars (#43–45)
6. **Solana program** — parameter prefixes (#46–54)
7. **Game client** — deprecated egui API, dead field, remaining vars (#63–74)
8. **Tauri** — deprecated API + dead function (#85–88)
9. **Verify** — `just dev` with zero warnings
