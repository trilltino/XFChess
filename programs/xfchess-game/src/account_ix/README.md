# Account Instructions

Solana program instructions for account management, including account creation, initialization, and validation.

## Overview of Anchor Account Instructions

In Anchor, instructions are functions that modify program state. Each instruction takes a `Context` struct that defines the accounts required for the instruction. The `#[derive(Accounts)]` macro automatically handles account validation, ensuring:
- Accounts are properly initialized
- Signers have signed the transaction
- PDAs have the correct seeds and bump
- Accounts are owned by the correct program

## Components

- Account creation and initialization
- Account validation and checks
- Account constraints and relationships

## Instruction Context Pattern

Every Anchor instruction follows this pattern:

```rust
pub fn instruction_name(ctx: Context<AccountStruct>, params: ...) -> Result<()>
```

Where:
- `Context<AccountStruct>` - Contains all the accounts for the instruction
- `params` - Additional instruction-specific parameters
- Returns `Result<()>` - Success or error

## Example: Creating a Tournament Account

This instruction creates a new tournament account as a PDA derived from the authority's public key.

```rust
use anchor_lang::prelude::*;

/// Context for initializing a tournament account
/// 
/// This struct defines all accounts required to create a new tournament.
/// The `#[derive(Accounts)]` macro generates validation code.
#[derive(Accounts)]
pub struct InitializeTournament<'info> {
    /// The authority (organizer) who will own the tournament
    /// 
    /// - `mut` - This account's data can be modified
    /// - `Signer` - This account must have signed the transaction
    #[account(mut)]
    pub authority: Signer<'info>,
    
    /// The tournament account being created
    /// 
    /// Constraints:
    /// - `init` - Create a new account
    /// - `payer = authority` - Authority pays the rent exemption
    /// - `seeds` - PDA derivation seeds
    /// - `bump` - Store the bump seed in the account
    /// - `space` - Account size (8 bytes discriminator + struct size)
    #[account(
        init,
        payer = authority,
        seeds = [b"tournament", authority.key().as_ref()],
        bump,
        space = 8 + std::mem::size_of::<Tournament>()
    )]
    pub tournament: Account<'info, Tournament>,
    
    /// System program required for account creation
    /// 
    /// The system program is needed to:
    /// - Create new accounts
    /// - Transfer lamports for rent exemption
    pub system_program: Program<'info, System>,
}

/// Initializes a new tournament account
/// 
/// This instruction creates a tournament account with the specified entry fee.
/// The tournament starts in the "Open" status, allowing players to register.
/// 
/// # Arguments
/// * `ctx` - The instruction context containing all accounts
/// * `entry_fee` - The SOL amount required to join (in lamports)
/// 
/// # Errors
/// - Can fail if the authority cannot pay the rent exemption
/// 
/// # Example
/// ```ignore
/// let entry_fee = 1_000_000_000; // 1 SOL in lamports
/// initialize_tournament(ctx, entry_fee)?;
/// ```
pub fn initialize_tournament(
    ctx: Context<InitializeTournament>,
    entry_fee: u64,
) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    
    // Set the tournament authority (who can administer it)
    tournament.authority = ctx.accounts.authority.key();
    
    // Set the entry fee required to join
    tournament.entry_fee = entry_fee;
    
    // Initialize prize pool (starts at 0, grows as players join)
    tournament.prize_pool = 0;
    
    // Set initial status to Open (players can register)
    tournament.status = TournamentStatus::Open;
    
    // No players registered yet
    tournament.player_count = 0;
    
    // Tournament hasn't started yet
    tournament.round = 0;
    
    // Store the bump seed for PDA verification
    tournament.bump = ctx.bumps.tournament;
    
    msg!("Tournament initialized with entry fee: {} lamports", entry_fee);
    Ok(())
}
```

## Example: Account Validation

This example shows how to validate that a signer is authorized to perform an action and that the game is in the correct state.

```rust
use anchor_lang::prelude::*;

/// Game account representing an active chess game
#[account]
pub struct Game {
    /// Public key of the player playing white pieces
    pub player_white: Pubkey,
    
