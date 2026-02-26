# shared

## Purpose

Shared protocol types and messages for cross-crate communication. Provides common definitions used by both client and server components.

## Role in XFChess

**Protocol glue for multiplayer networking.**

Defines message types used across:
- Main application (`src/multiplayer/`)
- `braid-iroh` - Iroh networking
- `braid_uri` - Braid HTTP protocol

## Key Types

| Type | Purpose |
|------|---------|
| `GameMessage` | In-game events (Move, GameStart, etc.) |
| `LobbyMessage` | Matchmaking messages |

## Architecture

```
┌─────────────────┐
│   Main App      │
└────────┬────────┘
         │ GameMessage
         ▼
┌─────────────────────┐
│   shared            │ ◄── YOU ARE HERE
│   - GameMessage     │
│   - LobbyMessage    │
└──────────┬──────────┘
           │
    ┌──────┴──────┐
    ▼             ▼
┌─────────┐   ┌─────────┐
│braid-iroh│   │braid_uri│
└─────────┘   └─────────┘
```

## Usage

```rust
// Used throughout multiplayer modules
use shared::protocol::{GameMessage, LobbyMessage};

// Sent/received over Braid network
let msg = GameMessage::Move { from, to, ... };
```

## Dependencies

- `serde` - Serialization

## Notes

- **Lightweight** - only protocol definitions
- **Required** for P2P multiplayer
- Keeps message types consistent across crates
