# xfchess-ai-service

## Purpose

Standalone AI service for PvAI (Player vs AI) wagered games on Solana. Monitors the blockchain for game invites and plays as an AI opponent.

## Role in XFChess

**Server-side AI opponent for wagered games.**

Runs as a separate binary (not part of main XFChess client):
- Polls Solana for new game invitations
- Accepts wagers on behalf of the AI
- Uses Stockfish to compute optimal moves
- Signs and submits transactions
- Handles game finalization

## Architecture

```
┌─────────────────────────────────────────┐
│         Solana Blockchain               │
│  ┌─────────┐      ┌─────────────────┐  │
│  │ Game PDA│◄────►│ xfchess-game    │  │
│  │ Wager   │      │ program         │  │
│  └────┬────┘      └─────────────────┘  │
└───────┬─────────────────────────────────┘
        │
        │ Poll for games
        ▼
┌─────────────────────────────────────────┐
│   xfchess-ai-service                    │ ◄── YOU ARE HERE
│   ┌─────────────────────────────────┐   │
│   │ 1. Monitor on-chain games       │   │
│   │ 2. Accept AI vs Player invites  │   │
│   │ 3. Compute moves (Stockfish)    │   │
│   │ 4. Submit signed transactions   │   │
│   └─────────────────────────────────┘   │
└──────────────────┬──────────────────────┘
                   │
                   ▼
         ┌─────────────────────┐
         │  braid_stockfish_ai │
         │  - Stockfish UCI    │
         │  - Move evaluation  │
         └─────────────────────┘
```

## Configuration

Requires `ai-authority.json` keypair file for signing transactions.

## Usage

```bash
# Run the AI service
cargo run -p xfchess-ai-service
```

## Dependencies

- `solana-chess-client` - RPC client
- `braid_stockfish_ai` - Stockfish integration
- `solana-sdk` - Transaction signing

## Notes

- **Separate binary** - not linked to main XFChess app
- **Server-side** - runs on infrastructure, not player machines
- **NOT required for ER integration** - this is for AI opponents only
- Can be deployed independently from the main game
