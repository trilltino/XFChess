# braid-iroh

## Purpose

Iroh-based networking transport for the Braid synchronization protocol. Provides peer-to-peer networking using the Iroh library (QUIC + gossip).

## Role in XFChess

**Primary P2P networking layer.** Used by:

- Main application (`src/multiplayer/`) for Braid P2P mode
- Alternative to `braid-http` for lower-latency connections

## Key Features

| Feature | Description |
|---------|-------------|
| `BraidIrohNode` | Iroh node with Braid protocol support |
| Gossip | Topic-based message broadcasting |
| Discovery | Peer discovery via Iroh's discovery service |
| Direct connections | QUIC-based direct peer connections |

## Architecture

```
┌─────────────────────────────────┐
│     XFChess Main Application    │
│  src/multiplayer/braid_node.rs  │
└─────────────┬───────────────────┘
              │
              ▼
┌─────────────────────┐
│   braid-iroh        │ ◄── YOU ARE HERE
│   - BraidIrohNode   │
│   - Gossip          │
│   - Discovery       │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│   iroh-gossip       │
│   (forked library)  │
└─────────────────────┘
```

## Usage in Main Application

```rust
// src/multiplayer/mod.rs
use braid_iroh::{BraidIrohNode, BraidGameConfig, DiscoveryConfig};

let config = BraidGameConfig {
    secret_key: Some(secret_key),
    discovery: DiscoveryConfig::Default,
    ...
};

let node = BraidIrohNode::spawn(config).await?;
let mut rx = node.subscribe(GAME_TOPIC, vec![]).await?;
```

## Dependencies

- `iroh-gossip` - Gossip protocol (forked)
- `braid-core` - Core Braid protocol
- `tokio` - Async runtime

## Notes

- **PRIMARY P2P transport** in XFChess
- Used for gossip-based matchmaking
- More efficient than HTTP for real-time games
- NOT used for Solana/ER integration
