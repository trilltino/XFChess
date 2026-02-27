//! Solana instruction builders

use anyhow::Result;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

/// Program ID constant - should be replaced with actual program ID
pub const PROGRAM_ID: &str = "XFChessGame1111111111111111111111111111111";

/// Create instruction to initialize player profile
pub fn init_profile_ix(_payer: Pubkey) -> Instruction {
    // TODO: Implement actual instruction building
    // This is a placeholder that needs proper implementation
    Instruction {
        program_id: PROGRAM_ID.parse().unwrap(),
        accounts: vec![],
        data: vec![],
    }
}

/// Create instruction to create a new game
pub fn create_game_ix(program_id: Pubkey, payer: Pubkey, game_id: u64) -> Instruction {
    use solana_sdk::instruction::AccountMeta;
    
    // Derive game PDA
    let game_pda = Pubkey::find_program_address(
        &[b"game", &game_id.to_le_bytes()],
        &program_id,
    ).0;
    
    // Build the instruction data
    let data = {
        let mut data = vec![0]; // Instruction discriminator for create_game
        data.extend_from_slice(&game_id.to_le_bytes());
        data
    };
    
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer, true), // payer, signer
            AccountMeta::new(game_pda, false), // game_pda
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false), // system_program
        ],
        data,
    }
}

/// Create instruction to record a move
pub fn make_move_instruction(
    payer: Pubkey,
    game_pda: Pubkey,
    from: u8,
    to: u8,
) -> Result<Instruction> {
    use solana_sdk::instruction::AccountMeta;
    
    // Get program ID
    let program_id: Pubkey = PROGRAM_ID.parse()?;
    
    // Build the instruction data
    let data = {
        let mut data = vec![1]; // Instruction discriminator for record_move
        data.push(from);
        data.push(to);
        data
    };
    
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer, true), // payer, signer
            AccountMeta::new(game_pda, false), // game_pda
        ],
        data,
    })
}

/// Create instruction to finish a game
pub fn finish_game_instruction(
    payer: Pubkey,
    game_pda: Pubkey,
    _winner: Pubkey,
) -> Result<Instruction> {
    use solana_sdk::instruction::AccountMeta;
    
    // Get program ID
    let program_id: Pubkey = PROGRAM_ID.parse()?;
    
    // Build the instruction data
    let data = vec![2]; // Instruction discriminator for finalize_game
    
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer, true), // payer, signer
            AccountMeta::new(game_pda, false), // game_pda
        ],
        data,
    })
}

/// Create instruction to commit a batch of moves
pub fn commit_move_batch_ix(
    payer: Pubkey,
    game_pda: Pubkey,
    moves: Vec<(u8, u8)>,
    signatures: Vec<[u8; 64]>,
) -> Result<Instruction> {
    use solana_sdk::instruction::AccountMeta;
    
    // Get program ID
    let program_id: Pubkey = PROGRAM_ID.parse()?;
    
    // Build the instruction data
    let mut data = vec![3]; // Instruction discriminator for commit_move_batch
    data.extend_from_slice(&(moves.len() as u16).to_le_bytes());
    for (from, to) in moves {
        data.push(from);
        data.push(to);
    }
    for sig in signatures {
        data.extend_from_slice(&sig);
    }
    
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer, true), // payer, signer
            AccountMeta::new(game_pda, false), // game_pda
        ],
        data,
    })
}

/// Create instruction to authorize a session key
pub fn authorize_session_key_ix(
    payer: Pubkey,
    game_pda: Pubkey,
    session_key: Pubkey,
    expires_at: i64,
) -> Result<Instruction> {
    use solana_sdk::instruction::AccountMeta;
    
    // Get program ID
    let program_id: Pubkey = PROGRAM_ID.parse()?;
    
    // Build the instruction data
    let mut data = vec![4]; // Instruction discriminator for authorize_session
    data.extend_from_slice(session_key.as_ref());
    data.extend_from_slice(&expires_at.to_le_bytes());
    
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer, true), // payer, signer
            AccountMeta::new(game_pda, false), // game_pda
            AccountMeta::new_readonly(session_key, false), // session_key
        ],
        data,
    })
}

/// Re-export program ID for convenience
pub fn get_program_id() -> Result<Pubkey> {
    PROGRAM_ID.parse().map_err(|e| anyhow::anyhow!("Invalid program ID: {}", e))
}
