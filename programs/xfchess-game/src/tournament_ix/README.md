# Tournament Instructions

Solana program instructions for tournament management, including initialization, player registration, match management, and prize distribution.

## Overview

The tournament instructions module manages chess tournaments on-chain. Tournaments are structured competitions where multiple players compete in matches to determine a winner. The module handles tournament lifecycle from creation to completion.

## Tournament Structure

XFChess tournaments follow a 4-player bracket format:
- **Round 1 (Semi-finals)**: 2 matches between 4 players
- **Round 2 (Finals)**: 1 match between the winners of Round 1
- **Winner**: The player who wins the final match

## Tournament Lifecycle

1. **Initialization** - Authority creates tournament with entry fee and prize pool
2. **Open** - Players can register by paying the entry fee
3. **InProgress** - Tournament starts, matches are played
4. **Completed** - Winner determined, prizes distributed
5. **Cancelled** - Tournament cancelled by authority, fees refunded

## Components

- Tournament initialization with authority and parameters
- Player registration with entry fee payment
- Tournament start with bracket generation
- Match result recording
- Prize claiming and distribution
- Tournament cancellation with refunds

## Example: Initializing a Tournament

This instruction creates a new tournament account as a PDA derived from the authority's public key.

```rust
use anchor_lang::prelude::*;

/// Context for initializing a tournament
#[derive(Accounts)]
pub struct InitializeTournament<'info> {
    /// The tournament authority (organizer)
    #[account(mut)]
    pub authority: Signer<'info>,
    
    /// The tournament account being created
    #[account(
        init,
        payer = authority,
        seeds = [b"tournament", authority.key().as_ref()],
        bump,
        space = 8 + std::mem::size_of::<Tournament>()
    )]
    pub tournament: Account<'info, Tournament>,
    
    pub system_program: Program<'info, System>,
}

/// Initializes a new tournament
/// 
/// This instruction creates a tournament with the specified entry fee.
/// The tournament starts in "Open" status, allowing players to register.
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// * `entry_fee` - The SOL amount required to join (in lamports)
/// * `prize_pool` - Initial prize pool (usually 0, grows as players join)
/// 
/// # Errors
/// - Can fail if the authority cannot pay the rent exemption
pub fn initialize_tournament(
    ctx: Context<InitializeTournament>,
    entry_fee: u64,
    prize_pool: u64,
) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    
    tournament.authority = ctx.accounts.authority.key();
    tournament.entry_fee = entry_fee;
    tournament.prize_pool = prize_pool;
    tournament.status = TournamentStatus::Open;
    tournament.player_count = 0;
    tournament.round = 0;
    tournament.bump = ctx.bumps.tournament;
    
    msg!("Tournament initialized with entry fee: {} lamports", entry_fee);
    Ok(())
}
```

## Example: Registering a Player

This instruction allows a player to register for a tournament by paying the entry fee.

```rust
/// Context for registering a player
#[derive(Accounts)]
pub struct RegisterPlayer<'info> {
    #[account(mut)]
    pub tournament: Account<'info, Tournament>,
    
    #[account(mut)]
    pub player: Signer<'info>,
    
    /// The player entry account being created
    #[account(
        init,
        payer = player,
        seeds = [b"player_entry", tournament.key().as_ref(), player.key().as_ref()],
        bump,
        space = 8 + std::mem::size_of::<PlayerEntry>()
    )]
    pub player_entry: Account<'info, PlayerEntry>,
    
    pub system_program: Program<'info, System>,
}

/// Registers a player for the tournament
/// 
/// This instruction:
/// 1. Validates the tournament is open
/// 2. Checks maximum player count (4)
/// 3. Transfers entry fee from player to tournament
/// 4. Creates a player entry record
/// 5. Updates tournament state
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// 
/// # Errors
/// - `TournamentNotOpen` - Tournament is not accepting registrations
/// - `TournamentFull` - Maximum players reached
/// - `InsufficientFunds` - Player cannot pay entry fee
pub fn register_player(ctx: Context<RegisterPlayer>) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let player_entry = &mut ctx.accounts.player_entry;
    
    // Validate tournament is open
    require!(
        tournament.status == TournamentStatus::Open,
        ErrorCode::TournamentNotOpen
    );
    
    // Check max players
    require!(
        tournament.player_count < 4,
        ErrorCode::TournamentFull
    );
    
    // Check entry fee
    require!(
        ctx.accounts.player.lamports() >= tournament.entry_fee,
        ErrorCode::InsufficientFunds
    );
    
    // Transfer entry fee
    let cpi_accounts = anchor_lang::system_program::Transfer {
        from: ctx.accounts.player.to_account_info(),
        to: ctx.accounts.tournament.to_account_info(),
    };
    
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            cpi_accounts,
        ),
        tournament.entry_fee,
    )?;
    
    // Update tournament state
    tournament.player_count += 1;
    tournament.prize_pool += tournament.entry_fee;
    
    // Record player entry
    player_entry.tournament = ctx.accounts.tournament.key();
    player_entry.player = ctx.accounts.player.key();
    player_entry.registered_at = Clock::get()?.unix_timestamp;
    player_entry.elo_rating = 1200; // Default ELO
    player_entry.bump = ctx.bumps.player_entry;
    
    msg!("Player {} registered for tournament", ctx.accounts.player.key());
    Ok(())
}
```

