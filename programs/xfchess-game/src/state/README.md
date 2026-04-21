# State

Solana program state structures and account definitions for the XFChess smart contract.

## Overview of Solana Accounts

In Solana, data is stored in "accounts" which are similar to structs in other languages. Each account has:
- A unique address (public key)
- Data (the actual state)
- Lamports (SOL balance for rent exemption)
- Owner (the program that can modify the data)

Anchor, the framework used for this program, simplifies account management with the `#[account]` macro, which automatically handles:
- Space allocation (8 bytes for discriminator + struct size)
- PDA (Program Derived Address) derivation
- Bump seeds for collision resistance
- Serialization/deserialization

## Components

- **tournament.rs** - Tournament PDA (Program Derived Address) state
- **tournament_match.rs** - Tournament match PDA state
- Account definitions for game state, players, and tournaments

## Account Types

### Tournament Account

The tournament account stores all information about a tournament, including its status, participants, prize pool, and current round.

```rust
use anchor_lang::prelude::*;

/// Tournament account representing a chess tournament on-chain
/// 
/// This account is a PDA derived from the tournament authority's public key,
/// ensuring each authority can only have one active tournament at a time.
/// 
/// # Fields
/// * `authority` - The public key of the tournament organizer/owner
/// * `entry_fee` - Required SOL amount to join the tournament (in lamports)
/// * `prize_pool` - Total SOL available as prize (in lamports)
/// * `status` - Current state of the tournament (Open, InProgress, etc.)
/// * `player_count` - Number of players currently registered
/// * `round` - Current tournament round (1-indexed)
/// * `bump` - Bump seed used for PDA derivation (prevents collisions)
#[account]
pub struct Tournament {
    /// Public key of the tournament organizer/authority
    /// Only this key can make administrative changes to the tournament
    pub authority: Pubkey,
    
    /// Entry fee required to join the tournament
    /// Expressed in lamports (1 SOL = 1,000,000,000 lamports)
    pub entry_fee: u64,
    
    /// Total prize pool for the tournament
    /// Accumulated from player entry fees
    pub prize_pool: u64,
    
    /// Current status of the tournament
    pub status: TournamentStatus,
    
    /// Number of players currently registered
    /// Maximum is 4 for the current tournament format
    pub player_count: u8,
    
    /// Current round number (1-indexed)
    /// Round 1: Semi-finals, Round 2: Finals
    pub round: u8,
    
    /// Bump seed for PDA derivation
    /// Used to ensure deterministic address generation
    pub bump: u8,
}

/// Represents the current status of a tournament
/// 
/// Tournaments progress through these states in order:
/// Open → InProgress → Completed (or Cancelled)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum TournamentStatus {
    /// Tournament is open for player registration
    /// Players can join by paying the entry fee
    Open,
    
    /// Tournament has started and matches are in progress
    /// No new players can join
    InProgress,
    
    /// Tournament has completed successfully
    /// Winner has been determined and prizes distributed
    Completed,
    
    /// Tournament was cancelled by the authority
    /// Entry fees are refunded to players
    Cancelled,
}
```

### Tournament Match Account

The tournament match account stores information about a single match within a tournament, including the players and the winner.

```rust
/// Tournament match account representing a single match in a tournament
/// 
/// Each tournament consists of multiple matches (semi-finals and finals).
/// This account stores the match-specific state including participants and result.
/// 
/// # Fields
/// * `tournament` - Reference to the parent tournament account
/// * `player_white` - Public key of the player playing white pieces
/// * `player_black` - Public key of the player playing black pieces
/// * `winner` - Public key of the winning player (None if match not finished)
/// * `round` - Tournament round number (1 = semi-final, 2 = final)
/// * `match_index` - Index of the match within the round
/// * `bump` - Bump seed for PDA derivation
#[account]
pub struct TournamentMatch {
    /// Public key of the parent tournament account
    /// Used to verify the match belongs to the correct tournament
    pub tournament: Pubkey,
    
    /// Public key of the player playing white pieces
    /// White moves first in chess
    pub player_white: Pubkey,
    
    /// Public key of the player playing black pieces
    /// Black moves second in chess
    pub player_black: Pubkey,
    
    /// Public key of the winning player
    /// None if the match has not yet completed
    pub winner: Option<Pubkey>,
    
    /// Tournament round number
    /// Round 1: Semi-finals (2 matches)
    /// Round 2: Finals (1 match)
    pub round: u8,
    
    /// Index of this match within the current round
    /// Used to generate a unique PDA for each match
    pub match_index: u8,
    
    /// Bump seed for PDA derivation
    pub bump: u8,
}
```

