# XFChess Smart Contracts Guide

This document explains how the on-chain smart contracts power the XFChess decentralized chess platform.

## Overview

The XFChess program is an **Anchor-based Solana smart contract** that enables:
- **Wager-based chess games** with SOL staking
- **Tournament management** with bracket progression
- **Session delegation** via MagicBlock Ephemeral Rollups for sub-second moves
- **ELO rating system** with Glicko-2 calculations
- **Dispute resolution** for fair play governance

**Program ID**: `C624Z53FYEVDYVkMWSQ1KPQm4o1Jmdhpc5movSSBnezf`

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        CLIENT APPLICATION                        │
│                    (Rust Desktop / Web App)                      │
└─────────────────────────────┬───────────────────────────────────┘
                              │
        ┌─────────────────────┴─────────────────────┐
        │                                           │
        ▼                                           ▼
┌───────────────┐                         ┌─────────────────┐
│  MAGICBLOCK   │                         │   SOLANA BASE   │
│     ER        │◄───────────────────────►│     LAYER       │
│ (Ephemeral    │   Delegate/Undelegate   │  (Smart Contract)│
│  Rollups)     │   Commit Move Batches   │                 │
└───────────────┘                         └─────────────────┘
                                                  │
                    ┌─────────────────────────────┼─────────────────────────────┐
                    ▼                             ▼                             ▼
            ┌───────────────┐            ┌───────────────┐              ┌───────────────┐
            │  GAME STATE   │            │  TOURNAMENT   │              │   PROFILES    │
            │     PDAs      │            │     PDAs      │              │     PDAs      │
            └───────────────┘            └───────────────┘              └───────────────┘
```

---

## Core Account Types

### 1. Game Account

The central data structure for a chess match:

```rust
pub struct Game {
    pub game_id: u64,              // Unique identifier
    pub white: Pubkey,           // White player's wallet
    pub black: Pubkey,           // Black player's wallet
    pub status: GameStatus,      // Current lifecycle state
    pub fen: String,             // Board position (FEN notation)
    pub wager_amount: u64,       // Lamports each player staked
    pub move_count: u16,         // Total half-moves made
    pub turn: u8,                // 1 = white to move, 2 = black, etc.
    pub is_delegated: bool,      // True if on Ephemeral Rollup
    pub match_type: MatchType,   // Free, Rated, Wager, or Tournament
    pub result: GameResult,      // None, Winner(Pubkey), or Draw
    pub tournament_id: Option<u64>, // Link to tournament if applicable
}
```

**PDA Seeds**: `[b"game", game_id.to_le_bytes()]`

**Game Status Lifecycle**:
```
WaitingForOpponent → Active → Finished → Settled
         │              │          │
         ▼              ▼          ▼
    Cancelled      Disputed   Expired
