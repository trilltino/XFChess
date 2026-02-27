# XFChess - Graveyard Hackathon Submission

**By Tino | Solana SuperteamUK**

Hi, I'm Tino - a Rust developer from Solana SuperteamUK, and my submission for the Graveyard Hackathon is **XFChess**.

XFChess is a Chess game I've been working on that takes advantage of Solana and Ephemeral Rollups to create a strong financialised layer that can not only grow the game's users but also provides opportunities for 3D artists and developers.

Simply put, it is a 3D chess game built end-to-end in Rust with my own networking protocol (Braid) which means there is no need for a server - allowing it to be played anywhere with no downtime.

This provides several opportunities:
- **For Solana Devs**: Build games end-to-end in Rust, a language they are already familiar with
- **No Servers**: Scaling is simple and affordable
- **Open Source**: A tool that anyone can fork, learn from, or innovate with

Online Chess is one of the most played games in the world. The game also includes Stockfish as an AI opponent, as well as allowing developers to use their own chess engine for testing purposes.

**It is the first 3D chess game that utilises Stockfish natively.**

---

## The Financial Layer: Solana Smart Contracts & Ephemeral Rollups

### Why This Matters

Traditional online chess lacks provable fairness and financial incentives. XFChess solves this by leveraging Solana's blockchain for:
- **Trustless wagering** - Bet SOL on matches with automated payouts
- **Provable game history** - Every move recorded immutably on-chain
- **No middlemen** - Smart contracts handle everything

### Solana Smart Contract Architecture

The on-chain program (`programs/xfchess-game/`) is built with Anchor and handles:

#### 1. **Game State Management**
```rust
pub struct Game {
    pub game_id: u64,
    pub white: Pubkey,
    pub black: Pubkey,
    pub wager_amount: u64,
    pub status: GameStatus,
    pub move_log: Vec<MoveRecord>,
}
```

#### 2. **Wager Escrow**
- Players deposit SOL into a PDA when joining
- Funds are locked until game completion
- Winner receives automatic payout via smart contract

#### 3. **Session Delegation**
```rust
pub struct SessionDelegation {
    pub session_key: Pubkey,
    pub expires_at: i64,
    pub game_pda: Pubkey,
}
```
- Ephemeral session keys enable sub-second gameplay
- Delegated authority for move signing
- Automatic expiration and revocation

### Ephemeral Rollups (ER) Integration

**The Problem**: Traditional blockchain gaming is slow. Waiting 400ms-12s per move kills the experience.

**The Solution**: MagicBlock's Ephemeral Rollups provide sub-second finality while maintaining Solana's security guarantees.

#### How It Works

```
┌─────────────────────────────────────────────────────────────────┐
│              TRADITIONAL vs EPHEMERAL ROLLUPS                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  TRADITIONAL (Slow)              EPHEMERAL ROLLUPS (Fast)       │
│  ────────────────────            ────────────────────────       │
│                                                                  │
│  Player makes move               Player makes move               │
│       ↓                               ↓                          │
│  Wait for Solana block           Send to ER validator            │
│  (~400-12,000ms)                 (~100ms)                        │
│       ↓                               ↓                          │
│  Move confirmed                  Move confirmed instantly        │
│  Opponent sees move              Opponent sees move via P2P      │
│                                                                  │
│  4 moves = ~2-48 seconds         4 moves = ~400ms total          │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

#### Technical Implementation

1. **Delegation Phase**
   - Game PDA is delegated to ER validator on game start
   - Session keys authorized for ephemeral signing
   - State exists on both Solana and ER

2. **Gameplay Phase**
   - Moves sent to ER endpoint (`https://devnet-eu.magicblock.app`)
   - ER provides sub-second confirmation
   - P2P network (Braid/Iroh) syncs moves between players
   - Moves batched locally, committed periodically

3. **Settlement Phase**
   - Game ends (checkmate/resignation)
   - Final state undelegated from ER
   - All moves committed to Solana in single transaction
   - Smart contract distributes wager to winner

### Financial Opportunities

#### For Players
- **Skill-based earnings** - Win SOL by beating opponents
- **Transparent odds** - No house edge, pure PvP
- **Instant withdrawals** - Winnings available immediately after game

#### For Developers
- **Fee monetization** - Take small % of wagers
- **NFT integration** - Premium boards/pieces as NFTs
- **Tournament hosting** - Automated bracket management via smart contracts

#### For Artists
- **3D asset marketplace** - Sell custom piece sets
- **Board themes** - Seasonal/limited edition boards
- **Royalties** - Earn on every match played with your assets

### Why Graveyard Hackathon?

XFChess represents a paradigm shift for blockchain gaming:

1. **Rust-Native** - No JavaScript/TypeScript required. Pure Rust from smart contracts to game engine
2. **Serverless** - P2P networking eliminates infrastructure costs
3. **Provably Fair** - Every move on-chain, no cheating possible
4. **Actually Fun** - Sub-second gameplay makes it competitive with traditional chess platforms
5. **Open Source** - Full codebase available for the community to build upon

### Program Details

- **Program ID**: `3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP`
- **Network**: Solana Devnet (mainnet ready)
- **Features**: Wager games, session delegation, move verification, automated payouts
- **Tech Stack**: Anchor, MagicBlock ER, Braid P2P, Bevy Engine

---

**Play Anywhere. Own Your History. Win Real Value.**

XFChess isn't just a chess game - it's a demonstration of what's possible when you combine Solana's financial infrastructure with cutting-edge P2P networking and world-class game engines.

*Built with ❤️ by Tino | Solana SuperteamUK*
