# Braid Proxy: What It Gives Us + Redundancy Removal Plan

## What the codebase actually does today

Two independent paths carry every move in a `BraidMultiplayer` game:

```
Local move made
  │
  ├── PATH A — iroh gossip (fast, ~10–30 ms P2P)
  │     systems.rs: msg_tx.send(NetworkMessage::Move)
  │     → node.put("/xfchess-game/{id}", Update::snapshot(payload))
  │     → iroh_gossip broadcasts bytes to peer
  │     → peer's process_gossip_stream decodes → NetworkEvent::MessageReceived
  │
  └── PATH B — Braid-HTTP to backend (slow, ~80–200 ms)
        braid_pvp.rs: spawn_publish_move()
        → ChessPublisher::publish_move() → HTTP PUT to backend
        → ChessSubscriber::subscribe_moves() long-polls backend
        → session.rx channel → drain_incoming_messages
        → NetworkEvent::MessageReceived (duplicate of PATH A)
```

Both paths feed the **same** `NetworkEvent::MessageReceived` handler. For every move
between two online desktop peers, PATH B is always slower and always delivers a duplicate.

---

## The `proxy` feature: what it actually does

`braid-iroh` has a compiled-out feature gate at `crates/networking/braid-iroh/Cargo.toml`.
When enabled it does two things:

### 1. Mounts a Braid-HTTP/3 server on the iroh endpoint

