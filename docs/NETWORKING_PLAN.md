# XFChess Networking — Full Implementation Plan

> Audit-driven. Every item maps to a confirmed gap in the codebase.  
> Phase order follows dependency: each phase builds on the previous.

---

## Current State (as of audit)

| Layer | Status | Blocker |
|-------|--------|---------|
| Braid-HTTP transport | Working | Tokio panic on publish (fixed) |
| Iroh gossip fast-path | Working | DNS failures in dev (cosmetic) |
| P2P connection state machine | Working | Wager handshake never fires |
| Move protocol / signing | Defined | Nonces tracked but never validated |
| Lobby browser (host+join) | Working | Spectator button logs only |
| Game-over state | Working | No settlement trigger |
| Draw offer events | Defined | Zero handlers |
| Reconnect / resync | Defined | Zero handlers |
| Batch / rollup settlement | Defined | Zero handlers |
| Matchmaking algorithm | Stub | FIFO only, no ELO pairing |

---

## Phase 1 — Reliable Game Session (E2E playable)

> Goal: two players can connect, play a full game, and see a correct result screen.  
> No wagers, no spectators, no draws yet.

### 1.1 Lobby: game code display + join by code

**Files:** `src/states/main_menu/screens.rs`, `src/multiplayer/network/p2p.rs`

- Host config screen already calls `p2p_announce_game()` → backend returns a game ID.  
  Currently the game ID is shown but not copyable.  
  **Task:** render the game ID as a selectable text field with a "Copy" button next to it.

- Add a "Join by code" text input on the lobby selector popup (alongside the existing lobby browser).  
  On submit: call `p2p_join_game(code)`, skip the lobby-browser row entirely.

- `P2PUIState` already has `lobby_name: String` — wire this to the host-side `p2p_announce_game` call so the room name appears in the lobby browser list (currently hardcoded).

```
src/states/main_menu/screens.rs
  render_host_p2p_config_screen()  ← add lobby_name field, pass to p2p_announce_game
  render_lobby_selection_popup()   ← add "Join by code" input + submit
  render_p2p_waiting_screen()      ← render game ID as copyable field
```

### 1.2 Correct player-color assignment visible in UI

**Files:** `src/multiplayer/network/p2p.rs`, `src/states/main_menu/screens.rs`

- `P2PConnectionState.player_color` is set (White=host, Black=joiner) during `handle_network_events`.  
  **Task:** display "You are playing as White/Black" on the waiting screen and on the game-start transition.

- `initialize_players` in `src/game/systems/game_init.rs` correctly reads `P2PConnectionState.player_color` for `BraidMultiplayer` mode — verify this path is exercised in the `P2PWaiting → InGame` transition.

### 1.3 Heartbeat timeout: detect disconnected opponent

**Files:** `src/multiplayer/network/p2p.rs`, `src/multiplayer/types.rs`

- `P2PConnectionState` has `connecting_since: Option<Instant>`.  
  `tick_connection_timeout` runs but only covers the *connecting* phase (12s).  
  **Task:** add a separate `last_opponent_heartbeat: Option<Instant>` field and a system that fires if no move or keepalive arrives for 30s during `InGame`.

```rust
// src/multiplayer/types.rs — add to P2PConnectionState
pub last_opponent_activity: Option<Instant>,
pub heartbeat_interval: f32,   // seconds between sent keepalives
pub heartbeat_timer: f32,

// New NetworkMessage variant in protocol.rs
Keepalive { game_id: u64, nonce: u64 },
```

- New system `tick_game_heartbeat`:
  - Sends `Keepalive` every 10s during `InGame`.
  - If `last_opponent_activity` > 30s ago → set `GameOverState` to `OpponentDisconnected` (new variant) and show UI.

### 1.4 FEN desync check on every received move

**Files:** `src/multiplayer/network/relay.rs` (or `systems.rs`), `src/engine/board_state.rs`

- `dispatch_remote_moves` receives `NetworkMessage::Move { next_fen, ... }` and applies the move.  
  After applying, compute the local FEN and compare.

```rust
// After applying the remote move:
let local_fen = engine.current_fen().to_string();
if local_fen != msg.next_fen {
    warn!("[NET] FEN desync: local={} remote={}", local_fen, msg.next_fen);
    // Write ResyncRequest to channel
    network_state.message_sender.send(NetworkMessage::ResyncRequest {
        game_id, last_confirmed_fen: local_fen,
    });
}
```

