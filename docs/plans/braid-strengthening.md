# Braid Strengthening Plan

Use the Braid versioned-document protocol for what it was designed for: spectator
catch-up, reconnection recovery, and auditable game replay.  All the infrastructure
already exists — `braid_uri`, `BraidIrohNode`, `xfchess-braid-server`, `PgnReplayState`.
This plan wires them together.

---

## Current state (what is actually happening)

```
Local move
  → NetworkMessage::Move          (iroh gossip, primary)
  → ChessPublisher::publish_move  (Braid-HTTP to VPS, fallback)
```

The iroh gossip path wraps `NetworkMessage` bytes in `Update::snapshot(random_uuid, bytes)`.
The random UUID means the Braid version chain is broken — every message looks like a new
unrelated snapshot.  `BraidIrohNode` stores these Updates in `HashMap<url, Vec<Update>>`
but nothing ever reads them back.

`BoardStateSync` in `src/game/sync/board_state.rs` has two explicit TODO comments:
> `// TODO: Write to braid-blob file or send via braid-iroh`
> `// TODO: Read from braid-blob file or receive via braid-iroh`

These are the integration points this plan completes.

---

## The five resources

Each game creates five URL-addressed Braid resources, one per concern:

| URL | Type | Content |
|-----|------|---------|
| `/game/{id}/moves` | AppendLog | `MovePayload` per ply, content-addressed version |
| `/game/{id}/clock` | PatchedDoc | `{ white_ms, black_ms, active_color, updated_at }` |
| `/game/{id}/state` | PatchedDoc | `{ status, result, reason }` — game lifecycle |
| `/game/{id}/chat`  | AppendLog | `ChatPayload` — already in `braid_uri`, not yet wired |
| `/game/{id}/offer` | PatchedDoc | `{ type: "draw"\|"takeback", from, accepted }` |

All five flow over iroh gossip via `BraidIrohNode`.  The VPS also stores them via
`xfchess-braid-server` so web spectators and post-game replay work without a live peer.

---

## Phase 1 — Content-addressed move versioning

**Problem**: versions are random UUIDs so the chain is meaningless.

**Fix**: use `braid_uri::version_hash(fen_after, move_number)` as the version,
exactly as `ChessPublisher` already does on the HTTP path.

### Changes

**`src/multiplayer/network/braid_pvp.rs` — `handle_publish_move`**

Remove the manual `NetworkMessage::Move` gossip send.  Instead call the existing
`spawn_publish_move` (which uses `ChessPublisher`) but point it at the `BraidIrohNode`
transport rather than the VPS URL.

```rust
// Before — iroh path sends a NetworkMessage with random UUID wrapper
let update = Update::snapshot(Version::new(uuid::Uuid::new_v4().to_string()), payload_bytes);
node_send.put(&topic, update).await;

// After — use content-addressed version from braid_uri
let version = braid_uri::version_hash(&event.fen_after, move_number);
let parent  = session.last_version.clone();
let update  = Update::new(version.clone(), vec![parent], body_bytes);
node_send.put(&format!("/game/{}/moves", game_id), update).await;
session.last_version = version;
```

Add `last_version: String` to `BraidPvpSession` (defaults to `"0"`).

**Why this matters**: every peer now shares a deterministic version chain.  Version N
with parent N-1 for every move.  This is the prerequisite for phases 2 and 3.

---

## Phase 2 — Spectator catch-up

**Problem**: iroh gossip does not replay past messages to late joiners.  A spectator
or reconnecting client joins and hears nothing until the next move.

**Solution**: when `IrohEvent::NeighborUp` fires on a game topic, the node that has
history replays it.

### How BraidIrohNode already stores history

`BraidIrohNode` keeps `HashMap<url, Vec<Update>>` in memory (from `node.put()`).
This is the move log.  We just need to send it.

### Changes

**`src/multiplayer/systems.rs` — `process_gossip_stream`**

In the `IrohEvent::NeighborUp` arm, check if we have stored history for the game
topic the new neighbor just joined.  If yes, broadcast a synthetic snapshot:

```rust
Ok(IrohEvent::NeighborUp(peer_id)) => {
    // existing peer discovery code ...

    // NEW: replay history to late joiner
    if let Some(history) = node.get_history(&topic_url).await {
        if !history.is_empty() {
            // Build a snapshot from the full move log
            let moves: Vec<MovePayload> = history
                .iter()
                .filter_map(|u| serde_json::from_slice(u.body.as_deref()?).ok())
                .collect();
            let snapshot_body = serde_json::to_vec(&moves).unwrap();
            let snap_version  = history.last().unwrap().version.clone();
            let snapshot = Update::snapshot(snap_version, Bytes::from(snapshot_body));
            node.put(&topic_url, snapshot).await.ok();
        }
    }
}
```

Add `BraidIrohNode::get_history(url) -> Option<Vec<Update>>` — a read-only accessor
over the existing `HashMap`.