`protocol.rs` builds an Axum router with `GET /:resource` and `PUT /:resource` routes
and wraps it in `IrohAxum` (iroh's HTTP/3 adapter). This router gets mounted on the iroh
node with `BRAID_H3_ALPN`:

```rust
Router::builder(endpoint.clone())
    .accept(BRAID_H3_ALPN.to_vec(), braid_handler)   // ← Braid/H3 on the iroh node itself
    .accept(GOSSIP_ALPN.to_vec(), gossip.clone())
    .spawn()
```

Any peer that knows your iroh node ID can now do:

```
GET  https://<node_id>/xfchess-game/42/moves
PUT  https://<node_id>/xfchess-game/42/clock
```

over iroh QUIC directly — no backend involved.

### 2. Starts a local TCP proxy bridge

`proxy.rs` binds a local TCP port (e.g. `127.0.0.1:8181`) and translates
incoming HTTP/1.1 requests to HTTP/3 over iroh to a named peer. This is what
lets a browser or `curl` talk to an iroh node without QUIC support:

```
Browser → HTTP/1.1 → localhost:8181 → IrohH3Client → HTTP/3 → iroh node
```

The `Subscribe: true` header on a GET returns status 209 (Braid live stream),
so the browser gets a streaming update feed — exactly like a server-sent event.

### What you get when proxy is on

| Capability | Without proxy | With proxy |
|---|---|---|
| Desktop peer subscribes to moves | iroh gossip ✓ | iroh gossip ✓ (unchanged) |
| Web browser watches moves live | impossible (no QUIC) | `GET localhost:8181/xfchess-game/{id}/moves` → 209 stream ✓ |
| Clock/chat without backend | no (backend-only) | `PUT node/clock` stored on iroh node, streamed to subscribers ✓ |
| Spectator catch-up on join | VPS poll (2 s) | instant from iroh node's local resource store ✓ |
| Backend dependency for live play | yes (Braid-HTTP PUT) | no (removed, see plan below) |

The iroh node's `resources: HashMap<String, Vec<Update>>` becomes the
authoritative in-memory log for the duration of the game. `get_updates_since`
is what serves spectator catch-up.

---

## Redundancy removal plan

### What to keep

| Usage | Keep? | Why |
|---|---|---|
| iroh gossip for moves (PATH A) | **Yes** | Fast, signed, authenticated, nonce-protected |
| `ChessPublisher::publish_clock()` | **Migrate** | Replace with `node.put()` on iroh node |
| `ChessSubscriber::subscribe_clock()` | **Migrate** | Replace with gossip/proxy subscription |
| `ChessSubscriber::subscribe_chat()` | **Migrate** | Replace with `NetworkMessage::Chat` over gossip |
| Backend REST (`record_move`, VPS) | **Yes** | Canonical on-chain record, rollup settlement |
| `braid_uri` in `replay_braid.rs` | **Yes** | `MovePayload` type, PGN export, game-end only |

### What to remove

| File | What to cut |
|---|---|
| `braid_pvp.rs` | `spawn_publish_move()` async task — HTTP PUT per move |
| `braid_pvp.rs` | `handle_publish_move()` Bevy system |
| `braid_pvp.rs` | `PublishBraidMove` event (replaced by direct gossip via `msg_tx`) |
| `braid_pvp.rs` | moves subscriber task (the `subscribe_moves().await` block in `start_session`) |
| `braid_pvp.rs` | `publish_local_moves_via_braid()` system (replaced by gossip) |
| `braid_pvp.rs` | `request_braid_resync_on_reconnect()` (keep `BraidResyncRequest` gossip path instead) |
| `braid_pvp.rs` | `BraidPvpSession::sender` / `rx` channels (moves no longer go through session) |
| `braid_pvp.rs` | `drain_incoming_messages()` (moves come via gossip `NetworkEvent::MessageReceived`) |
| `braid_pvp.rs` | `spawn_publish_resign()` (resign already sent as `NetworkMessage::Resign` via gossip) |
| `braid_pvp.rs` | `handle_publish_resign()` system (same) |
| `BraidPvpSession` | `rx`, `sender`, `last_version`, `last_seen_version` fields |

### What to migrate (keep functionality, change transport)

**Clock** — currently: `ChessPublisher::publish_clock()` HTTP PUT → `ChessSubscriber::subscribe_clock()`  
Replace with: `node.put("/xfchess-game/{id}/clock", Update::snapshot(...))` via the existing
`msg_tx` channel, and subscribe via a dynamic gossip topic (`sub_tx.send("/xfchess-game/{id}/clock")`).

**Chat** — currently: `ChessPublisher::publish_chat()` HTTP PUT → `ChessSubscriber::subscribe_chat()`  
Replace with: add `NetworkMessage::Chat { game_id, player, text, timestamp_ms }` variant and route
it through the existing iroh gossip path. `drain_chat_messages` stays but reads from gossip events
instead of `session.chat_rx`.

---

## Step-by-step execution

### Step 1 — Enable proxy feature

**File: `crates/networking/braid-iroh/Cargo.toml`**

Check what the proxy feature currently gates, then add it to the game client's
dependency declaration:

```toml
# Cargo.toml (workspace)
braid-iroh = { path = "crates/networking/braid-iroh", features = ["proxy"] }
```

**File: `src/multiplayer/systems.rs`**

Replace the `proxy_config: None` in `initialize_braid_network`:

```rust
let proxy_addr: std::net::SocketAddr = "127.0.0.1:8181".parse().unwrap();
// Use the opponent's node ID as default peer once known; for now leave as
// a placeholder — the proxy is useful even without a default peer set.
let config = BraidIrohConfig {
    secret_key: Some(secret_key),
    discovery: DiscoveryConfig::Real,
    proxy_config: Some(braid_iroh::ProxyConfig {
        listen_addr: proxy_addr,
        default_peer: node_id,  // self — will be overridden per-game
    }),
};
```

Expose the proxy port via a new `BraidNetworkState` field so the UI can
display the spectate URL (`http://localhost:8181/xfchess-game/{id}/moves`).

### Step 2 — Remove move PATH B from `braid_pvp.rs`

Delete the entire moves subscriber block (currently lines ~207-234):
```rust
// ── Moves subscriber ─── DELETE THIS ENTIRE BLOCK
bevy::tasks::IoTaskPool::get().spawn(async move {
    let sub = ChessSubscriber::new(...);
    let (rx, _handle) = sub.subscribe_moves().await;
    while let Ok(msg) = rx.recv().await { ... }
}).detach();
```

Delete `spawn_publish_move`, `handle_publish_move`, `publish_local_moves_via_braid`,
`spawn_publish_resign`, `handle_publish_resign`.

Remove `PublishBraidMove` event registration from `BraidPvpPlugin`.

Remove `sender`, `rx`, `last_version`, `last_seen_version` from `BraidPvpSession`.
Remove `drain_incoming_messages` system (moves already arrive via `NetworkEvent::MessageReceived`
from iroh gossip in `systems.rs`).

### Step 3 — Migrate clock to iroh gossip topic

**File: `braid_pvp.rs`** — replace `publish_clock_on_move`:

```rust
fn publish_clock_on_move(
    mut move_events: MessageReader<MoveMadeEvent>,
    session: Res<BraidPvpSession>,
    game_timer: Option<Res<GameTimer>>,
    network_state: Res<BraidNetworkState>,
) {
    for _ in move_events.read() {
        let Some(timer) = game_timer.as_ref() else { continue };
        let clock_topic = format!("/xfchess-game/{}/clock", session.game_id);
        let payload = serde_json::to_vec(&ChessMessage::Clock(ClockState {
            white_ms: timer.white_ms(),
            black_ms: timer.black_ms(),
            timestamp_ms: unix_now_ms(),
        })).unwrap_or_default();
        let update = Update::snapshot(Version::new(uuid::Uuid::new_v4().to_string()), payload);
        if let Some(tx) = &network_state.message_sender {
            // Put directly onto the iroh node's clock topic via a new PutUpdate message
            // variant (see Step 3b below).
        }
    }
}
```

Add `NetworkMessage::PutResource { url: String, payload: Vec<u8> }` variant to `protocol.rs`.
Handle it in `systems.rs` outgoing loop by calling `node.put(&url, update)` instead of gossip broadcast
(it goes into local resource store + broadcasts to topic subscribers).

Migrate `ChessSubscriber::subscribe_clock` to a dynamic gossip subscription:
after `start_session`, send `sub_tx.send(format!("/xfchess-game/{}/clock", id))`.
Receive clock updates in `process_gossip_stream` → decode `ChessMessage::Clock` →
`NetworkEvent::MessageReceived` → existing `drain_clock_to_spectator`.

### Step 4 — Migrate chat to iroh gossip

Add `NetworkMessage::Chat { game_id: u64, player: String, text: String, timestamp_ms: u64 }`.

Replace `handle_publish_chat` + `ChessPublisher::publish_chat()` with
`tx.send(NetworkMessage::Chat { ... })` via gossip.

In `drain_chat_messages`: read `NetworkEvent::MessageReceived(NetworkMessage::Chat { ... })`
instead of `session.chat_rx`.

Remove `chat_rx`, `chat_sender` from `BraidPvpSession` and the chat subscriber task from `start_session`.

### Step 5 — Clean up `BraidPvpSession`

After Steps 2–4 the session struct becomes:

```rust
pub struct BraidPvpSession {
    pub base_url: String,       // kept for PGN export / VPS record_move
    pub game_id: String,
    pub active: bool,
    pub next_move_number: u32,
    pub next_nonce: u64,
    pub wager_amount: f64,
    pub clock_rx: Receiver<ClockState>,   // kept only until Step 3 migration complete
    clock_sender: Sender<ClockState>,
}
```

Once Step 3 is done, `clock_rx`/`clock_sender` are also removed.

### Step 6 — Spectator URL in the UI

In `spectator_overlay.rs`, when `GameMode::Spectator`, show:

```
Spectating — share: http://localhost:8181/xfchess-game/{id}/moves
```

Web users can open that URL (with `Subscribe: true` header) to get a live 209 stream.

---

## What the architecture looks like after

```
Local move
  │
  └── iroh gossip only (PATH A, as today)
        NetworkMessage::Move → node.put() → peer receives
        NetworkMessage::Chat → node.put("/game/{id}/chat") → stored + broadcast
        NetworkMessage::PutResource clock → node.put("/game/{id}/clock") → stored + broadcast

Spectator (desktop) joins:
  └── sub_tx.send("/xfchess-game/{id}") → gossip subscription
        → BraidResyncRequest over gossip → instant catch-up from node.get_updates_since()

Spectator (web browser):
  └── GET http://localhost:8181/xfchess-game/{id}/moves   (Subscribe: true)
        → proxy.rs: HTTP/1.1 → IrohH3Client → iroh node → 209 stream
        → receives all moves in real time

Backend:
  └── record_move() REST call still fires for canonical VPS record + rollup
        (this is separate from the Braid layer — unchanged)

braid_uri (ChessPublisher / ChessSubscriber):
  └── used only in replay_braid.rs (MovePayload type) — no live sessions
```

---

## Risk / caveats

- `iroh-h3-axum` and `iroh-h3-client` are the proxy's H3 adapters. Check they compile on
  Windows before enabling. The feature compiles out cleanly today (`#[cfg(not(feature="proxy"))]`
  stubs exist), so the only risk is new deps not yet tested on the Windows build.

- The local `resources` store in `BraidIrohNode` is **in-memory only** — it disappears when
  the game client exits. Games longer than a session need the VPS `record_move` path (Step 6
  above explicitly keeps that).

- Step 2 removes the `last_version` / `last_seen_version` reconnection fields.
  The `BraidResyncRequest` gossip path (already implemented) covers reconnect catch-up, so
  this is safe to drop.

- Enabling the proxy exposes a local HTTP port. If the user is behind a NAT this is fine.
  If `listen_addr` is `0.0.0.0` it becomes LAN-accessible — keep it `127.0.0.1`.
