# Move Instructions

Solana program instructions for chess moves, including move validation, execution, and history tracking.

## Overview

The moves instructions module handles chess move operations on-chain. It uses the shakmaty chess library for move validation and board state management. Moves are recorded with full history tracking for transparency and verification.

## Chess Move Validation

Move validation is performed using the shakmaty library, which implements full chess rules including:
- Legal move generation for all piece types
- Castling rules and restrictions
- En passant captures
- Pawn promotion
- Check and checkmate detection
- Stalemate detection

## Components

- Move validation against chess rules
- Move execution on-chain with board state updates
- Move history tracking in separate accounts
- Move log PDA derivation for each move

## Move Log Account

Each move creates a separate MoveLog account to maintain a complete history of the game:

```rust
use anchor_lang::prelude::*;

/// Move log account recording a single move in a game
#[account]
pub struct MoveLog {
    /// Reference to the parent game account
    pub game: Pubkey,
    
    /// Public key of the player who made the move
    pub player: Pubkey,
    
    /// Source square coordinates (file, rank)
    pub from: (u8, u8),
    
    /// Destination square coordinates (file, rank)
    pub to: (u8, u8),
    
    /// Piece type for pawn promotion (if applicable)
    pub promotion: Option<PieceType>,
    
    /// Unix timestamp when the move was made
    pub timestamp: i64,
    
    /// Index of this move in the game (0-indexed)
    pub move_index: u64,
    
    /// Bump seed for PDA derivation
    pub bump: u8,
}
```

## Example: Move Validation

This example shows how to validate a chess move before executing it on-chain.

```rust
use anchor_lang::prelude::*;
use shakmaty::{Board, ChessMove, Role, Square};

/// Context for validating a move
#[derive(Accounts)]
pub struct ValidateMove<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    
    pub player: Signer<'info>,
}

/// Validates a chess move against chess rules
/// 
/// This instruction checks if a proposed move is legal according to
/// standard chess rules without modifying the game state.
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// * `from` - Source square coordinates (file, rank)
/// * `to` - Destination square coordinates (file, rank)
/// * `promotion` - Piece type for pawn promotion (if applicable)
/// 
/// # Returns
/// true if the move is legal, false otherwise
/// 
/// # Errors
/// - `NotYourTurn` - Signer is not the current player
/// - `GameNotActive` - Game is not in active state
pub fn validate_move(
    ctx: Context<ValidateMove>,
    from: (u8, u8),
    to: (u8, u8),
    promotion: Option<Role>,
) -> Result<bool> {
    let game = &ctx.accounts.game;
    
    // Get the current player
    let current_player = match game.current_turn {
        PlayerColor::White => game.player_white,
        PlayerColor::Black => game.player_black,
    };
    
    // Validate it's the player's turn
    require!(
        ctx.accounts.player.key() == current_player,
        ErrorCode::NotYourTurn
    );
    
    // Validate game is active
    require!(
        game.status == GameStatus::Active,
        ErrorCode::GameNotActive
    );
    
    // Parse current board state from FEN string
    let board = Board::from_fen(&game.fen);
    
    // Create the chess move
    let chess_move = ChessMove::new(
        Square::from_coords(from.0, from.1),
        Square::from_coords(to.0, to.1),
        promotion,
    );
    
    // Check if move is in the legal moves list
    Ok(board.legal_moves().contains(&chess_move))
}
```

## Example: Recording a Move

This instruction executes a move on-chain and creates a move log entry for history tracking.

```rust
/// Context for recording a move
#[derive(Accounts)]
pub struct RecordMove<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    
    /// The move log account being created
    #[account(
        init,
        payer = player,
        seeds = [b"move_log", game.key().as_ref(), &game.move_count.to_le_bytes()],
        bump,
        space = 8 + std::mem::size_of::<MoveLog>()
    )]
    pub move_log: Account<'info, MoveLog>,
    
    #[account(mut)]
    pub player: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    
    pub clock: Sysvar<'info, Clock>,
}

/// Records and executes a chess move
/// 
/// This instruction:
/// 1. Validates the move is legal
/// 2. Creates a move log entry for history
/// 3. Updates the game board state
/// 4. Switches turns
/// 5. Increments the move counter
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
pub fn record_move(
    ctx: Context<RecordMove>,
    from: (u8, u8),
    to: (u8, u8),
    promotion: Option<PieceType>,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let move_log = &mut ctx.accounts.move_log;
    
    // Get the current player
    let current_player = match game.current_turn {
        PlayerColor::White => game.player_white,
        PlayerColor::Black => game.player_black,
    };
    
    // Validate it's the player's turn
    require!(
        ctx.accounts.player.key() == current_player,
        ErrorCode::NotYourTurn
    );
    
    // Validate game is active
    require!(
        game.status == GameStatus::Active,
        ErrorCode::GameNotActive
    );
    
    // Parse current board state from FEN string
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
    
    // Record move details in the move log
    move_log.game = ctx.accounts.game.key();
    move_log.player = ctx.accounts.player.key();
    move_log.from = from;
    move_log.to = to;
    move_log.promotion = promotion;
    move_log.timestamp = Clock::get()?.unix_timestamp;
    move_log.move_index = game.move_count;
    move_log.bump = ctx.bumps.move_log;
    
    // Execute the move and update the board state
    game.fen = board.make_move_new(chess_move).to_fen();
    
    // Switch turns
    game.current_turn = game.current_turn.opposite();
    
    // Increment move counter
    game.move_count += 1;
    
    msg!("Move recorded: {:?} -> {:?}", from, to);
    Ok(())
}
```

## Example: Move History Query

This example shows how to retrieve a specific move from the game history.

```rust
/// Context for querying move history
#[derive(Accounts)]
pub struct GetMoveHistory<'info> {
    pub game: Account<'info, Game>,
    
    /// CHECK: Move log PDA
    #[account(
        seeds = [b"move_log", game.key().as_ref(), &index.to_le_bytes()],
        bump
    )]
    pub move_log: Account<'info, MoveLog>,
}

/// Retrieves a specific move from the game history
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// * `index` - The move index to retrieve
/// 
/// # Returns
/// MoveLogData struct containing the move information
/// 
/// # Errors
/// - `MoveNotFound` - Move at the specified index doesn't exist
pub fn get_move_history(
    ctx: Context<GetMoveHistory>,
    index: u64,
) -> Result<MoveLogData> {
    let move_log = &ctx.accounts.move_log;
    
    // Validate the move belongs to the correct game
    require!(
        move_log.game == ctx.accounts.game.key(),
        ErrorCode::MoveNotFound
    );
    
    Ok(MoveLogData {
        from: move_log.from,
        to: move_log.to,
        promotion: move_log.promotion,
        player: move_log.player,
        timestamp: move_log.timestamp,
        move_index: move_log.move_index,
    })
}

/// Simplified move data structure for client consumption
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MoveLogData {
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub promotion: Option<PieceType>,
    pub player: Pubkey,
    pub timestamp: i64,
    pub move_index: u64,
}
```
