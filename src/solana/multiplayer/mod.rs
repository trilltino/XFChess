use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::system_instruction;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar},
};

use crate::solana::{errors::XfChessError, state::GameStatus};

// Seeds for PDAs
const GAME_SEED: &[u8] = b"game";
const PLAYER_SEED: &[u8] = b"player";

// BOLT ECS Components - stubs
#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Component))]
pub struct BoardState {
    pub positions: Vec<(u8, u8, PieceType)>, // x, y, piece type
    pub fen: String,                         // Current FEN representation
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "bevy", derive(Reflect))]
pub enum PieceType {
    Pawn,
    Rook,
    Knight,
    Bishop,
    Queen,
    King,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Component))]
pub struct GameSession {
    pub players: [Option<Pubkey>; 2],
    pub current_turn: u8,
    pub game_status: GameStatus,
    pub game_result: crate::solana::state::GameResult,

    pub move_history: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub bet_amount: u64,
}

// System stub
pub struct ProcessMoveSystem;

impl ProcessMoveSystem {
    pub fn execute(
        game_session: &mut GameSession,
        board_state: &mut BoardState,
        move_params: MoveParams,
    ) -> ProgramResult {
        // Validate the move against the current board state
        if !Self::validate_chess_move(board_state, &move_params.move_data) {
            return Err(XfChessError::InvalidMove.into());
        }

        // Apply the move to the board state
        Self::apply_move(board_state, &move_params.move_data)?;

        // Add move to history
        game_session.move_history.push(move_params.move_data);

        // Switch turns
        game_session.current_turn = (game_session.current_turn + 1) % 2;

        // Update game status if needed (checkmate, stalemate, etc.)

        Ok(())
    }

    fn validate_chess_move(board_state: &BoardState, move_data: &str) -> bool {
        // Implement proper chess move validation logic
        // This is a simplified placeholder
        // In a real implementation, we would check:
        // - if the piece exists at the source position
        // - if the move is valid for that piece type
        // - if the destination is empty or contains an opponent's piece
        // - if the move doesn't put own king in check
        // - special moves like castling, en passant, pawn promotion
        move_data.len() == 4 || move_data.len() == 5 // Basic UCI format check
    }

    fn apply_move(board_state: &mut BoardState, move_data: &str) -> ProgramResult {
        // Parse move_data in UCI format (e.g., "e2e4") and update the board state
        // This is a simplified implementation - a full implementation would require
        // detailed chess logic to update positions correctly
        msg!("Applying move: {}", move_data);
        Ok(())
    }
}

pub struct MultiplayerGame;

impl MultiplayerGame {
    /// Initialize a new multiplayer game using ECS pattern
    pub fn initialize_game(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        params: InitializeGameParams,
    ) -> ProgramResult {
        // Verify accounts
        let [game_account, player_account, system_program] = accounts else {
            return Err(solana_program::program_error::ProgramError::NotEnoughAccountKeys);
        };

        // Check if game account is already initialized
        if !game_account.data_is_empty() {
            return Err(XfChessError::GameFull.into());
        }

        // Get clock to set timestamps
        let clock = Clock::get()?;

        // Create ECS entities: BoardState and GameSession components
        let board_state = BoardState {
            positions: Self::initial_board_setup(),
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
        };

        let game_session = GameSession {
            players: [Some(*player_account.key), None],
            current_turn: 0,
            game_status: GameStatus::Active, // Placeholder if WaitingForPlayers is missing
            game_result: crate::solana::state::GameResult::None, // Initialize game_result

            move_history: vec![],
            created_at: clock.unix_timestamp,
            updated_at: clock.unix_timestamp,
            bet_amount: params.bet_amount,
        };

        // Serialize both components into the account
        let mut account_data = Vec::new();
        board_state
            .serialize(&mut account_data)
            .map_err(|_| ProgramError::AccountDataTooSmall)?;
        game_session
            .serialize(&mut account_data)
            .map_err(|_| ProgramError::AccountDataTooSmall)?;

        // Calculate space needed for account
        let space = account_data.len() + 1024; // Extra space for future moves
        let lamports = solana_program::rent::Rent::get()?.minimum_balance(space);

        // Create the account
        solana_program::program::invoke_signed(
            &system_instruction::create_account(
                player_account.key,
                game_account.key,
                lamports,
                space as u64,
                program_id,
            ),
            &[
                system_program.clone(),
                game_account.clone(),
                player_account.clone(),
            ],
            &[&[GAME_SEED, &params.game_id, &[params._bump]]], // Use constant for seed
        )?;

        // Save the serialized data to the account
        let mut account_data_ref = game_account.try_borrow_mut_data()?;
        account_data_ref[..account_data.len()].copy_from_slice(&account_data);

        msg!(
            "Initialized multiplayer game with ECS components, ID: {:?}",
            params.game_id
        );
        Ok(())
    }