    /// Public key of the player playing black pieces
    pub player_black: Pubkey,
    
    /// Whose turn it is to move
    pub current_turn: PlayerColor,
    
    /// Current status of the game
    pub status: GameStatus,
    
    /// FEN string representing the board state
    pub fen: String,
    
    /// Number of moves made so far
    pub move_count: u64,
}

/// Context for making a move in a game
#[derive(Accounts)]
pub struct MakeMove<'info> {
    /// The game account being modified
    /// 
    /// `mut` is required because we're updating the game state
    #[account(mut)]
    pub game: Account<'info, Game>,
    
    /// The player making the move
    /// 
    /// Must be the signer and must be the current player
    #[account(mut)]
    pub player: Signer<'info>,
    
    /// Clock sysvar for timestamp
    pub clock: Sysvar<'info, Clock>,
}

/// Makes a move in the game
/// 
/// This instruction validates that:
/// 1. The signer is the current player
/// 2. The game is active (not completed or cancelled)
/// 3. The move is legal according to chess rules
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// * `from` - Source square (e.g., (4, 1) for e2)
/// * `to` - Destination square (e.g., (4, 3) for e4)
/// * `promotion` - Piece type for pawn promotion (if applicable)
/// 
/// # Errors
/// - `NotYourTurn` - Signer is not the current player
/// - `GameNotActive` - Game is not in active state
/// - `IllegalMove` - Move violates chess rules
pub fn make_move(
    ctx: Context<MakeMove>,
    from: (u8, u8),
    to: (u8, u8),
    promotion: Option<PieceType>,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    
    // Get the public key of the current player
    let current_player = match game.current_turn {
        PlayerColor::White => game.player_white,
        PlayerColor::Black => game.player_black,
    };
    
    // Validate that the signer is the current player
    require!(
        ctx.accounts.player.key() == current_player,
        ErrorCode::NotYourTurn
    );
    
    // Validate that the game is active
    require!(
        game.status == GameStatus::Active,
        ErrorCode::GameNotActive
    );
    
    // Parse the current board state from FEN string
    let board = Board::from_fen(&game.fen);
    
    // Create the chess move
    let chess_move = ChessMove::new(
        Square::from_coords(from.0, from.1),
        Square::from_coords(to.0, to.1),
        promotion,
    );
    
    // Validate the move is legal
    require!(
        board.legal_moves().contains(&chess_move),
        ErrorCode::IllegalMove
    );
    
    // Execute the move and update the board state
    game.fen = board.make_move_new(chess_move).to_fen();
    
    // Switch turns
    game.current_turn = game.current_turn.opposite();
    
    // Increment move counter
    game.move_count += 1;
    
    msg!("Move made: {:?} -> {:?}", from, to);
    Ok(())
}
```

## Common Account Constraints

Anchor provides many constraints for account validation:

### has_one
Ensures an account matches a field in another account:

```rust
#[derive(Accounts)]
pub struct UpdateTournament<'info> {
    #[account(
        mut,
        has_one = authority
    )]
    pub tournament: Account<'info, Tournament>,
    
    pub authority: Signer<'info>,
}
```

### constraint
Custom validation logic:

```rust
#[derive(Accounts)]
pub struct CustomValidation<'info> {
    #[account(
        constraint = account.owner == program_id @ ErrorCode::InvalidOwner
    )]
    pub account: Account<'info, CustomAccount>,
}
```

### close
Close an account and return lamports to a recipient:

```rust
#[derive(Accounts)]
pub struct CloseAccount<'info> {
    #[account(
        mut,
        close = recipient
    )]
    pub account: Account<'info, AccountToClose>,
    
    /// CHECK: Recipient account
    #[account(mut)]
    pub recipient: UncheckedAccount<'info>,
}
```

## Error Handling

Define custom error codes:

```rust
#[error_code]
pub enum ErrorCode {
    #[msg("It is not your turn to move")]
    NotYourTurn,
    
    #[msg("Game is not currently active")]
    GameNotActive,
    
    #[msg("The move is illegal according to chess rules")]
    IllegalMove,
    
    #[msg("You are not authorized to perform this action")]
    Unauthorized,
}
```
