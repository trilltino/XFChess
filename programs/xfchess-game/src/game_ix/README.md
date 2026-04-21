# Game Instructions

Solana program instructions for game operations, including game creation, move validation, and game completion.

## Overview

The game instructions module handles all operations related to individual chess games stored on-chain. Each instruction modifies game state while validating that the operation is legal according to chess rules and game rules.

## Game Lifecycle

A chess game on-chain follows this lifecycle:

1. **Creation** - A player creates a game account as a PDA
2. **Waiting for Opponent** - Game waits for another player to join
3. **Active** - Both players have joined and moves can be made
4. **Completed** - Game ends with a winner or draw
5. **Cancelled** - Game is cancelled before completion

## Components

- Game creation and initialization
- Game state updates
- Move validation and execution
- Game completion and cancellation

## Game Account Structure

The game account stores all information about an active chess game:

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
    
    /// Winner of the game (if completed)
    pub winner: Option<PlayerColor>,
    
    /// Reason for game end (if completed)
    pub end_reason: Option<GameEndReason>,
    
    /// Unix timestamp when game was created
    pub created_at: i64,
    
    /// Unix timestamp when game ended (if completed)
    pub ended_at: Option<i64>,
    
    /// Bump seed for PDA derivation
    pub bump: u8,
}
```

## Example: Creating a Game

This instruction creates a new game account as a PDA derived from the creator's public key.

```rust
use anchor_lang::prelude::*;

/// Context for creating a new game
#[derive(Accounts)]
pub struct CreateGame<'info> {
    /// The player creating the game
    #[account(mut)]
    pub player: Signer<'info>,
    
    /// The game account being created
    #[account(
        init,
        payer = player,
        seeds = [b"game", player.key().as_ref()],
        bump,
        space = 8 + std::mem::size_of::<Game>()
    )]
    pub game: Account<'info, Game>,
    
    pub system_program: Program<'info, System>,
}

/// Creates a new chess game
/// 
/// This instruction initializes a game account with the creating player
/// as the white player. The game starts in "WaitingForOpponent" status
/// until another player joins.
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// 
/// # Errors
/// - Can fail if the player cannot pay the rent exemption
pub fn create_game(ctx: Context<CreateGame>) -> Result<()> {
    let game = &mut ctx.accounts.game;
    
    // Set the creating player as white
    game.player_white = ctx.accounts.player.key();
    
    // Black player will be set when opponent joins
    game.player_black = Pubkey::default();
    
    // Game starts waiting for opponent
    game.status = GameStatus::WaitingForOpponent;
    
    // White always moves first
    game.current_turn = PlayerColor::White;
    
    // Initialize with standard starting position
    game.fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string();
    
    // No moves made yet
    game.move_count = 0;
    
    // No winner yet
    game.winner = None;
    game.end_reason = None;
    game.ended_at = None;
    
    // Record creation time
    game.created_at = Clock::get()?.unix_timestamp;
    
    // Store bump seed
    game.bump = ctx.bumps.game;
    
    msg!("Game created by {}", ctx.accounts.player.key());
    Ok(())
}
```

## Example: Making a Move

This instruction validates and executes a chess move, ensuring it's legal according to chess rules.

```rust
/// Context for making a move in a game
#[derive(Accounts)]
pub struct MakeMove<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    
    #[account(mut)]
    pub player: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

/// Makes a move in the game
/// 
/// This instruction:
/// 1. Validates it's the player's turn
/// 2. Validates the game is active
/// 3. Validates the move is legal according to chess rules
/// 4. Updates the board state
/// 5. Switches turns
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// * `from` - Source square coordinates (file, rank)
/// * `to` - Destination square coordinates (file, rank)
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

## Example: Ending a Game

This instruction records the end of a game with the winner and reason.

```rust
/// Context for ending a game
#[derive(Accounts)]
pub struct EndGame<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    
    pub authority: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

/// Ends a game with a result
/// 
/// This instruction:
/// 1. Validates the game is active
/// 2. Records the winner
/// 3. Records the end reason
/// 4. Sets the game status to completed
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// * `winner` - The winning player color
/// * `reason` - The reason the game ended
/// 
/// # Errors
/// - `GameNotActive` - Game is not in active state
/// - `Unauthorized` - Signer is not authorized to end the game
pub fn end_game(
    ctx: Context<EndGame>,
    winner: PlayerColor,
    reason: GameEndReason,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    
    // Validate game is active
    require!(
        game.status == GameStatus::Active,
        ErrorCode::GameNotActive
    );
    
    // Validate authority (either player can end the game)
    require!(
        ctx.accounts.authority.key() == game.player_white
            || ctx.accounts.authority.key() == game.player_black,
        ErrorCode::Unauthorized
    );
    
    // Update game state
    game.status = GameStatus::Completed;
    game.winner = Some(winner);
    game.end_reason = Some(reason);
    game.ended_at = Clock::get()?.unix_timestamp;
    
    msg!("Game ended. Winner: {:?}, Reason: {:?}", winner, reason);
    Ok(())
}
```
