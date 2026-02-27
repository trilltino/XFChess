# solana-chess-client

## Purpose

Solana RPC client for the xfchess-game Anchor program. Provides high-level methods to interact with on-chain chess games.

## Role in XFChess

**CRITICAL for Ephemeral Rollups integration.** Used by:

- Main application for all Solana transactions
- `xfchess-ai-service` for AI opponent
- ER integration (delegate, commit, finalize)

## Key Features

| Feature | Description |
|---------|-------------|
| `ChessRpcClient` | High-level RPC wrapper |
| `create_game_ix` | Build create_game instruction |
| `create_join_game_ix` | Build join_game instruction |
| `create_delegate_game_ix` | Build delegate_game instruction (ER) |
| `create_commit_move_batch_ix` | Build commit_move_batch instruction (ER) |
| `create_finalize_game_ix` | Build finalize_game instruction |

## Usage in Main Application

```rust
// src/multiplayer/solana_integration.rs
use solana_chess_client::rpc::ChessRpcClient;

let client = ChessRpcClient::new(rpc_url)?;

// Create wagered game
let ix = client.create_create_game_ix(
    payer.pubkey(),
    game_id,
    wager_amount,
)?;

// Delegate to ER
let delegate_ix = client.create_delegate_game_ix(
    payer.pubkey(),
    game_id,
)?;
```

## Architecture

```
┌─────────────────────────────────────┐
│      XFChess Main Application       │
│  src/multiplayer/solana_integration │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│   solana-chess-client               │ ◄── YOU ARE HERE
│   - ChessRpcClient                  │
│   - Instruction builders            │
│   - Transaction helpers             │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│   xfchess-game (Anchor program)     │
└─────────────────────────────────────┘
```

## Dependencies

- `solana-sdk` - Solana types
- `solana-client` - RPC client
- `xfchess-game` - Program types

## Notes

- **CRITICAL for ER plan** - all on-chain interactions go through this
- Must be updated alongside program changes
- Handles transaction building and signing