```

### 2. Session Delegation Account

Enables passwordless gameplay via session keys:

```rust
pub struct SessionDelegation {
    pub game_id: u64,
    pub player: Pubkey,          // Real wallet address
    pub session_key: Pubkey,     // Hot key on VPS/relayer
    pub expires_at: i64,         // Session TTL
    pub enabled: bool,           // Can be revoked
    pub max_batch_len: u16,    // Batch size for ER commits
}
```

**Use Case**: Players authorize a session key once. The relayer then submits moves without requiring wallet popups for each move.

### 3. Tournament Account

Manages multi-player structured competitions:

```rust
pub struct Tournament {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee: u64,          // Cost to register
    pub prize_pool: u64,         // Accumulated entry fees
    pub max_players: u16,        // Power of 2 (8, 16, 32, 64, 128, 256)
    pub status: TournamentStatus, // Registration → Active → Completed
    pub tournament_type: TournamentType, // Swiss or Single-Elimination
    pub prize_shares: [u16; 10], // Prize distribution in basis points
    pub players: Vec<Pubkey>,    // Registered participants
    pub player_elos: Vec<u32>,   // ELO at registration time
}
```

**Prize Distribution Examples**:
- **≤64 players**: Top 3 receive 60/30/10%
- **128 players**: Top 5 receive 50/25/15/5/5%
- **256 players**: Top 10 receive graduated payouts
- **Winner-takes-all**: 100% to 1st place

### 4. Player Profile Account

Stores persistent player statistics:

```rust
pub struct PlayerProfile {
    pub owner: Pubkey,           // Wallet address
    pub username: String,
    pub elo_rating: u32,         // Current ELO (starts at 1500)
    pub elo_rd: u32,            // Rating deviation (Glicko-2)
    pub games_played: u32,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub ranked_games: u32,
    pub tournament_wins: u32,
    pub win_streak: u16,
    pub best_streak: u16,
    pub country: String,       // For regional fee compliance
}
```

---

## Instruction Reference

### Game Lifecycle Instructions

| Instruction | Description | Who Calls |
|-------------|-------------|-----------|
| `create_game` | Initialize new game with wager | Player 1 |
| `join_game` | Match wager and join as opponent | Player 2 |
| `record_move` | Submit a chess move | Either player (via session) |
| `finalize_game` | Settle game and distribute pot | Either player |
| `resign` | Concede defeat | Either player |
| `claim_timeout` | Win by time forfeit | Opponent |
| `cancel_game` | Abort and refund if no opponent | Creator |

**Example: Creating a Game**
```rust
// Player stakes wager_amount lamports
// Creates Game PDA + MoveLog PDA + Escrow PDA
create_game(
    game_id: u64,               // Client-generated unique ID
    wager_amount: u64,          // Lamports to stake (0 for casual)
    match_type: MatchType,      // Free or Rated
    country: String,           // "GB", "US", etc. for regional fees
    base_time_seconds: u64,    // Chess clock (0 = no limit)
    increment_seconds: u16,    // Fischer increment
)
```

**Example: Recording a Move**
```rust
record_move(
    game_id: u64,
    move_str: String,          // UCI notation (e.g., "e2e4")
    next_fen: String,          // Resulting board state
    nonce: u64,                // Replay protection counter
    signature: Option<Vec<u8>>, // Player's off-chain signature
)
```

### Tournament Instructions

| Instruction | Description |
|-------------|-------------|
| `initialize_tournament` | Create new tournament with parameters |
| `register_player` | Join tournament by paying entry fee |
| `leave_tournament` | Exit before start (full refund) |
| `start_tournament` | Lock registration, generate brackets |
| `initialize_match` | Create individual bracket match |
| `record_match_result` | Report winner/loser for bracket advancement |
| `record_swiss_result` | Report round result for Swiss tournaments |
| `advance_winner` | Move winner to next bracket round |
| `claim_tournament_prize` | Winners withdraw their share |
| `cancel_tournament` | Abort and refund all entries |
| `fund_usdc_prize` | Add USDC to prize pool |

### Session & Delegation Instructions

| Instruction | Description |
|-------------|-------------|
| `authorize_session_key` | Allow session key to act on player's behalf |
| `revoke_session_key` | Disable an active session |
| `delegate_game` | Move game state to Ephemeral Rollup |
| `undelegate_game` | Return game to base layer |
| `process_undelegation` | Handle post-undelegation cleanup |
| `create_session` | Create fee vault session for tournament play |
| `revoke_session` | Revoke tournament session |

### Governance Instructions

| Instruction | Description | Authority Required |
|-------------|-------------|-------------------|
| `dispute_game` | Open a dispute with evidence | Either player |
| `resolve_dispute` | Admin resolves and allocates prize | Dispute Authority |
| `claim_stale_dispute` | Auto-split after 7 days | Either player |

---

## Money Flow

### Wager Game Economics

```
Player 1          Escrow PDA           Player 2         Treasury
    │                  │                  │                 │
    ├─wager_amount────►│                  │                 │
    │                  │                  │                 │
    │                  │◄──wager_amount───┤                 │
    │                  │                  │                 │
    │                  │ (Game plays)     │                 │
    │                  │                  │                 │
    │◄────2×wager──────┤ (Winner paid)    │                 │
    │                  │                  │                 │
    │                  │─country_fee─────►├────►(Regional Treasury)
