# Braid UI Wiring Plan

Hooks up Phase 4 (PGN export), Phase 2 (spectator), and Phase 5 (clock/chat) into the
actual Bevy/egui UI surfaces. All the networking plumbing is done; this plan is about
closing the gap between "data arrives" and "player sees it".

---

## 1. Phase 4 — PGN Export into Review / Save buttons

### The gap

`apply_pgn_export_result` (bridge.rs) inserts `ParsedPgnGameResource` from the VPS Braid
log. But the **Review** / **Analyze** / **Save PGN** buttons in `game_over_popup.rs` read
from `CachedGamePgn`, which is built from the *local* `MoveHistory` at game-over time
(`cache_pgn_on_game_over`). The Braid-fetched version is never seen by the buttons.

### Fix

**File: `src/multiplayer/rollup/bridge.rs`**
- In `apply_pgn_export_result`, add `Option<ResMut<crate::ui::menus::game_over_popup::CachedGamePgn>>`
  to the system params.
- When `pgn_rx` delivers `Some(pgn)`:
  1. Insert `ParsedPgnGameResource` (already done).
  2. Also write into `CachedGamePgn`:
     ```rust
     if let Some(ref mut cached) = cached_pgn {
         cached.pgn_string = pgn_to_string_from_parsed(&pgn); // helper below
         cached.pgn = Some(pgn.clone());
     }
     ```
  3. Set a flag: `CachedGamePgn.braid_pgn_ready = true` (new field, see below).

**File: `src/ui/menus/game_over_popup.rs`**
- Add field to `CachedGamePgn`:
  ```rust
  pub braid_pgn_ready: bool,
  ```
- In the Review/Analyze/Save row: while `!cached_pgn.braid_pgn_ready`, show a small
  spinner or dim the buttons with tooltip "Fetching game record…" so the player knows
  it is loading. Once ready, buttons work normally.
- `cache_pgn_on_game_over` resets `braid_pgn_ready = false` so the flag is fresh each game.

**Helper** (can live in `game_over_popup.rs` or `replay_braid.rs`):
```rust
pub fn pgn_to_string_from_parsed(pgn: &ParsedPgnGame) -> String { ... }
```
Formats the standard PGN header + moves text from an already-parsed game (reuses the
same loop as `braid_move_log_to_pgn_text`).

### Result
After a multiplayer game: player sees the popup, buttons are dimmed for ≈1–2 s while
the VPS fetch completes, then all three (Review / Analyze / Save PGN) light up using
the authoritative Braid move log.

---

## 2. Phase 2 — Spectator Watch button → board

### Current state
- `screens.rs:416` Watch button only calls `info!(...)`, nothing else.
- `SpectatorPlugin` is wired: `SpectateViaLinkEvent` → `handle_spectate_link` →
  `GameMode::Spectator` + `GameState::InGame`.
- VPS poll (2 s) already dispatches `NetworkMoveEvent` to drive the board.
- `broadcast_snapshot_to_new_peer` sends `GameSnapshot` when a peer joins, but the
  spectator is not yet subscribing to the iroh gossip topic.

### Step 1 — Wire the Watch button

**File: `src/states/main_menu/screens.rs`**

Add `MessageWriter<SpectateViaLinkEvent>` to `render_spectator_popup`'s system params.

Replace the stub:
```rust
// BEFORE
info!("[SPECTATOR] Watch clicked for game {}", game_id);

// AFTER
spectate_writer.write(SpectateViaLinkEvent { game_id: game_id.clone() });
competitive.show_spectator_popup = false;
```

Also close the popup so it doesn't linger over the board.

### Step 2 — Braid snapshot catch-up on join

**File: `src/multiplayer/spectator.rs`**

In `handle_spectate_link`, after setting game mode, also:
1. Subscribe the node to the game-specific iroh gossip topic so `GameSnapshot` broadcasts
   arrive via the normal `NetworkEvent::MessageReceived` path.
2. Send a `BraidResyncRequest { game_id, since_version: "0" }` — version `"0"` means
   "give me everything". This triggers the active player to respond with the full
   `BraidResyncResponse`, which `bridge.rs` already handles by emitting `ResyncedMove`
   events one per move.

```rust
// in handle_spectate_link, after setting game_mode:
if let Some(ref sub_tx) = network_state.subscription_sender {
    let topic = format!("/xfchess-game/{}", ev.game_id);
    let _ = sub_tx.send(topic);
}
if let Some(ref tx) = network_state.message_sender {
    let gid = ev.game_id.parse::<u64>().unwrap_or(0);
    let _ = tx.send(NetworkMessage::BraidResyncRequest {
        game_id: gid,
        since_version: "0".to_string(),
    });
}
```

