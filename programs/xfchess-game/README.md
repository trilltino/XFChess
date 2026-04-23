# XFChess Solana Program

## Purpose
The XFChess Solana Program is the on-chain smart contract that powers decentralized chess gaming with wagering on the Solana blockchain. It manages game state, player wagers, move recording, and payout distribution.

## Program ID
```
C624Z53FYEVDYVkMWSQ1KPQm4o1Jmdhpc5movSSBnezf
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

## Source Directory Structure
The following is a breakdown of the on-chain programs codebase organized by responsibility:

### Root (`src/`)
- **`constants.rs`**: Program-wide constants, discriminators, and magic numbers.
- **`errors.rs`**: Custom error codes and definitions used across the program.
- **`lib.rs`**: Main entry point mapping all Anchor program instructions to their handler functions.

### Accounts (`src/accounts/`)
- **`mod.rs`**: Exports account-related data structures used internally.
- **`move_batch.rs`**: Structures representing an Ephemeral Rollup move batch payload.
- **`session_delegation.rs`**: Definitions for delegating a user's session internally.

### Account_ix (`src/account_ix/`)
- **`fee_vault_ix.rs`**: Instructions for platform fee vault management and ELO updates.
- **`mod.rs`**: Account management instructions (profiles, user data, fees).
- **`profile.rs`**: Instruction for initializing and verifying player profiles.
- **`set_username.rs`**: Instruction for setting or updating a username associated with a profile.
- **`withdraw.rs`**: Instruction to withdraw wagers from expired/abandoned games.

### Delegation_ix (`src/delegation_ix/`)
- **`delegate.rs`**: Instruction for delegating games to MagicBlock Ephemeral Rollups.
- **`mod.rs`**: Delegation and Ephemeral Rollup integration instructions.
- **`session.rs`**: Instruction for authorizing session keys for passwordless gameplay.
- **`undelegation.rs`**: Instruction to correctly close or resolve games after ER undelegation.

### Game_ix (`src/game_ix/`)
- **`cancel.rs`**: Instruction to cancel a game and return escrowed wagers.
- **`create.rs`**: Instruction to create a new active wagered game context.
- **`finalize.rs`**: Instruction to finalize a completed game and distribute payouts.
- **`join.rs`**: Instruction allowing a second player to match the wager and join a game.
- **`mod.rs`**: Core chess game lifecycle instructions (create, join, cancel, finalize).

### Governance_ix (`src/governance_ix/`)
- **`dispute.rs`**: Instruction for opening a game dispute (e.g., cheating suspected).
- **`mod.rs`**: Governance instructions for resolving disputes and game issues.
- **`resolve.rs`**: Instruction for admins to resolve an open dispute and allocate the prize pool.

### Moves_ix (`src/moves_ix/`)
- **`commit_batch.rs`**: Instruction for syncing batched moves from an ephemeral session to the base layer.
- **`mod.rs`**: Gameplay instructions handling chess moves during an active match.
- **`record.rs`**: Instruction for validating and recording a single state-transitioning chess move.

### State (`src/state/`)
- **`dispute.rs`**: Data models for in-game dispute metadata and evidence.
- **`game.rs`**: Core state structure defining an active or historical game's properties.
- **`mod.rs`**: Contains all global Anchor account structs defining the program's on-chain database layout.
- **`move_log.rs`**: Account struct designed to track the chronological sequence of FENs/moves.
- **`platform_fee_vault.rs`**: Defines the global treasury state holding collected platform fees.
- **`player_profile.rs`**: Account structure encompassing a player's long-term ranking and stats.
- **`player_session.rs`**: Account tracking the state and active keys for a game session.
- **`tournament.rs`**: State structure defining tournament meta-info, prize pools, and progression.
- **`tournament_match.rs`**: State definitions representing a single match bracket within a tournament.
- **`username_record.rs`**: Account structure mapping unique usernames back to player profiles.

### Tournament_ix (`src/tournament_ix/`)
- **`cancel.rs`**: Instruction for safely halting a tournament and refunding entry fees.
- **`claim_prize.rs`**: Instruction allowing the ultimate winners to claim their tournament share.
- **`initialize.rs`**: Instruction to bootstrap a new bracket-based tournament.
- **`mod.rs`**: Instructions managing multi-player structured tournaments.
- **`record_result.rs`**: Instruction resolving an individual tournament game to advance players.
- **`register.rs`**: Instruction allowing players to opt-in and pay their entry fee for the tournament.
- **`start.rs`**: Instruction to lock registration and officially generate ongoing tournament brackets.

## Source Directory Structure
The following is a breakdown of the on-chain programs codebase organized by responsibility:

### Root (`src/`)
- **`constants.rs`**: Program-wide constants, discriminators, and magic numbers.
- **`errors.rs`**: Custom error codes and definitions used across the program.
- **`lib.rs`**: Main entry point mapping all Anchor program instructions to their handler functions.

### Accounts (`src/accounts/`)
- **`mod.rs`**: Exports account-related data structures used internally.
- **`move_batch.rs`**: Structures representing an Ephemeral Rollup move batch payload.
- **`session_delegation.rs`**: Definitions for delegating a user's session internally.

### Account_ix (`src/account_ix/`)
- **`fee_vault_ix.rs`**: Instructions for platform fee vault management and ELO updates.
- **`mod.rs`**: Account management instructions (profiles, user data, fees).
- **`profile.rs`**: Instruction for initializing and verifying player profiles.
- **`set_username.rs`**: Instruction for setting or updating a username associated with a profile.
- **`withdraw.rs`**: Instruction to withdraw wagers from expired/abandoned games.

### Delegation_ix (`src/delegation_ix/`)
- **`delegate.rs`**: Instruction for delegating games to MagicBlock Ephemeral Rollups.
- **`mod.rs`**: Delegation and Ephemeral Rollup integration instructions.
- **`session.rs`**: Instruction for authorizing session keys for passwordless gameplay.
- **`undelegation.rs`**: Instruction to correctly close or resolve games after ER undelegation.

### Game_ix (`src/game_ix/`)
- **`cancel.rs`**: Instruction to cancel a game and return escrowed wagers.
- **`create.rs`**: Instruction to create a new active wagered game context.
- **`finalize.rs`**: Instruction to finalize a completed game and distribute payouts.
- **`join.rs`**: Instruction allowing a second player to match the wager and join a game.
- **`mod.rs`**: Core chess game lifecycle instructions (create, join, cancel, finalize).

### Governance_ix (`src/governance_ix/`)
- **`dispute.rs`**: Instruction for opening a game dispute (e.g., cheating suspected).
- **`mod.rs`**: Governance instructions for resolving disputes and game issues.
- **`resolve.rs`**: Instruction for admins to resolve an open dispute and allocate the prize pool.

### Moves_ix (`src/moves_ix/`)
- **`commit_batch.rs`**: Instruction for syncing batched moves from an ephemeral session to the base layer.
- **`mod.rs`**: Gameplay instructions handling chess moves during an active match.
- **`record.rs`**: Instruction for validating and recording a single state-transitioning chess move.

### State (`src/state/`)
- **`dispute.rs`**: Data models for in-game dispute metadata and evidence.
- **`game.rs`**: Core state structure defining an active or historical game's properties.
- **`mod.rs`**: Contains all global Anchor account structs defining the program's on-chain database layout.
- **`move_log.rs`**: Account struct designed to track the chronological sequence of FENs/moves.
- **`platform_fee_vault.rs`**: Defines the global treasury state holding collected platform fees.
- **`player_profile.rs`**: Account structure encompassing a player's long-term ranking and stats.
- **`player_session.rs`**: Account tracking the state and active keys for a game session.
- **`tournament.rs`**: State structure defining tournament meta-info, prize pools, and progression.
- **`tournament_match.rs`**: State definitions representing a single match bracket within a tournament.
- **`username_record.rs`**: Account structure mapping unique usernames back to player profiles.

### Tournament_ix (`src/tournament_ix/`)
- **`cancel.rs`**: Instruction for safely halting a tournament and refunding entry fees.
- **`claim_prize.rs`**: Instruction allowing the ultimate winners to claim their tournament share.
- **`initialize.rs`**: Instruction to bootstrap a new bracket-based tournament.
- **`mod.rs`**: Instructions managing multi-player structured tournaments.
- **`record_result.rs`**: Instruction resolving an individual tournament game to advance players.
- **`register.rs`**: Instruction allowing players to opt-in and pay their entry fee for the tournament.
- **`start.rs`**: Instruction to lock registration and officially generate ongoing tournament brackets.