**Spectator game mode**

Add `GameMode::Spectator` (currently absent).  On enter:
1. Subscribe to `/game/{id}/moves`, `/game/{id}/clock`, `/game/{id}/state`.
2. On first `Update` with `is_snapshot = true`: apply the full move log to reconstruct
   board state via `nimzovich_engine::game_from_fen` + sequential `do_move`.
3. Subsequent patch Updates apply one move at a time, same as the live game path.

The spectator never sends moves; read-only gossip subscriber only.

---

## Phase 3 — Reconnection recovery

**Problem**: when a player drops and reconnects, `ResyncRequest/Response` in
`rollup/bridge.rs` exchanges a single FEN + turn number.  Any moves the peer made
during the outage are lost unless the reconnector happens to have them.

**With Braid versioning**: the reconnecting client knows its last seen version
(content hash of last FEN it applied).  It can ask for everything after that point.

### Changes

**`BraidPvpSession`** — add field:
```rust
pub last_seen_version: String,   // content hash of last applied move, "0" if none
```

Updated in `drain_incoming_messages` after each applied move:
```rust
session.last_seen_version = payload.version_hash();
```

**New `NetworkMessage` variant**:
```rust
BraidResyncRequest {
    game_id: u64,
    since_version: String,   // client's last known version
},
BraidResyncResponse {
    game_id: u64,
    updates: Vec<BraidUpdate>,  // all moves after since_version, in order
},
```

**`src/multiplayer/systems.rs`** — on reconnect (existing `P2PConnectionStatus` transition
`Disconnected → Connected`), emit `BraidResyncRequest` with `last_seen_version`.

**Peer handler** (in `handle_network_to_rollup_events`):
```rust
NetworkMessage::BraidResyncRequest { game_id, since_version } => {
    if let Some(updates) = node.get_updates_since(&format!("/game/{}/moves", game_id), &since_version).await {
        send_network_msg(&network_state, NetworkMessage::BraidResyncResponse {
            game_id: *game_id,
            updates,
        });
    }
}
```

Add `BraidIrohNode::get_updates_since(url, since_version) -> Option<Vec<Update>>` —
walks `Vec<Update>` for that URL and returns everything after the matching version hash.

**Reconnecting client**:
```rust
NetworkMessage::BraidResyncResponse { game_id, updates } => {
    for update in updates {
        // apply each MovePayload in sequence via existing NetworkEvent::MessageReceived path
        let payload: MovePayload = serde_json::from_slice(&update.body)?;
        event_tx.send(NetworkEvent::MessageReceived(NetworkMessage::Move {
            game_id, turn: payload.move_number as u16,
            move_uci: payload.uci, next_fen: payload.fen_after,
            nonce: payload.move_number as u64, timestamp_ms: 0,
        })).ok();
    }
}
```

This replays missed moves through the existing game logic path — no special-case needed.

---

## Phase 4 — PGN from Braid move log

**Problem**: `PgnReplayState` is loaded from a PGN string, which today requires the
user to paste one.  After any live game, we have the full move log in
`BraidIrohNode`'s local store.

**Goal**: after `GameEndedEvent`, automatically populate `ParsedPgnGameResource` from
the Braid move log so the player can immediately enter `GameMode::PgnReplay`.

### Data path

```
BraidIrohNode
  .get_history("/game/{id}/moves")  →  Vec<Update>
  → each body: MovePayload { uci, fen_after, move_number, player }
  → apply moves sequentially with nimzovich_engine to get SAN notation
  → format as PGN string
  → nimzovich_engine::parse_pgn(pgn_string)  →  ParsedPgnGame
  → insert_resource(ParsedPgnGameResource { inner: parsed, .. })
  → transition to GameState::InGame + GameMode::PgnReplay
```

### New function: `braid_move_log_to_pgn`

Add to `src/multiplayer/network/braid_pvp.rs` or a new
`src/game/replay_braid.rs`:

```rust
pub fn braid_move_log_to_pgn(
    updates: &[Update],
    white_name: &str,
    black_name: &str,
    result: &str,        // "1-0" | "0-1" | "1/2-1/2" | "*"
) -> Option<String> {
    let mut engine = nimzovich_engine::new_game();
    let mut san_moves = Vec::new();

    for update in updates {
        let body = update.body.as_deref()?;
        let payload: MovePayload = serde_json::from_slice(body).ok()?;
        let mv = nimzovich_engine::uci_to_move(&engine, &payload.uci)?;
        let san = nimzovich_engine::move_to_san(&engine, mv);
        nimzovich_engine::apply_move(&mut engine, mv);
        san_moves.push(san);
    }

    Some(format!(
        "[White \"{}\"]\n[Black \"{}\"]\n[Result \"{}\"]\n\n{} {}",
        white_name, black_name, result,
        san_moves.iter().enumerate().map(|(i, san)| {
            if i % 2 == 0 { format!("{}. {}", i / 2 + 1, san) }
            else           { san.clone() }
        }).collect::<Vec<_>>().join(" "),
        result
    ))
}
```