## PDA (Program Derived Address) Derivation

PDAs are deterministic addresses derived from program seeds. They allow a program to "own" accounts without needing a private key. This is essential for organizing program state.

### Tournament PDA Seeds

```rust
use anchor_lang::prelude::*;

/// Returns the seeds used to derive a tournament PDA
/// 
/// PDAs are derived using the formula:
/// PDA = find_program_address(seeds, program_id)
/// 
/// For tournaments, we use the authority's public key as the seed,
/// ensuring each authority can only have one tournament.
/// 
/// # Arguments
/// * `authority` - The public key of the tournament authority
/// 
/// # Returns
/// A slice of byte slices representing the seeds
/// 
/// # Example
/// ```
/// let seeds = tournament_seeds(&authority);
/// let (pda, bump) = Pubkey::find_program_address(&seeds, &program_id);
/// ```
pub fn tournament_seeds(authority: &Pubkey) -> [&[u8]; 2] {
    [b"tournament", authority.as_ref()]
}

/// Finds the tournament PDA and its bump seed
/// 
/// This function calculates the deterministic address for a tournament account
/// based on the authority's public key. The bump seed is used to ensure
/// address uniqueness if multiple addresses could be derived from the same seeds.
/// 
/// # Arguments
/// * `authority` - The public key of the tournament authority
/// 
/// # Returns
/// A tuple containing:
/// - The derived PDA (public key)
/// - The bump seed (u8) used for the derivation
/// 
/// # Example
/// ```
/// let (tournament_pda, bump) = find_tournament_pda(&authority);
/// println!("Tournament PDA: {}", tournament_pda);
/// println!("Bump seed: {}", bump);
/// ```
pub fn find_tournament_pda(authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &tournament_seeds(authority),
        &crate::ID, // Program ID from declare_id! macro
    )
}
```

### Tournament Match PDA Seeds

```rust
/// Returns the seeds used to derive a tournament match PDA
/// 
/// Tournament matches are derived from:
/// - The tournament account (parent reference)
/// - The round number
/// - The match index within the round
/// 
/// This ensures each match has a unique address within its tournament.
/// 
/// # Arguments
/// * `tournament` - The public key of the parent tournament
/// * `round` - The tournament round number
/// * `match_index` - The index of the match within the round
/// 
/// # Returns
/// A slice of byte slices representing the seeds
pub fn tournament_match_seeds(
    tournament: &Pubkey,
    round: u8,
    match_index: u8,
) -> [&[u8]; 4] {
    [
        b"match",
        tournament.as_ref(),
        &round.to_le_bytes(),
        &match_index.to_le_bytes(),
    ]
}

/// Finds the tournament match PDA and its bump seed
/// 
/// # Arguments
/// * `tournament` - The public key of the parent tournament
/// * `round` - The tournament round number
/// * `match_index` - The index of the match within the round
/// 
/// # Returns
/// A tuple containing the derived PDA and bump seed
pub fn find_tournament_match_pda(
    tournament: &Pubkey,
    round: u8,
    match_index: u8,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &tournament_match_seeds(tournament, round, match_index),
        &crate::ID,
    )
}
```

## Space Calculation

Anchor automatically calculates the required space for accounts, but it's important to understand the formula:

```
Total Space = 8 (discriminator) + struct_size
```

The discriminator is a unique 8-byte identifier that Anchor uses to determine which account type is stored at a given address.

```rust
/// Calculates the required space for a tournament account
/// 
/// # Example
/// ```
/// let space = calculate_tournament_space();
/// assert_eq!(space, 8 + std::mem::size_of::<Tournament>());
/// ```
pub fn calculate_tournament_space() -> usize {
    8 // Account discriminator
        + std::mem::size_of::<Pubkey>() // authority
        + std::mem::size_of::<u64>()   // entry_fee
        + std::mem::size_of::<u64>()   // prize_pool
        + 1                            // status (enum)
        + std::mem::size_of::<u8>()    // player_count
        + std::mem::size_of::<u8>()    // round
        + std::mem::size_of::<u8>()    // bump
}
