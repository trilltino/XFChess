# XFChess Performance Fix Plan

## Overview

Three distinct categories of stutters identified via codebase audit:
1. **AI move lag** — Stockfish process lifecycle and redundant validation
2. **Click lag** — synchronous board sync + uncached legal move generation on every click
3. **Visual stutter** — entity despawn/spawn cycle on every selection change

---

## Phase 1 — AI Move Lag (Biggest Win)

**Target: cut 500ms–1s+ per AI move**

### 1.1 Reuse persistent Stockfish process

- **File**: `src/game/ai/systems.rs`
- **Problem**: Lines 325–330 spawn a brand-new `Command::new(stockfish)` on every AI move. The `StockfishProcess` resource (lines 24–46) is already defined but never wired up.
- **Fix**:
  - On app startup (or first AI move), spawn Stockfish once and store stdin/stdout in `StockfishProcess`
  - Each move: send `position fen <fen>` + `go movetime <ms>` to the existing process
  - On app exit / game reset: send `quit`
- **Expected gain**: eliminates cold-start overhead (~500ms–1s)

### 1.2 Remove blocking sleep in async task

- **File**: `src/game/ai/systems.rs:339`
- **Problem**: `std::thread::sleep(Duration::from_millis(100))` blocks the async executor thread
- **Fix**: Replace with a proper `readyok` handshake — send `isready`, read lines until `readyok` is received
- **Expected gain**: removes guaranteed 100ms stall

### 1.3 Validate AI move without full legal move generation

- **File**: `src/game/ai/systems.rs:574–590`
- **Problem**: After Stockfish returns a move, code calls `engine.legal_moves()` (generates ALL ~30–40 moves, clones Game struct per candidate) just to check if one move is legal
- **Fix**: Call `is_legal_move(src, dst)` directly — single check, no allocation
- **Expected gain**: eliminates O(n) allocation pass per AI move completion

---

## Phase 2 — Click Lag

**Target: make piece selection feel instant**

### 2.1 Cache legal moves in Selection resource

- **Files**: `src/game/systems/input.rs`, wherever `Selection` is defined
- **Problem**: Every click calls `sync_ecs_to_engine_with_transform()` (full ECS walk) then `get_legal_moves_for_square()` (clones Game per candidate). No result is stored.
- **Fix**:
  - Add `cached_legal_moves: HashMap<(u8,u8), Vec<(u8,u8)>>` to `Selection` (or a new `LegalMoveCache` resource)
  - Populate cache once per turn, immediately after a move is executed (when board state is already known clean)
  - On click: read from cache — no sync, no generation
  - Invalidate on: move executed, game reset, board loaded

### 2.2 Remove redundant sync calls

Three separate sync calls that all fire within the same logical "turn change":

| Location | Line | Action |
|---|---|---|
| `src/game/systems/input.rs` | 196 | Remove — cache covers this |
| `src/game/systems/game_logic.rs` | 75 | Remove — engine already updated by `execute_move` |
| `src/game/ai/systems.rs` | 219–221 | Remove — board was synced when move was made |

- **Fix**: Establish one canonical sync point: inside `execute_move` (or the system that applies a move). All downstream code reads from the already-synced engine state.

### 2.3 Stop cloning Game struct per legality check

- **File**: `src/engine/board_state.rs:215–234`
- **Problem**: `is_legal_move()` clones the entire `Game` struct for each pseudo-legal move candidate (~30–40 per piece)
- **Fix**: Use make/unmake move pattern instead of clone — apply the move to the existing struct, check for check, then unmake. Avoids heap allocation per candidate.
- **Note**: Only needed if Phase 2.1 cache doesn't cover all call sites. Audit after 2.1.

---

## Phase 3 — Visual Stutter on Selection

**Target: eliminate entity churn on click**

### 3.1 Reuse hint entities instead of despawn/spawn

- **File**: `src/game/systems/visual.rs:43–90`
- **Problem**: On every selection change — despawns all marker entities, iterates all 64 squares, spawns new marker entities (~15–20 entity creates per click). Each spawn allocates mesh/material handles.
- **Fix**:
  - Pre-spawn a fixed pool of 64 hint marker entities on board setup, hidden by default (`Visibility::Hidden`)
  - On selection change: show/hide the relevant subset, update their `Transform` if needed
  - Never despawn/spawn during gameplay
- **Expected gain**: zero allocation on selection, just visibility toggles

### 3.2 Only iterate affected squares

- **File**: `src/game/systems/visual.rs:47`
- **Problem**: Iterates all 64 squares to update materials even though at most ~10 change per selection
- **Fix**: Track previously highlighted squares in `Selection`. On change, only touch the diff (deselect old set, select new set).

---

## Phase 4 — Minor / Low Priority

These are worth doing after the above but won't have dramatic impact:

| Issue | File | Fix |
|---|---|---|
| King entity lookup on every check | `src/rendering/effects/check_highlight.rs:35` | Cache king entity in a resource on spawn |
| AI depth not capped for blitz | `src/game/ai/systems.rs:150–177` | Add blitz depth cap (≤8) alongside bullet cap |
| `update_check_highlight` iterates all pieces | `src/rendering/effects/check_highlight.rs` | Use cached king entity ref |

---

## Implementation Order

```
Phase 1.1  →  Phase 1.2  →  Phase 1.3
     ↓
Phase 2.1  →  Phase 2.2  →  Phase 2.3 (if needed)
     ↓
Phase 3.1  →  Phase 3.2
     ↓
Phase 4 (as time allows)
```

Phase 1 can be done independently of Phase 2/3. Phases 2 and 3 share the concept of "one sync point per turn" and should be done together.

---

## Success Criteria

| Metric | Before | Target |
|---|---|---|
| AI move response (Stockfish) | 500ms–1.5s | < 100ms (excluding think time) |
| Piece click → highlights visible | ~50–150ms | < 16ms (1 frame) |
| Animation stutter at idle | Yes | No (WinitSettings fix already applied) |
| Entity spawns per click | ~15–20 | 0 |