```

### Fee Structure

| Component | Amount | Purpose |
|-----------|--------|---------|
| **Wager Range** | 0.001 - 10 SOL | Min/max stake per game |
| **Platform Fee** | 5% of pot | Operational costs |
| **Country Fees** | Variable (GB: 0.05 SOL) | Regional compliance |
| **ELO Update Fee** | 0.000005 SOL | Rating calculation |
| **Transaction Costs** | ~10,000 lamports | Relayer reimbursement |

---

## Ephemeral Rollups Integration

### Why Delegation?

Standard Solana has ~400ms block times. For real-time chess with clocks, moves must be faster.

**MagicBlock ER** provides:
- **Sub-second finality** within the rollup
- **Zero transaction fees** for delegated moves
- **Periodic commits** back to base layer for finality

### Delegation Flow

```
1. CREATE GAME (on Base Layer)
   └── Game PDA created, wagers escrowed

2. DELEGATE GAME
   └── State copied to Ephemeral Rollup
   └── Game marked is_delegated = true

3. PLAY ON ER
   └── record_move called on ER (no fees, instant)
   └── Moves batched and signed with session_key

4. UNDELEGATE/COMMIT
   └── Final state written back to base layer
   └── Winner claims, ELO updates, fees settled
```

### Security Model

- **Session keys** are time-limited and game-specific
- **Player wallet** retains ultimate authority
- **Batch commitments** include player signatures for verification
- **Undelegation** can be forced if ER validator misbehaves

---

## PDA Seed Reference

| Account Type | Seeds | Purpose |
|--------------|-------|---------|
| Game | `["game", game_id]` | Core game state |
| MoveLog | `["move_log", game_id]` | Move history |
| Escrow | `["escrow", game_id]` | Wager holding |
| Profile | `["profile", owner_pubkey]` | Player stats |
| Username | `["username", username]` | Name uniqueness |
| SessionDelegation | `["session_delegation", game_id, player]` | Session auth |
| Tournament | `["tournament", tournament_id]` | Tournament state |
| TournamentMatch | `["t_match", tournament_id, match_index]` | Individual match |
| TournamentEscrow | `["t_escrow", tournament_id]` | Entry fee pool |

---

## ELO Rating System

Uses **Glicko-2** algorithm for accurate rating calculation:

```
New Rating = Old Rating + K × (Actual Score - Expected Score)

Where:
- K = 32 (development factor)
- Actual Score: 1 for win, 0.5 for draw, 0 for loss
- Expected Score: Based on rating difference
```

Rating deviation (RD) decreases with more games played, increasing confidence in the rating.

---

## Dispute Resolution

### Dispute Lifecycle

```
Player A ──dispute_game──► Game marked Disputed
                                │
                                ▼
                    Evidence hash stored on-chain
                    Reason logged
                                │
                                ▼
                    7-day resolution window
                                │
                ┌───────────────┴───────────────┐
                ▼                               ▼
        Admin resolves                  No action taken
        (resolve_dispute)               (claim_stale_dispute)
                │                               │
                ▼                               ▼
        Winner declared                 50/50 split
        Prize allocated                 Auto-distributed
```

---

## Error Codes

Common errors you may encounter:

| Error | Cause | Resolution |
|-------|-------|------------|
| `GameNotActive` | Move submitted to finished game | Check game status first |
| `NotYourTurn` | Player moved out of turn | Wait for opponent |
| `WagerTooHigh` | Stake exceeds 10 SOL | Reduce wager amount |
| `InvalidSessionKey` | Session not authorized | Call authorize_session_key |
| `SessionExpired` | TTL exceeded | Revoke and create new session |
| `InvalidNonce` | Replay attack protection | Use correct sequence number |

---

## Deployment

### Environment Variables
```bash
ANCHOR_PROVIDER_URL=https://api.devnet.solana.com
ANCHOR_WALLET=~/.config/solana/id.json
```

### Build & Test
```bash
cd programs/xfchess-game
anchor build
anchor test
```

### Deploy
```bash
anchor deploy --provider.cluster devnet
```

---

## Related Documentation

- `programs/xfchess-game/README.md` - Technical program details
- `MAGICBLOCK_SETUP.md` - ER integration setup
- `revenue-model.md` - Economic model