### Bevy system: `handle_game_end_pgn_export`

Add to `src/multiplayer/rollup/bridge.rs` alongside `handle_game_end_undelegation`:

```rust
fn handle_game_end_pgn_export(
    mut game_ended: MessageReader<GameEndedEvent>,
    braid_pvp: Res<BraidPvpSession>,
    network_state: Res<BraidNetworkState>,
    mut commands: Commands,
) {
    for event in game_ended.read() {
        let game_id_str = braid_pvp.game_id.clone();
        let result_str = match event.winner.as_deref() {
            Some("white") => "1-0",
            Some("black") => "0-1",
            _             => "1/2-1/2",
        };

        // Retrieve move log from node history via oneshot channel
        // (same pattern as delegation/finalization tasks)
        // → braid_move_log_to_pgn → insert ParsedPgnGameResource
    }
}
```

The async retrieval uses the existing `oneshot::channel` pattern from `bridge.rs`.

---

## Phase 5 — Additional resources that strengthen the connection

These extend Braid beyond moves to give every game a richer shared document set.

### `/game/{id}/clock`

Both players publish their remaining time after every move:
```json
{ "white_ms": 182000, "black_ms": 195000, "active_color": "black", "updated_at": 1718123456 }
```

Patch type: `PatchedDoc`.  Spectators see live clocks.  On reconnect, the clock
snapshot is part of the initial state burst.

Wire into: `src/ui/game/game_ui.rs` clock display — read from `BraidPvpSession`
clock resource instead of a local timer when in multiplayer mode.

### `/game/{id}/state`

Game lifecycle document:
```json
{ "status": "active", "result": null, "reason": null }
```

Patched to `{ "status": "ended", "result": "white", "reason": "checkmate" }` on
`GameEndedEvent`.

A reconnecting or spectating client subscribing to this resource knows immediately
whether the game is still live or already finished, without waiting for the next move.

### `/game/{id}/offer`

Draw and takeback negotiation as a shared mutable document:
```json
{ "type": null, "from": null, "accepted": null, "seq": 0 }
```

One player patches it to `{ "type": "draw", "from": "white", "seq": 1 }`.  The other
either patches `accepted: true` or resets it.  This eliminates the current ad-hoc
`NetworkMessage` variants for offers and gives both sides a consistent view.

### `/game/{id}/chat`

`braid_uri::ChatPayload` and `ChessSubscriber::subscribe_chat` already exist.
`BraidPvpSession` already has a `chat_rx` channel.  The only missing piece is wiring
`chat_rx` into the in-game chat UI in `src/ui/game/game_ui.rs`.

---

## Wire the BoardStateSync TODOs

`src/game/sync/board_state.rs` has two placeholder systems:

```rust
pub fn broadcast_state_system(...)  // TODO: send via braid-iroh
pub fn receive_state_system(...)    // TODO: receive via braid-iroh
```

After Phase 1 these become:

```rust
// broadcast_state_system
if board_sync.sync_status == SyncStatus::PendingLocal {
    publish_events.write(PublishBraidMove { uci, fen_after, player });
    board_sync.on_sync_complete(serialized_state);
}

// receive_state_system  
while let Ok(msg) = session.rx.try_recv() {
    if let ChessMessage::Move(payload) = msg {
        // apply via existing NetworkEvent path
    }
}
```

This replaces the TODO stubs with real calls into the already-wired `BraidPvpPlugin`
event system.

---

## File change summary

| File | Change |
|------|--------|
| `crates/networking/braid-iroh/src/node.rs` | Add `get_history(url)`, `get_updates_since(url, version)` |
| `src/multiplayer/network/braid_pvp.rs` | Content-addressed versions; wire clock/state/offer publish |
| `src/multiplayer/types.rs` | Add `BraidResyncRequest`, `BraidResyncResponse` to `NetworkMessage` |
| `src/multiplayer/systems.rs` | `NeighborUp` → replay history; handle new resync variants |
| `src/multiplayer/rollup/bridge.rs` | Add `handle_game_end_pgn_export` system |
| `src/game/sync/board_state.rs` | Implement the two TODO stubs |
| `src/game/replay_braid.rs` | New file — `braid_move_log_to_pgn` |
| `src/ui/game/game_ui.rs` | Clock from Braid clock resource; chat from `chat_rx` |
| `src/core/states.rs` | Add `GameMode::Spectator` |

---

## Implementation order

1. **Phase 1** (content-addressed versions) — prerequisite for everything else.
   Small change, high leverage.
2. **Phase 3** (reconnection) — builds directly on Phase 1, no new UI needed.
3. **Phase 4** (PGN export) — self-contained, uses existing replay system.
4. **Phase 2** (spectators) — needs `GameMode::Spectator` and UI work.
5. **Phase 5** (clock / state / offer / chat) — add one resource at a time.