Add `Res<BraidNetworkState>` to the system params.

### Step 3 — Apply ResyncedMove to the spectator board

**File: `src/multiplayer/spectator.rs`**

Add a new system `apply_braid_resync_to_spectator`:
- Reads `RollupEvent::ResyncedMove` events (already emitted by bridge.rs).
- For each, emits `NetworkMoveEvent { from, to, promotion, expected_fen: Some(next_fen) }`.
- This gives the spectator the full position fast (one move per frame) instead of
  waiting for the 2-second VPS poll.

Register in `SpectatorPlugin`.

### Step 4 — Spectator game-over

**File: `src/multiplayer/spectator.rs`**

Add system `detect_spectator_game_over`:
- When `GameMode::Spectator` and a `RollupEvent::SnapshotReceived` arrives with
  `move_payloads` matching game end (checkmate/stalemate detectable via FEN), OR
  when VPS poll returns no new moves for 30 s after a known terminal state, show
  the spectator game-over overlay (already exists in `game_over_popup.rs` as
  `spectator_game_over_overlay()`).

---

## 3. Phase 5 — Spectator in-game overlay (clock + chat)

### Step 1 — Clock display

**File: `src/multiplayer/spectator.rs`** (new resource + system)

```rust
#[derive(Resource, Default)]
pub struct SpectatorClockState {
    pub white_ms: u64,
    pub black_ms: u64,
    pub last_update_ms: u64,
}
```

Add system `update_spectator_clock`:
- Reads `BraidPvpIncomingMessage` events (or a new subscription to `/game/{id}/clock`).
- When `ChessMessage::Clock(state)` arrives, write into `SpectatorClockState`.
- The clock ticks down locally between updates using `Time` delta so it doesn't freeze
  between Braid clock broadcasts.

### Step 2 — Spectator overlay UI

**File: `src/multiplayer/ui/spectator_overlay.rs`** (new file)

```rust
pub fn spectator_hud_system(
    mut contexts: EguiContexts,
    game_mode: Res<GameMode>,
    session: Res<SpectatorSession>,
    clock: Res<SpectatorClockState>,
    mut chat_events: MessageReader<BraidChatMessage>,
    mut chat_log: Local<Vec<(String, String)>>,  // (player, text)
    time: Res<Time>,
)
```

Renders a `TopBottomPanel::bottom` when `GameMode::Spectator`:
- Left: "Spectating game {id}" label
- Centre: white clock | black clock (counting down live)
- Right: spectator count (from VPS poll response if available)

Below that or as a side panel: last N chat messages from `chat_log` (fed by
`BraidChatMessage` events).

Register in `SpectatorPlugin`.

### Step 3 — In-game chat for active players

**File: `src/ui/game/game_ui.rs`** (or a new `chat_panel.rs`)

Add a small collapsible chat panel (bottom-right corner) for `GameMode::BraidMultiplayer`:
- Reads `BraidChatMessage` events → `Local<Vec<(String, String, u64)>>` history.
- Text input + send button → fires `PublishBraidChat`.
- 8-message rolling history, auto-scrolls.

---

## Implementation order

| Step | File(s) | Complexity |
|------|---------|------------|
| 1 | `bridge.rs` — update `CachedGamePgn` from Braid PGN | Small |
| 2 | `game_over_popup.rs` — `braid_pgn_ready` flag + spinner | Small |
| 3 | `screens.rs` — wire Watch button to `SpectateViaLinkEvent` | Tiny |
| 4 | `spectator.rs` — subscribe to gossip topic + send `BraidResyncRequest` | Small |
| 5 | `spectator.rs` — `apply_braid_resync_to_spectator` system | Small |
| 6 | `spectator.rs` — `SpectatorClockState` resource + clock tick system | Small |
| 7 | `ui/spectator_overlay.rs` — HUD (clocks + "Spectating" label + chat) | Medium |
| 8 | `ui/game/chat_panel.rs` — in-game chat for BraidMultiplayer | Medium |

Steps 1–3 are the minimum viable wiring (Review works, Watch navigates).
Steps 4–6 make the spectator board fast and accurate.
Steps 7–8 add the visible spectator experience.

---

## Key invariants to preserve

- `SpectatorSession` polling (VPS, 2 s) stays as the fallback — Braid resync is the
  fast path but not reliable if the active peer is offline.
- Review/Analyze buttons must still work without Braid (offline/local games) — the
  `cached_pgn.pgn` from `MoveHistory` is the default; Braid overwrites it only if it
  arrives.
- The spectator board never accepts local input — `GameMode::Spectator` check in the
  existing input systems already guards this.
