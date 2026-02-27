# XFChess Solana Program

## Purpose
The XFChess Solana Program is the on-chain smart contract that powers decentralized chess gaming with wagering on the Solana blockchain. It manages game state, player wagers, move recording, and payout distribution.

## Program ID
```
3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP
```

## Impact on Game
This program is the **source of truth** for all wager-based games:
- **Game Creation:** Initializes game accounts with wager amounts
- **Player Matching:** Validates both players have staked equal amounts
- **Move Recording:** Stores move history on-chain for transparency
- **Payout Distribution:** Automatically distributes pot to winner
- **Session Delegation:** Supports MagicBlock ER for sub-second moves

## Key Components

### Accounts
- **Game:** Core game state (players, wager, status, FEN)
- **MoveLog:** Batched move history for gas efficiency
- **PlayerProfile:** Player statistics and history
- **SessionDelegation:** ER validator delegation records

### Instructions
| Instruction | Purpose |
|-------------|---------|
| `create_game` | Initialize new wager game (Player 1) |
| `join_game` | Join existing game with matching wager (Player 2) |
| `record_move` | Submit a chess move |
| `commit_move_batch` | Batch multiple moves for ER efficiency |
| `finalize_game` | End game and distribute payout |
| `delegate_game` | Delegate to MagicBlock ER validator |
| `withdraw_expired_wager` | Refund if opponent never joins |

## Architecture

```
Player 1              Solana Program              Player 2
   |                        |                        |
   |-- create_game -------->|                        |
   |   (wager: 0.01 SOL)    |                        |
   |<-- Game PDA -----------+                        |
   |                        |                        |
   |                        |<-- join_game ----------|
   |                        |   (wager: 0.01 SOL)    |
   |                        |                        |
   |<-- Game Active --------+-- Game Active -------->|
   |                        |                        |
   |-- record_move -------->|-- record_move -------->|
   |   (e2e4)               |   (e7e5)               |
   |                        |                        |
   |-- finalize_game ------>|-- finalize_game ------>|
   |   (winner: Player 1)   |   (0.02 SOL payout)    |
```

## Game State Lifecycle

1. **WaitingForOpponent** - Game created, waiting for Player 2
2. **Active** - Both players joined, game in progress
3. **Finished** - Game completed, payout distributed

## Account Structure

### Game Account
```rust
pub struct Game {
    pub game_id: u64,
    pub white: Pubkey,
    pub black: Option<Pubkey>,
    pub wager_amount: u64,
    pub escrow_pda: Pubkey,
    pub status: GameStatus,
    pub fen: String,
    pub turn: u8, // 0 = White, 1 = Black
    pub move_count: u16,
    pub created_at: i64,
    pub updated_at: i64,
}
```

## Testing

Run the test suite:
```bash
cd programs/xfchess-game
anchor test
```

## Dependencies
- Anchor Framework v0.30
- Solana Program
- MagicBlock ER (optional feature)

## Configuration
Environment variables for deployment:
```bash
ANCHOR_PROVIDER_URL=https://api.devnet.solana.com
ANCHOR_WALLET=~/.config/solana/id.json
```
