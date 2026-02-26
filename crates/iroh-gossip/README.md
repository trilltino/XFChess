# iroh-gossip

## Purpose

**FORK** of the iroh-gossip crate from n0-computer/iroh. Provides gossip-based message broadcasting over Iroh's peer-to-peer network.

## Role in XFChess

**Underlying gossip protocol for Braid networking.**

This is a forked/modified version of the official iroh-gossip crate with potential customizations for XFChess needs.

## Key Features

| Feature | Description |
|---------|-------------|
| HyParView | Peer membership management |
| PlumTree | Epidemic broadcast protocol |
| Topics | Named channels for message routing |
| Metrics | Performance and health monitoring |

## Architecture

```
┌─────────────────────┐
│   braid-iroh        │
│   (Braid layer)     │
└────────┬────────────┘
         │
         ▼
┌─────────────────────┐
│   iroh-gossip       │ ◄── YOU ARE HERE (forked)
│   - HyParView       │
│   - PlumTree        │
│   - Topic routing   │
└─────────────────────┘
```

## Status

| Aspect | Status |
|--------|--------|
| Forked from | n0-computer/iroh |
| Modifications | Custom changes possible |
| Critical | Yes - required by braid-iroh |

## Notes

- **FORKED LIBRARY** - may have custom patches
- **DO NOT REMOVE** - required by braid-iroh
- Used for P2P matchmaking and game sync
- Part of the Iroh networking stack