    /// Process a move in a multiplayer game using the ProcessMoveSystem
    pub fn process_move(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        params: MoveParams,
    ) -> ProgramResult {
        // Verify accounts
        let [game_account, player_account] = accounts else {
            return Err(solana_program::program_error::ProgramError::NotEnoughAccountKeys);
        };

        // Deserialize ECS components from account data
        let account_data = game_account.try_borrow_data()?;
        let mut data_slice = &account_data[..];

        // Deserialize board state
        let mut board_state = BoardState::deserialize(&mut data_slice)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        // Deserialize game session
        let mut game_session = GameSession::deserialize(&mut data_slice)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        // Verify game status
        if matches!(
            game_session.game_status,
            GameStatus::Finished | GameStatus::Expired
        ) {
            return Err(XfChessError::GameEnded.into());
        }

        // Verify it's the player's turn
        if !Self::is_players_turn(&game_session, player_account.key) {
            return Err(XfChessError::NotPlayersTurn.into());
        }

        // Verify the player is part of this game
        if !Self::is_player_in_game(&game_session, player_account.key) {
            return Err(XfChessError::UnauthorizedPlayer.into());
        }

        // Execute the move through the ECS system
        ProcessMoveSystem::execute(&mut game_session, &mut board_state, params)?;

        // Update timestamp
        let clock = Clock::get()?;
        game_session.updated_at = clock.unix_timestamp;

        // Re-serialize components back to account data
        let mut new_account_data = Vec::new();
        board_state
            .serialize(&mut new_account_data)
            .map_err(|_| ProgramError::AccountDataTooSmall)?;
        game_session
            .serialize(&mut new_account_data)
            .map_err(|_| ProgramError::AccountDataTooSmall)?;

        let mut account_data_ref = game_account.try_borrow_mut_data()?;
        account_data_ref[..new_account_data.len()].copy_from_slice(&new_account_data);

        msg!("Processed move for player: {:?}", player_account.key);
        Ok(())
    }

    /// Join an existing multiplayer game
    pub fn join_game(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        _params: JoinGameParams,
    ) -> ProgramResult {
        let [game_account, player_account] = accounts else {
            return Err(solana_program::program_error::ProgramError::NotEnoughAccountKeys);
        };

        // Deserialize ECS components from account data
        let account_data = game_account.try_borrow_data()?;
        let mut data_slice = &account_data[..];

        // Deserialize board state
        let _board_state = BoardState::deserialize(&mut data_slice)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        // Deserialize game session
        let mut game_session = GameSession::deserialize(&mut data_slice)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        // Verify game can accept another player
        if game_session.players[1].is_some() {
            return Err(XfChessError::GameFull.into());
        }

        // Add player to game
        game_session.players[1] = Some(*player_account.key);
        game_session.game_status = GameStatus::Active;

        // Update timestamp
        let clock = Clock::get()?;
        game_session.updated_at = clock.unix_timestamp;

        // Re-serialize components back to account data
        let mut new_account_data = Vec::new();
        _board_state
            .serialize(&mut new_account_data)
            .map_err(|_| ProgramError::AccountDataTooSmall)?;
        game_session
            .serialize(&mut new_account_data)
            .map_err(|_| ProgramError::AccountDataTooSmall)?;

        let mut account_data_ref = game_account.try_borrow_mut_data()?;
        account_data_ref[..new_account_data.len()].copy_from_slice(&new_account_data);

        msg!("Player joined game: {:?}", player_account.key);
        Ok(())
    }

    /// Complete a game and determine the winner
    pub fn complete_game(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        winner: Option<u8>, // 0 for player 1, 1 for player 2, None for draw
    ) -> ProgramResult {
        let [game_account, _player_account] = accounts else {
            return Err(solana_program::program_error::ProgramError::NotEnoughAccountKeys);
        };

        // Deserialize ECS components from account data
        let account_data = game_account.try_borrow_data()?;
        let mut data_slice = &account_data[..];

        // Deserialize board state
        let _board_state = BoardState::deserialize(&mut data_slice)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        // Deserialize game session
        let mut game_session = GameSession::deserialize(&mut data_slice)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        // Set game status based on winner
        match winner {
            Some(0) => {
                game_session.game_status = GameStatus::Finished;
                // Note: GameResult needs a Pubkey for Winner, this is a placeholder
            }
            Some(1) => {
                game_session.game_status = GameStatus::Finished;
            }
            None => {
                game_session.game_status = GameStatus::Finished;
            }
            _ => return Err(ProgramError::InvalidArgument),
        }

        // Update timestamp
        let clock = Clock::get()?;
        game_session.updated_at = clock.unix_timestamp;

        // Re-serialize components back to account data
        let mut new_account_data = Vec::new();
        _board_state
            .serialize(&mut new_account_data)
            .map_err(|_| ProgramError::AccountDataTooSmall)?;
        game_session
            .serialize(&mut new_account_data)
            .map_err(|_| ProgramError::AccountDataTooSmall)?;

        let mut account_data_ref = game_account.try_borrow_mut_data()?;
        account_data_ref[..new_account_data.len()].copy_from_slice(&new_account_data);

        msg!("Game completed with result: {:?}", game_session.game_status);
        Ok(())
    }

