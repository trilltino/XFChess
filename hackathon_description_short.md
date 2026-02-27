i# XFChess - Project Description

## What I Built

XFChess is a 3D chess game where players can bet SOL on matches. It's built entirely in Rust - from the smart contracts to the game engine. No servers needed, runs on peer-to-peer networking.

The game uses Bevy for rendering, has native Stockfish AI integration (first 3D chess to do this), and supports wagered matches on Solana devnet.

## Sponsor Prize: MagicBlock Ephemeral Rollups

I'm applying for the MagicBlock ER prize. Here's what I implemented:

### Solana Contracts (Anchor)
- Game PDA with wager escrow - players deposit SOL when joining, winner gets auto-payout
- Session delegation - ephemeral keys sign moves so players don't need to approve every transaction
- Move batching - compress multiple moves into single on-chain commit

### MagicBlock Ephemeral Rollup SDK
- `delegate_game` instruction using `ephemeral_rollups_sdk::cpi::delegate_account`
- `undelegate_game` with `commit_and_undelegate_accounts` for settlement
- Session keys authorized via `SessionDelegation` accounts (2hr expiry)
- Transactions routed to `https://devnet-eu.magicblock.app` during gameplay

### Bevy Integration
- Real-time 3D board with piece animations
- P2P move sync via Iroh gossip - no central server
- Web launcher (React) generates session keys, passes to native Bevy client

## How It Works

1. Players connect wallets in browser, create game lobby
2. Session keys generated, game PDA delegated to ER
3. Bevy client launches, connects P2P to opponent
4. Moves execute locally in ~100ms, batched to ER
5. Game ends → undelegate commits final state + distributes wager

The whole thing is Rust end-to-end. Smart contracts to graphics.

## Roadmap

- Clean up codebase and ship to Superteam UK community for live games
- First NFT drop: tradeable board themes and piece sets (artists earn royalties on every match played with their assets)
- Security audits on the wager escrow and session delegation logic
- Bot detection/monitoring to keep competitive play fair
- Tournament brackets with automated payouts
- Mainnet deployment after battle-testing on devnet
