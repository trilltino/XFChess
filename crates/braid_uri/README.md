# braid_uri

## Purpose

HTTP protocol layer for P2P chess games using the Braid synchronization protocol. Provides typed messages and publish/subscribe over standard HTTP.

## Role in XFChess

Enables **Braid P2P multiplayer mode** - a decentralized alternative to Solana for casual games:

- **P2P Move Sync**: Send/receive moves without blockchain latency
- **Game Events**: Resign, offer draw, engine hints
- **Braid Protocol**: Real-time sync via HTTP SSE (Server-Sent Events)

## Key Types

| Type | Purpose |
|------|---------|
| `ChessMessage` | All game events (Move, Resign, OfferDraw, etc.) |
| `ChessPublisher` | PUT moves to Braid HTTP endpoint |
| `ChessSubscriber` | Subscribe to opponent moves via SSE |
| `ChessUri` | Typed resource paths (`/game/{id}/moves`) |

## Usage in Main Application

```rust
// src/multiplayer/braid_node.rs
use braid_uri::{ChessMessage, ChessPublisher, ChessSubscriber};

// Host: Create publisher + subscriber
let publisher = ChessPublisher::new(&url, &game_id)?;
let subscriber = ChessSubscriber::new(&url, &game_id)?;

// Publish local move
publisher.publish_move(&move_payload).await?;

// Subscribe to opponent moves
let (rx, _handle) = subscriber.subscribe_moves().await?;
while let Ok(msg) = rx.recv().await {
    match msg {
        ChessMessage::Move(mv) => apply_move(mv),
        ChessMessage::Resign { player } => end_game(),
        ...
    }
}
```

## Architecture

```
┌─────────────────┐         ┌─────────────────┐
│   Player A      │         │   Player B      │
│  (BraidNode)    │◄───────►│  (BraidNode)    │
└────────┬────────┘  HTTP   └─────────────────┘
         │
         ▼
┌─────────────────────┐
│   braid_uri         │ ◄── YOU ARE HERE
│   - ChessMessage    │
│   - ChessPublisher  │
│   - ChessSubscriber │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│   braid-http        │
│   (Braid protocol)  │
└─────────────────────┘
```

## Dependencies

- `braid-http` - Core Braid HTTP client
- `serde` - Message serialization
- `sha2` - Version hashing

## Notes

- **NOT used for Solana/ER integration** - separate networking stack
- Used in `BraidMultiplayer` mode (optional P2P feature)
- Braid protocol provides real-time sync without blockchain fees