    /// Helper function to check if it's a player's turn
    fn is_players_turn(game_session: &GameSession, player_key: &Pubkey) -> bool {
        if let Some(player) = game_session.players[game_session.current_turn as usize] {
            &player == player_key
        } else {
            false
        }
    }

    /// Helper function to check if a player is in the game
    fn is_player_in_game(game_session: &GameSession, player_key: &Pubkey) -> bool {
        game_session.players.contains(&Some(*player_key))
    }

    /// Setup initial board positions
    fn initial_board_setup() -> Vec<(u8, u8, PieceType)> {
        let mut positions = Vec::new();

        // Add pawns
        for col in 0..8 {
            positions.push((1, col, PieceType::Pawn)); // Black pawns
            positions.push((6, col, PieceType::Pawn)); // White pawns
        }

        // Add other pieces
        positions.push((0, 0, PieceType::Rook));
        positions.push((0, 1, PieceType::Knight));
        positions.push((0, 2, PieceType::Bishop));
        positions.push((0, 3, PieceType::Queen));
        positions.push((0, 4, PieceType::King));
        positions.push((0, 5, PieceType::Bishop));
        positions.push((0, 6, PieceType::Knight));
        positions.push((0, 7, PieceType::Rook));

        positions.push((7, 0, PieceType::Rook));
        positions.push((7, 1, PieceType::Knight));
        positions.push((7, 2, PieceType::Bishop));
        positions.push((7, 3, PieceType::Queen));
        positions.push((7, 4, PieceType::King));
        positions.push((7, 5, PieceType::Bishop));
        positions.push((7, 6, PieceType::Knight));
        positions.push((7, 7, PieceType::Rook));

        positions
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct InitializeGameParams {
    pub bet_amount: u64,
    pub game_id: [u8; 32], // Unique identifier for the game
    pub _bump: u8,         // PDA bump seed
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct MoveParams {
    pub move_data: String,   // Encoded move data (e.g., "e2e4")
    pub signature: [u8; 64], // Signature for optimistic sync
    pub block_number: u64,   // For Bolt/Ephemeral Rollups consistency
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct JoinGameParams {
    pub game_id: [u8; 32],
    pub _bump: u8, // PDA bump seed
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::{clock::Clock, pubkey::Pubkey, rent::Rent};
    use std::str::FromStr;

    // Mock implementations for testing
    pub fn create_mock_clock() -> Clock {
        Clock {
            slot: 1,
            epoch_start_timestamp: 0,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: 1234567890,
        }
    }

    pub fn create_mock_rent() -> Rent {
        Rent {
            lamports_per_byte_year: 1,
            exemption_threshold: 1.0,
            burn_percent: 1,
        }
    }

    fn create_mock_pubkey(seed: &str) -> Pubkey {
        Pubkey::from_str(&format!("{}1111111111111111111111111111111", seed)).unwrap()
    }

    #[test]
    fn test_validate_chess_move() {
        // Create a basic board state
        let board_state = BoardState {
            positions: MultiplayerGame::initial_board_setup(),
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
        };

        // Valid moves in UCI format
        assert!(ProcessMoveSystem::validate_chess_move(&board_state, "e2e4"));
        assert!(ProcessMoveSystem::validate_chess_move(&board_state, "g1f3"));

        // Invalid moves
        assert!(!ProcessMoveSystem::validate_chess_move(&board_state, "")); // Empty
        assert!(!ProcessMoveSystem::validate_chess_move(&board_state, "abc")); // Invalid format
    }

    #[test]
    fn test_process_move_system_execute() {
        let mut board_state = BoardState {
            positions: MultiplayerGame::initial_board_setup(),
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
        };

        let mut game_session = GameSession {
            players: [Some(create_mock_pubkey("1")), Some(create_mock_pubkey("2"))],
            current_turn: 0,
            game_status: GameStatus::Active,
            move_history: vec![],
            created_at: 0,
            updated_at: 0,
            bet_amount: 0,
        };

        let move_params = MoveParams {
            move_data: "e2e4".to_string(),
            signature: [0; 64],
            block_number: 0,
        };

        // Execute a valid move
        let result = ProcessMoveSystem::execute(&mut game_session, &mut board_state, move_params);
        assert!(result.is_ok());

        // Verify the move was recorded and turn changed
        assert_eq!(game_session.move_history, vec!["e2e4"]);
        assert_eq!(game_session.current_turn, 1);
    }

    #[test]
    fn test_initial_board_setup() {
        let positions = MultiplayerGame::initial_board_setup();
        assert_eq!(positions.len(), 16 + 8 + 8); // 16 pawns + 8 white back rank + 8 black back rank

        // Check first few pieces
        assert_eq!(positions[0], (1, 0, PieceType::Pawn)); // Black pawn
        assert_eq!(positions[8], (0, 0, PieceType::Rook)); // Black rook
        assert_eq!(positions[16], (6, 0, PieceType::Pawn)); // White pawn
        assert_eq!(positions[24], (7, 0, PieceType::Rook)); // White rook
    }
}

// Add UI module
pub mod ui;
