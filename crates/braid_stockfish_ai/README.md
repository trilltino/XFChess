# braid_stockfish_ai

## Purpose

AI opponent service for wagered chess games on Solana. Runs as a standalone service that monitors on-chain games and responds with Stockfish-powered moves.

## Role in XFChess

This crate provides the **AI Authority** for PvAI (Player vs AI) wagered games:

- Watches Solana program for new game invites
- Accepts wagers on behalf of the AI
- Computes optimal moves using Stockfish engine
- Submits moves to the on-chain program
- Handles game finalization and fund distribution

## Architecture

```
┌─────────────────────┐
│ Solana Blockchain   │◄──── Game PDA with wager
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ xfchess-ai-service  │ (uses this crate)
│ - Polls for games   │
│ - Accepts invites   │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ braid_stockfish_ai  │ ◄── YOU ARE HERE
│ - Stockfish UCI     │
│ - Move calculation  │
│ - Difficulty levels │
└─────────────────────┘
```

## Usage

Only used by `crates/xfchess-ai-service`. Not linked into the main XFChess client.

## Dependencies

- `stockfish` - Chess engine binary (must be available at runtime)
- `tokio` - Async runtime
- `solana-sdk` - For transaction signing

## Notes

- Requires `ai-authority.json` keypair for signing transactions
- Should run on a server with reliable internet for production
- NOT required for ER integration - this is a separate AI service