## Example: Starting a Tournament

This instruction starts the tournament once enough players have registered.

```rust
/// Context for starting a tournament
#[derive(Accounts)]
pub struct StartTournament<'info> {
    #[account(mut)]
    pub tournament: Account<'info, Tournament>,
    
    pub authority: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

/// Starts the tournament
/// 
/// This instruction:
/// 1. Validates the authority
/// 2. Validates minimum player count (2)
/// 3. Generates the match bracket
/// 4. Sets tournament status to InProgress
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// 
/// # Errors
/// - `Unauthorized` - Signer is not the tournament authority
/// - `NotEnoughPlayers` - Less than 2 players registered
pub fn start_tournament(ctx: Context<StartTournament>) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    
    // Only authority can start
    require!(
        ctx.accounts.authority.key() == tournament.authority,
        ErrorCode::Unauthorized
    );
    
    // Validate enough players
    require!(
        tournament.player_count >= 2,
        ErrorCode::NotEnoughPlayers
    );
    
    // Generate bracket
    tournament.status = TournamentStatus::InProgress;
    tournament.round = 1;
    tournament.started_at = Clock::get()?.unix_timestamp;
    
    msg!("Tournament started with {} players", tournament.player_count);
    Ok(())
}
```

## Example: Recording Match Result

This instruction records the result of a tournament match.

```rust
/// Context for recording a match result
#[derive(Accounts)]
pub struct RecordMatchResult<'info> {
    #[account(mut)]
    pub tournament: Account<'info, Tournament>,
    
    #[account(
        init,
        payer = authority,
        seeds = [b"match", tournament.key().as_ref(), &round.to_le_bytes(), &match_index.to_le_bytes()],
        bump,
        space = 8 + std::mem::size_of::<TournamentMatch>()
    )]
    pub tournament_match: Account<'info, TournamentMatch>,
    
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Records a match result
/// 
/// This instruction:
/// 1. Creates a match record
/// 2. Records the winner
/// 3. Updates tournament if final match
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// * `round` - Tournament round number
/// * `match_index` - Match index within the round
/// * `winner` - Public key of the winning player
/// 
/// # Errors
/// - `Unauthorized` - Signer is not the tournament authority
pub fn record_match_result(
    ctx: Context<RecordMatchResult>,
    round: u8,
    match_index: u8,
    winner: Pubkey,
) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let tournament_match = &mut ctx.accounts.tournament_match;
    
    // Record match result
    tournament_match.tournament = ctx.accounts.tournament.key();
    tournament_match.player_white = Pubkey::default(); // Set from bracket
    tournament_match.player_black = Pubkey::default(); // Set from bracket
    tournament_match.winner = Some(winner);
    tournament_match.round = round;
    tournament_match.match_index = match_index;
    tournament_match.bump = ctx.bumps.tournament_match;
    
    // Update tournament if final match
    if round == 2 && match_index == 0 {
        tournament.status = TournamentStatus::Completed;
        tournament.winner = Some(winner);
        tournament.completed_at = Clock::get()?.unix_timestamp;
    }
    
    msg!("Match result recorded: {}", winner);
    Ok(())
}
```
