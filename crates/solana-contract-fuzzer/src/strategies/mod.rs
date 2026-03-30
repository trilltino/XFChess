//! Proptest strategies for fuzzing
//!
//! Generates random but valid instruction sequences for testing.

use proptest::prelude::*;
use proptest::strategy::BoxedStrategy;
use solana_sdk::{instruction::{AccountMeta, Instruction}, pubkey::Pubkey, system_program};
use xfchess_game::state::{GameType, GameResult};

/// A fuzz instruction to execute
#[derive(Debug, Clone)]
pub enum FuzzInstruction {
    CreateGame {
        game_id: u64,
        wager: u64,
        game_type: GameType,
        player_idx: usize,
    },
    JoinGame {
        game_id: u64,
        player_idx: usize,
    },
    RecordMove {
        game_id: u64,
        move_str: String,
        fen: String,
        player_idx: usize,
    },
    FinalizeGame {
        game_id: u64,
        result: u8, // 0=WhiteWins, 1=BlackWins, 2=Draw
        player_idx: usize,
    },
    WithdrawExpired {
        game_id: u64,
        player_idx: usize,
    },
    AuthorizeSession {
        game_id: u64,
        session_pubkey: Pubkey,
        player_idx: usize,
    },
    DelegateGame {
        game_id: u64,
        player_idx: usize,
    },
    UndelegateGame {
        game_id: u64,
        player_idx: usize,
    },
}

/// Strategy for generating game IDs
fn game_id_strategy() -> BoxedStrategy<u64> {
    // Use a mix of small sequential IDs and random large ones
    prop_oneof![
        80 => 1u64..10_000u64, // Most games use small IDs
        20 => any::<u64>(), // Some use random large IDs
    ].boxed()
}

/// Strategy for wager amounts (in lamports)
fn wager_strategy() -> BoxedStrategy<u64> {
    // Minimum wager: 0.001 SOL (1_000_000 lamports)
    // Maximum: 0.1 SOL (100_000_000 lamports)
    prop_oneof![
        30 => Just(1_000_000u64),      // No wager / minimal
        50 => 1_000_000u64..10_000_000u64, // 0.001-0.01 SOL
        20 => 10_000_000u64..100_000_000u64, // 0.01-0.1 SOL
    ].boxed()
}

/// Strategy for game types
fn game_type_strategy() -> BoxedStrategy<GameType> {
    prop_oneof![
        70 => Just(GameType::PvP),
        30 => Just(GameType::PvAI),
    ].boxed()
}

/// Strategy for player indices (0-9 for 10 test accounts)
fn player_idx_strategy(max_players: usize) -> BoxedStrategy<usize> {
    (0..max_players).boxed()
}

/// Generate a single fuzz instruction
pub fn instruction_strategy(max_players: usize) -> impl Strategy<Value = FuzzInstruction> {
    let player = player_idx_strategy(max_players);
    let game_id = game_id_strategy();
    let wager = wager_strategy();
    let game_type = game_type_strategy();

    (game_id, wager, game_type, player)
        .prop_flat_map(move |(id, w, gt, p)| {
            prop_oneof![
                // Create game (20% weight)
                20 => Just(FuzzInstruction::CreateGame {
                    game_id: id,
                    wager: w,
                    game_type: gt,
                    player_idx: p,
                }),
                // Join game (15% weight)
                15 => Just(FuzzInstruction::JoinGame {
                    game_id: id,
                    player_idx: p,
                }),
                // Record move (40% weight)
                40 => (valid_move_strategy(), valid_fen_strategy())
                    .prop_map(move |(mv, fen)| FuzzInstruction::RecordMove {
                        game_id: id,
                        move_str: mv,
                        fen,
                        player_idx: p,
                    }),
                // Finalize game (10% weight)
                10 => (0u8..3u8)
                    .prop_map(move |res| FuzzInstruction::FinalizeGame {
                        game_id: id,
                        result: res,
                        player_idx: p,
                    }),
                // Withdraw (5% weight)
                5 => Just(FuzzInstruction::WithdrawExpired {
                    game_id: id,
                    player_idx: p,
                }),
                // Session delegation (5% weight)
                5 => any::<[u8; 32]>()
                    .prop_map(move |pk_bytes| FuzzInstruction::AuthorizeSession {
                        game_id: id,
                        session_pubkey: Pubkey::new_from_array(pk_bytes),
                        player_idx: p,
                    }),
                // Delegate/Undelegate (5% weight)
                5 => Just(FuzzInstruction::DelegateGame {
                    game_id: id,
                    player_idx: p,
                }),
            ]
        })
}

/// Generate a sequence of instructions
pub fn instruction_sequence_strategy(
    max_len: usize,
    max_players: usize,
) -> impl Strategy<Value = Vec<FuzzInstruction>> {
    prop::collection::vec(instruction_strategy(max_players), 1..max_len)
}

/// Valid chess move strings (UCI format)
fn valid_move_strategy() -> impl Strategy<Value = String> {
    // Common opening moves
    let common = prop_oneof![
        Just("e2e4".to_string()),
        Just("d2d4".to_string()),
        Just("e7e5".to_string()),
        Just("d7d5".to_string()),
        Just("g1f3".to_string()),
        Just("b1c3".to_string()),
    ];
    
    // Random valid-looking UCI
    let random = (b'a'..b'i', b'1'..b'9', b'a'..b'i', b'1'..b'9')
        .prop_map(|(f1, r1, f2, r2)| {
            format!("{}{}{}{}", f1 as char, r1 as char, f2 as char, r2 as char)
        });

    prop_oneof![70 => common, 30 => random]
}

/// Valid FEN strings (starting position variations)
fn valid_fen_strategy() -> impl Strategy<Value = String> {
    // Most tests use starting position or early game
    prop_oneof![
        60 => Just("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string()),
        20 => Just("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string()),
        20 => Just("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2".to_string()),
    ]
}

/// Build a create_game instruction
pub fn build_create_game_ix(
    program_id: Pubkey,
    payer: Pubkey,
    game_id: u64,
    wager_amount: u64,
    game_type: GameType,
) -> anyhow::Result<Instruction> {
    use sha2::{Digest, Sha256};

    // Compute Anchor discriminator
    let mut hasher = Sha256::new();
    hasher.update(b"global:create_game");
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);

    // PDA seeds
    const GAME_SEED: &[u8] = b"game";
    const MOVE_LOG_SEED: &[u8] = b"move_log";
    const WAGER_ESCROW_SEED: &[u8] = b"escrow";
    const PROFILE_SEED: &[u8] = b"profile";

    let game_pda = Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    ).0;
    let move_log_pda = Pubkey::find_program_address(
        &[MOVE_LOG_SEED, &game_id.to_le_bytes()],
        &program_id,
    ).0;
    let escrow_pda = Pubkey::find_program_address(
        &[WAGER_ESCROW_SEED, &game_id.to_le_bytes()],
        &program_id,
    ).0;
    let player_profile = Pubkey::find_program_address(
        &[PROFILE_SEED, payer.as_ref()],
        &program_id,
    ).0;

    // Serialize instruction data
    let mut data = disc.to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager_amount.to_le_bytes());
    data.push(match game_type {
        GameType::PvP => 0,
        GameType::PvAI => 1,
    });

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(move_log_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(player_profile, false),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_instruction_generation(insts in instruction_sequence_strategy(10, 5)) {
            // Just verify we can generate sequences
            assert!(!insts.is_empty());
        }
    }
}