- Handle `ResyncRequest` on the sender side: re-emit the authoritative FEN as `ResyncResponse`.  
  On receive: overwrite local engine state with the received FEN.

```rust
// dispatch_remote_moves — add arm:
NetworkMessage::ResyncResponse { game_id, fen } => {
    engine.load_fen(&fen);
    info!("[NET] Resynced to FEN: {}", fen);
}
```

### 1.5 Nonce deduplication on inbound moves

**Files:** `src/multiplayer/network/braid_pvp.rs`, `src/multiplayer/types.rs`

- `BraidNetworkState.expected_nonces: HashMap<u64, u64>` already exists.
- `drain_incoming_messages` currently sets `nonce: 0` for all Braid-HTTP moves.
- **Task:** parse the nonce from `ChessMessage` (it's in `MovePayload`) and check it.

```rust
// drain_incoming_messages — replace nonce: 0 with:
let game_id_u64 = session.game_id.parse::<u64>().unwrap_or(0);
let expected = network_state.expected_nonces.entry(game_id_u64).or_insert(1);
if msg_nonce < *expected {
    warn!("[NET] Replayed move nonce {} (expected ≥{})", msg_nonce, expected);
    continue;
}
*expected = msg_nonce + 1;
```

### 1.6 End-of-game evaluation screen

**Files:** `src/game/plugin.rs`, `src/states/` (new screen), `src/game/events.rs`

- `GameOverState` has all result variants. There is currently no dedicated result screen.  
  **Task:** add a `GameState::PostGame` state and a `PostGameScreen` plugin.

```
src/states/post_game.rs   (new)
  - render_post_game_screen(): show winner, material balance, move count,
    time used per player, rematch button, main-menu button
  - transition triggered when GameOverState != Playing && GameState == InGame
```

- In `src/game/plugin.rs`, add:
```rust
OnEnter(GameState::PostGame): setup_post_game_screen
OnExit(GameState::PostGame):  teardown_post_game_screen
```

- `GameEndedEvent` already fires in the Solana path — fire it unconditionally on game over in all modes.

---

## Phase 2 — Game Controls (pause, forfeit, draw)

### 2.1 Pause and resume

**Note:** True pause is only meaningful in local/AI modes; in P2P the clock is each player's problem.

- **Local/AI:** Add `GameState::Paused`. ESC already has `InGameExitConfirmation`.  
  Extend it: first ESC press pauses (freezes timers), second or "Resume" button unpauses.

```rust
// src/core/states.rs
pub enum GameState { Splash, MainMenu, InGame, Paused, PostGame }

// src/game/systems/input.rs  handle_escape_key
if keyboard.just_pressed(KeyCode::Escape) {
    if *state == GameState::InGame {
        next_state.set(GameState::Paused);
    } else if *state == GameState::Paused {
        next_state.set(GameState::InGame);
    }
}
```

- **P2P:** Pause sends a `Keepalive`-class message; opponent's clock keeps running.  
  Show "opponent paused" banner but keep clock ticking. No hard pause in P2P.

### 2.2 Forfeit (in-game resign)

- `ResignEvent` already exists and `PublishBraidResign` publishes it over the network.  
  `handle_publish_resign` is fixed (Tokio panic resolved in this session).

- **Task:** wire the in-game exit confirmation popup to actually fire `ResignEvent` when confirmed.

```rust
// src/game/systems/input.rs  confirm_exit_game
resign_events.write(ResignEvent {
    winner: opponent_color_str,
    remote: false,
});
```

- On the remote side, `drain_incoming_messages` converts `ChessMessage::Resign` to `NetworkMessage::Resign`.  
  `dispatch_remote_moves` should then set `GameOverState` to the correct resignation variant.

```rust
// dispatch_remote_moves — add arm:
NetworkMessage::Resign { winner, .. } => {
    *game_over = if winner == "white" {
        GameOverState::WhiteWonByResignation
    } else {
        GameOverState::BlackWonByResignation
    };
}
```

### 2.3 Draw offer, accept, decline

**Files:** `src/game/events.rs`, `src/multiplayer/network/protocol.rs`, `src/game/systems/input.rs`

- `DrawOfferEvent` and `DrawResponseEvent` already defined but have zero handlers.

- Add to `NetworkMessage`:
```rust
DrawOffer { game_id: u64 },
DrawResponse { game_id: u64, accepted: bool },
```

- In-game HUD: show "Offer Draw" button. On click, fire `DrawOfferEvent { remote: false }`.  
  System listens, sends `NetworkMessage::DrawOffer` via message_sender.

- Remote side: on `DrawOffer` receive, show banner "Opponent offers a draw — Accept / Decline".  
  On accept: both sides set `GameOverState::Stalemate` (reuse) or add `AgreedDraw` variant.

```rust
// New GameOverState variant
AgreedDraw,
```

```
src/game/systems/draw.rs   (new)
  handle_draw_offer_sent()
  handle_draw_offer_received()   ← shows UI banner
  handle_draw_response()         ← sets GameOverState on acceptance
```

---

## Phase 3 — Connection Resilience (reconnect, resync, clock sync)

### 3.1 Reconnect: rejoin in-progress game by code

**Files:** `src/multiplayer/network/braid_pvp.rs`, `src/states/main_menu/screens.rs`

The game ID (used as Braid topic) is stable as long as the backend holds it. Reconnect is:
1. Re-subscribe to `GAME_TOPIC/{game_id}`.
2. Request `ResyncResponse` with full FEN + move history.
3. Restore clock state from the response.

```rust
// src/multiplayer/network/braid_pvp.rs — add system
pub fn handle_reconnect(
    mut session: ResMut<BraidPvpSession>,
    mut network_state: ResMut<BraidNetworkState>,
    tokio_runtime: Res<TokioRuntime>,
) {
    // If session.active == false but we have a stored game_id,
    // re-run start_session with that game_id
}
```

- Store `last_game_id: Option<String>` in a persistent resource (not reset on exit) so the
  user can reconnect even after the menu.

- On the waiting/reconnect screen: show "Reconnect to last game" if `last_game_id` is set.

### 3.2 Resync protocol (full implementation)

**Files:** `src/multiplayer/network/protocol.rs`, `src/multiplayer/systems.rs`

- `ResyncRequest` and `ResyncResponse` are already in `NetworkMessage`.  
  Neither has a handler. **Implement both sides:**

```rust
// Sender side (host or whoever has canonical state):
NetworkMessage::ResyncRequest { game_id, last_confirmed_fen } => {
    let current_fen = engine.current_fen().to_string();
    let history = move_history.to_uci_list();
    network_state.message_sender.send(NetworkMessage::ResyncResponse {
        game_id,
        fen: current_fen,
        move_history: history,
        white_time_left: game_timer.white_time_left,
        black_time_left: game_timer.black_time_left,
    });
}

// Receiver side:
NetworkMessage::ResyncResponse { fen, move_history, white_time_left, black_time_left, .. } => {
    engine.load_fen(&fen);
    game_timer.white_time_left = white_time_left;
    game_timer.black_time_left = black_time_left;
    // optionally replay move_history to rebuild capture state
}
```

- Extend `NetworkMessage::ResyncResponse` to carry clock values:
```rust
ResyncResponse {
    game_id: u64,
    fen: String,
    move_history: Vec<String>,
    white_time_left: f32,
    black_time_left: f32,
}
```

### 3.3 Clock synchronisation

**Files:** `src/game/resources/active_time_control.rs`, `src/multiplayer/network/protocol.rs`

- Problem: each client runs an independent clock. After a reconnect (or even a slow network move)
  the clocks can drift.

- Solution: include `white_time_left` and `black_time_left` in every `NetworkMessage::Move`.

```rust
// protocol.rs — extend Move variant
Move {
    game_id: u64,
    turn: u16,
    move_uci: String,
    next_fen: String,
    nonce: u64,
    white_ms_left: u32,   // milliseconds remaining
    black_ms_left: u32,
},
```

- `dispatch_remote_moves`: after applying move, clamp local clock to the received value
  if the difference is > 500ms (small drift tolerance).

```rust
let clock_delta_ms = (game_timer.white_time_left * 1000.0) as i32
    - msg.white_ms_left as i32;
if clock_delta_ms.abs() > 500 {
    game_timer.white_time_left = msg.white_ms_left as f32 / 1000.0;
    game_timer.black_time_left = msg.black_ms_left as f32 / 1000.0;
}
```

---

## Phase 4 — Spectators & Observers

### 4.1 Spectator subscription

**Files:** `src/states/main_menu/screens.rs`, `src/multiplayer/network/braid_pvp.rs`

The "Watch" button in `render_spectator_popup` logs only. Full path:

1. On "Watch": start a `BraidPvpSession` in **read-only mode** (no `message_sender`).
2. Subscribe to `GAME_TOPIC/{game_id}` via `ChessSubscriber`.
3. Transition to `GameState::InGame` with `GameMode::Spectator`.

```rust
// src/multiplayer/network/braid_pvp.rs — add spectator_start_session()
pub fn spectator_start_session(
    session: &mut BraidPvpSession,
    game_id: String,
    base_url: String,
    tokio_runtime: &TokioRuntime,
) {
    // same subscribe path as start_session but no publisher side
}
```

- `GameMode::Spectator` is already defined in `src/core/states.rs`.  
  `is_human_turn()` in `input.rs` already returns `false` for spectators.
- `initialize_players` needs a `Spectator` arm that sets both players as non-human.

### 4.2 Spectator live catch-up

- On join: send `ResyncRequest` immediately. Apply the response FEN before entering `InGame`.  
  All subsequent moves arrive via the subscriber and are applied as remote moves.

---

## Phase 5 — Move Signing & Replay Protection (security hardening)

### 5.1 Sign outgoing moves with session_signing_key

**Files:** `src/multiplayer/network/braid_pvp.rs`, `src/multiplayer/types.rs`, `src/multiplayer/network/protocol.rs`

- `BraidNetworkState.session_signing_key: Option<[u8; 32]>` is set but never used for signing.
- `SignedNetworkMessage::sign()` already works and has tests.
- **Task:** wrap every outgoing `NetworkMessage::Move` in `SignedNetworkMessage` before
  sending through `message_sender`.

```rust
// handle_publish_move — before tx.send(iroh_msg):
let signed = if let Some(key_bytes) = &network_state.session_signing_key {
    let key = SigningKey::from_bytes(key_bytes);
    match SignedNetworkMessage::sign(&iroh_msg, &key) {
        Ok(s) => Some(s),
        Err(e) => { warn!("Sign failed: {e}"); None }
    }
} else { None };

// send signed wrapper if available, plain message otherwise
```

- On receive in `handle_network_events`: if message is `SignedNetworkMessage`, verify before
  dispatching. Unknown session keys → reject.

### 5.2 Enforce nonce ordering (complete the existing skeleton)

```
src/multiplayer/network/braid_pvp.rs  drain_incoming_messages()
  + parse nonce from incoming ChessMessage
  + check against BraidNetworkState.expected_nonces[game_id]
  + reject and log if nonce < expected
  + increment expected on acceptance
```

---

## Phase 6 — Rematch Flow

### 6.1 Post-game rematch offer

**Files:** `src/states/post_game.rs` (new), `src/multiplayer/network/protocol.rs`

Add to `NetworkMessage`:
```rust
RematchOffer { game_id: u64 },
RematchResponse { game_id: u64, accepted: bool },
```

Post-game screen (Phase 1.6) has a "Rematch" button:
1. Click → send `RematchOffer`, show "Waiting for opponent…"
2. Opponent sees "Opponent wants a rematch — Accept / Decline"
3. Both accept → reset game state, swap colors, transition back to `InGame`

```rust
// src/game/systems/rematch.rs  (new)
pub fn handle_rematch_response(
    mut events: MessageReader<RematchAccepted>,
    mut game_state: ResMut<NextState<GameState>>,
    mut game_over: ResMut<GameOverState>,
    mut players: ResMut<Players>,
    mut current_turn: ResMut<CurrentTurn>,
) {
    *game_over = GameOverState::Playing;
    // swap colors
    std::mem::swap(&mut players.player_1.color, &mut players.player_2.color);
    current_turn.color = PieceColor::White;
    game_state.set(GameState::InGame);
}
```

---

## Phase 7 — Matchmaking (ELO pairing + host/player preference)

**Files:**
- `backend/src/signing/routes/matchmaking/handlers.rs` — queue logic
- `backend/src/signing/routes/matchmaking/state.rs` — `MatchmakingTicket`
- `src/multiplayer/network/p2p_vps.rs` — `VpsGameListing`
- `src/states/main_menu/screens.rs` — host config UI + lobby browser UI

### 7.1 Design overview

Both the **host** (when opening a room) and the **joining player** (when browsing or queuing) get explicit preference controls. A match is only made when both parties' requirements are mutually satisfied.

```
Host preferences                Joiner preferences
──────────────────              ──────────────────
Casual / Rated toggle           Casual / Rated filter
Open / ELO-restricted toggle    Preferred ELO range (±N)
  └─ min_elo / max_elo slider
```

### 7.2 Data structure changes

#### Backend — `MatchmakingTicket` (state.rs)

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchmakingTicket {
    pub pubkey:    String,
    pub elo:       u32,
    pub joined_at: DateTime<Utc>,
    // NEW
    pub game_type:    GameType,        // Casual | Rated
    pub elo_min:      Option<u32>,     // joiner's acceptable floor (None = no preference)
    pub elo_max:      Option<u32>,     // joiner's acceptable ceiling (None = no preference)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GameType { Casual, Rated }
```

#### Backend — `JoinRequest` (handlers.rs)

```rust
#[derive(Debug, Deserialize)]
pub struct JoinRequest {
    pub pubkey:    String,
    pub signature: String,
    pub timestamp: i64,
    // NEW
    pub game_type: GameType,
    pub elo_min:   Option<u32>,
    pub elo_max:   Option<u32>,
}
```

#### Backend — host config when announcing a room (new `HostPreferences`)

Add to the game-listing endpoint payload:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostPreferences {
    pub game_type:    GameType,
    pub elo_open:     bool,     // false = restricted to [elo_min, elo_max]
    pub elo_min:      Option<u32>,
    pub elo_max:      Option<u32>,
}
```

#### Client — `VpsGameListing` (p2p_vps.rs)

```rust
pub struct VpsGameListing {
    // existing fields …
    pub game_type:    String,   // already present
    // NEW
    pub elo_min:      Option<u32>,
    pub elo_max:      Option<u32>,
    pub is_rated:     bool,
}
```

### 7.3 Matching algorithm

Replace the FIFO push with a two-sided compatibility check:

```rust
// POST /matchmaking/join
fn tickets_compatible(host: &MatchmakingTicket, joiner: &MatchmakingTicket) -> bool {
    // 1. Game type must agree
    if host.game_type != joiner.game_type {
        return false;
    }

    let elapsed_host   = Utc::now().signed_duration_since(host.joined_at).num_seconds() as u32;
    let elapsed_joiner = Utc::now().signed_duration_since(joiner.joined_at).num_seconds() as u32;
    // Base ±150 window, expands +50 every 30 s each side spends waiting
    let base   = 150_u32;
    let expand = 50_u32;
    let window = base
        + (elapsed_host   / 30) * expand
        + (elapsed_joiner / 30) * expand;

    let diff = (host.elo as i32 - joiner.elo as i32).unsigned_abs();

    // 2. ELO difference must be within the expanding window
    if diff > window {
        return false;
    }

    // 3. If either side set hard ELO bounds, both must satisfy them
    if let Some(min) = host.elo_min {
        if joiner.elo < min { return false; }
    }
    if let Some(max) = host.elo_max {
        if joiner.elo > max { return false; }
    }
    if let Some(min) = joiner.elo_min {
        if host.elo < min { return false; }
    }
    if let Some(max) = joiner.elo_max {
        if host.elo > max { return false; }
    }

    true
}

// In the handler:
let candidate = queue.iter().find(|t| tickets_compatible(t, &ticket));
if let Some(opponent) = candidate {
    // create match …
} else {
    queue.push(ticket);
}
```

- Queue timeout: entry older than 5 minutes is auto-removed.
- Add `GET /matchmaking/queue/size?game_type=rated` for lobby browser "N players searching".

### 7.4 Host UI changes (screens.rs — host config screen)

Add to `render_host_p2p_config_screen()`:

```
┌─ Game type ────────────────────────────┐
│  ○ Casual  (no ELO change)             │
│  ● Rated   (ELO tracked)               │
└────────────────────────────────────────┘

┌─ ELO restriction ──────────────────────┐
│  ○ Open to all                         │
│  ● Restrict range                      │
│    Min ELO: [1200 ▲▼]  Max ELO: [1600 ▲▼] │
└────────────────────────────────────────┘
```

- Default: Rated, Open to all.
- When "Restrict range" is selected, show the min/max number inputs; seed them
  with `host_elo - 200` / `host_elo + 200`.
- Pass these values into the `p2p_announce_game` call so they appear in the
  `VpsGameListing` visible to browsers, and in the `HostPreferences` sent to the
  matchmaking endpoint.

### 7.5 Joiner UI changes (screens.rs — lobby browser + matchmaking queue entry)

**Lobby browser filter panel** (add to `render_lobby_browser()`):

```
┌─ Filter ───────────────────────────────┐
│  Game type:  [All ▼] / [Casual] [Rated] │
│  ELO range:  [──●────────] 900 – 1400  │
└────────────────────────────────────────┘
```

- Filter is client-side: re-render the listing rows that pass `game.is_rated == filter_rated`
  and `game.elo` is within the slider range.
- Show host ELO alongside the room name so the joiner can assess before clicking.
- Show a lock icon on rows where the host has set an ELO restriction; if the local
  player's ELO is outside that range, dim the row and show "ELO restricted" tooltip.

**Matchmaking queue entry** (add to `render_matchmaking_screen()`):

```
┌─ Matchmaking preferences ──────────────┐
│  ○ Casual   ● Rated                    │
│  Accept opponents:                     │
│    ○ Any ELO                           │
│    ● Within ±[200 ▲▼] of my ELO        │
└────────────────────────────────────────┘
```

- Translate "Within ±N" into `elo_min = my_elo - N`, `elo_max = my_elo + N` in the
  `JoinRequest`.
- Show a status line: "Searching … (±200 ELO, Rated) — 1 m 15 s elapsed".

---

## Phase 8 — Gamemode Matrix

Each mode needs a host and a join path. Status per mode:

| Mode | Host | Join | Notes |
|------|------|------|-------|
| Local PvP | ✓ | N/A | same machine |
| vs AI | ✓ | N/A | no network |
| Braid P2P (free) | ✓ | ✓ | working after Phase 1 |
| Braid P2P (wagered) | ✓ | partial | wager handshake missing |
| Solana wager lobby | ✓ | ✓ | on-chain settlement TBD |
| Tournament | partial | ✗ | Swiss pairing exists, relay missing |
| Spectator | ✗ | ✗ | Phase 4 |
| Matchmaking | partial | partial | Phase 7 |

### Tournament game relay (missing)
- `braid_pvp.rs` mentions tournament relay in doc comment but nothing is registered.
- Tournament match data lives in `TournamentLobbyState`.
- **Task:** when a tournament round starts, auto-configure a `BraidPvpSession` using the
  tournament's game ID and both players' node IDs, then notify both clients to connect.

---

## Implementation Order (recommended sprint sequence)

```
Sprint 1 (playable baseline):
  1.1 game code + lobby name
  1.2 color assignment visible
  1.4 FEN desync check
  1.5 nonce dedup
  1.6 post-game screen
  2.2 forfeit wired to ResignEvent

Sprint 2 (controls + safety):
  1.3 heartbeat timeout
  2.1 pause/resume
  2.3 draw offer
  3.3 clock sync in Move message

Sprint 3 (resilience):
  3.1 reconnect
  3.2 resync protocol
  5.2 nonce ordering enforced

Sprint 4 (spectators + rematch):
  4.1 spectator subscription
  4.2 spectator catch-up
  6.1 rematch flow

Sprint 5 (security):
  5.1 move signing

Sprint 6 (matchmaking + tournaments):
  7 ELO matchmaking
  8 tournament relay
```

---

## Key Files Quick Reference

| Task | Primary file(s) |
|------|----------------|
| Lobby / game code UI | `src/states/main_menu/screens.rs` |
| P2P connection state | `src/multiplayer/network/p2p.rs` |
| Move publish / subscribe | `src/multiplayer/network/braid_pvp.rs` |
| Network message types | `src/multiplayer/network/protocol.rs` |
| Remote move dispatch | `src/multiplayer/systems.rs` |
| Player init / colors | `src/game/systems/game_init.rs` |
| Game-over state | `src/game/resources/history/game_over.rs` |
| Game events | `src/game/events.rs` |
| Clock / timers | `src/game/resources/active_time_control.rs` |
| Post-game screen (new) | `src/states/post_game.rs` |
| Draw / rematch (new) | `src/game/systems/draw.rs`, `rematch.rs` |
| Matchmaking algorithm | `backend/src/signing/routes/matchmaking/handlers.rs` |
| Move signing | `src/multiplayer/types.rs` + `protocol.rs` |
