use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use super::{
    DELEGATION_PROGRAM_ID, GAME_SEED, MAGIC_CONTEXT_PUBKEY, MAGIC_PROGRAM_PUBKEY, PROFILE_SEED,
    SESSION_DELEGATION_SEED, TOURNAMENT_SEED, WAGER_ESCROW_SEED,
};

const TOURNAMENT_ESCROW_SEED: &[u8] = b"t_escrow";
const TOURNAMENT_PLAYERS_SEED: &[u8] = b"tourney_players";
pub(crate) const TOURNAMENT_MATCH_SEED: &[u8] = b"t_match";
const TOURNAMENT_USDC_PRIZE_SEED: &[u8] = b"t_usdc_prize";

pub struct MoveInput {
    pub game_id: u64,
    pub move_uci: [u8; 5],
    pub next_board: [u8; 68],
    pub nonce: u64,
    pub signature: Option<Vec<u8>>,
    pub parent_nonce: Option<u64>,
}

pub struct TournamentInit<'a> {
    pub tournament_id: u64,
    pub name: &'a str,
    pub entry_fee: u64,
    pub platform_fee: u64,
    pub max_players: u16,
    pub tournament_type: u8,
    pub swiss_rounds: u8,
    pub elo_min: u32,
    pub elo_max: u32,
    pub min_players: u16,
    pub prize_shares: [u16; 10],
    pub winner_takes_all: bool,
    pub host_treasury: &'a Pubkey,
}

pub struct MatchInit<'a> {
    pub tournament_id: u64,
    pub match_index: u16,
    pub round: u8,
    pub player_white: Option<&'a Pubkey>,
    pub player_black: Option<&'a Pubkey>,
    pub next_match_for_winner: Option<u16>,
    pub next_match_slot: u8,
    pub authority: &'a Pubkey,
}

pub(crate) fn anchor_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", name));
    let hash = hasher.finalize();
    let mut discriminator = [0; 8];
    discriminator.copy_from_slice(&hash[..8]);
    discriminator
}

fn borsh_string(s: &str) -> Vec<u8> {
    let mut v = (s.len() as u32).to_le_bytes().to_vec();
    v.extend_from_slice(s.as_bytes());
    v
}

pub fn record_move_ix(
    program_id: &Pubkey,
    session_pubkey: &Pubkey,
    wallet_pubkey: &Pubkey,
    input: MoveInput,
) -> anyhow::Result<Instruction> {
    let MoveInput {
        game_id,
        move_uci,
        next_board,
        nonce,
        signature,
        parent_nonce,
    } = input;
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[
            SESSION_DELEGATION_SEED,
            &game_id.to_le_bytes(),
            wallet_pubkey.as_ref(),
        ],
        program_id,
    )
    .0;

    let mut data = anchor_discriminator("record_move").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&move_uci);
    data.extend_from_slice(&next_board);
    data.extend_from_slice(&nonce.to_le_bytes());

    if let Some(sig) = signature {
        data.push(1);
        data.extend_from_slice(&(sig.len() as u32).to_le_bytes());
        data.extend_from_slice(&sig);
    } else {
        data.push(0);
    }

    if let Some(parent_nonce) = parent_nonce {
        data.push(1);
        data.extend_from_slice(&parent_nonce.to_le_bytes());
    } else {
        data.push(0);
    }

    Ok(Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(*session_pubkey, true),
            AccountMeta::new_readonly(session_delegation_pda, false),
        ],
        data,
    })
}

pub fn global_record_move_ix(
    program_id: &Pubkey,
    session_pubkey: &Pubkey,
    wallet_pubkey: &Pubkey,
    input: MoveInput,
) -> anyhow::Result<Instruction> {
    let MoveInput {
        game_id,
        move_uci,
        next_board,
        nonce,
        signature,
        parent_nonce,
    } = input;
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let global_session_pda =
        Pubkey::find_program_address(&[b"global_session", wallet_pubkey.as_ref()], program_id).0;

    let mut data = anchor_discriminator("global_record_move").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&move_uci);
    data.extend_from_slice(&next_board);
    data.extend_from_slice(&nonce.to_le_bytes());

    if let Some(sig) = signature {
        data.push(1);
        data.extend_from_slice(&(sig.len() as u32).to_le_bytes());
        data.extend_from_slice(&sig);
    } else {
        data.push(0);
    }

    if let Some(parent_nonce) = parent_nonce {
        data.push(1);
        data.extend_from_slice(&parent_nonce.to_le_bytes());
    } else {
        data.push(0);
    }

    Ok(Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(*session_pubkey, true),
            AccountMeta::new_readonly(global_session_pda, false),
        ],
        data,
    })
}

pub fn delegate_game_ix(
    program_id: &Pubkey,
    game_id: u64,
    payer: &Pubkey,
    fee_payer: &Pubkey,
) -> anyhow::Result<Instruction> {
    use ephemeral_rollups_sdk::pda::{
        delegate_buffer_pda_from_delegated_account_and_owner_program,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
    };

    let delegation_program_id: Pubkey = DELEGATION_PROGRAM_ID
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid delegation program id: {}", e))?;
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;

    let buffer_pda = Pubkey::new_from_array(
        delegate_buffer_pda_from_delegated_account_and_owner_program(
            &game_pda.to_bytes().into(),
            &program_id.to_bytes().into(),
        )
        .to_bytes(),
    );
    let delegation_record = Pubkey::new_from_array(
        delegation_record_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );
    let delegation_metadata = Pubkey::new_from_array(
        delegation_metadata_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );

    let valid_until: i64 = 600;
    let mut data = anchor_discriminator("delegate_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&valid_until.to_le_bytes());

    Ok(Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(*payer, true),
            AccountMeta::new_readonly(*program_id, false),
            AccountMeta::new(buffer_pda, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new_readonly(delegation_program_id, false),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
            AccountMeta::new(*fee_payer, true),
        ],
        data,
    })
}

pub fn request_force_undelegate_ix(
    program_id: &Pubkey,
    game_id: u64,
    payer: &Pubkey,
) -> anyhow::Result<Instruction> {
    use ephemeral_rollups_sdk::pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        undelegation_request_pda_from_delegated_account,
    };

    let delegation_program_id: Pubkey = DELEGATION_PROGRAM_ID
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid delegation program id: {}", e))?;
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;

    let undelegation_request = Pubkey::new_from_array(
        undelegation_request_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );
    let delegation_record = Pubkey::new_from_array(
        delegation_record_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );
    let delegation_metadata = Pubkey::new_from_array(
        delegation_metadata_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );

    let mut data = anchor_discriminator("request_force_undelegate").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(*payer, true),
            AccountMeta::new_readonly(*program_id, false),
            AccountMeta::new(undelegation_request, false),
            AccountMeta::new_readonly(delegation_record, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new_readonly(delegation_program_id, false),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data,
    })
}

pub fn force_undelegate_after_timeout_ix(
    program_id: &Pubkey,
    game_id: u64,
    payer: &Pubkey,
) -> anyhow::Result<Instruction> {
    use ephemeral_rollups_sdk::pda::{
        commit_record_pda_from_delegated_account, commit_state_pda_from_delegated_account,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        undelegation_request_pda_from_delegated_account,
    };

    let delegation_program_id: Pubkey = DELEGATION_PROGRAM_ID
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid delegation program id: {}", e))?;
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;

    let undelegation_request = Pubkey::new_from_array(
        undelegation_request_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );
    let delegation_record = Pubkey::new_from_array(
        delegation_record_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );
    let delegation_metadata = Pubkey::new_from_array(
        delegation_metadata_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );
    let commit_state = Pubkey::new_from_array(
        commit_state_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );
    let commit_record = Pubkey::new_from_array(
        commit_record_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );

    let mut data = anchor_discriminator("force_undelegate_after_timeout").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(*program_id, false),
            AccountMeta::new(undelegation_request, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new(*payer, false),
            AccountMeta::new(commit_state, false),
            AccountMeta::new(commit_record, false),
            AccountMeta::new(*payer, false), // commit_reimbursement placeholder
            AccountMeta::new_readonly(delegation_program_id, false),
        ],
        data,
    })
}

pub fn recover_stuck_delegation_ix(
    program_id: &Pubkey,
    game_id: u64,
    white: &Pubkey,
    black: &Pubkey,
    dispute_authority: &Pubkey,
) -> Instruction {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], program_id).0;
    let treasury_vault = Pubkey::find_program_address(&[b"treasury_vault"], program_id).0;

    let mut data = anchor_discriminator("recover_stuck_delegation").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(treasury_vault, false),
            AccountMeta::new(*white, false),
            AccountMeta::new(*black, false),
            AccountMeta::new_readonly(*dispute_authority, true),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data,
    }
}

pub fn undelegate_game_ix(
    program_id: &Pubkey,
    session_pubkey: &Pubkey,
    game_id: u64,
) -> anyhow::Result<Instruction> {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let magic_context: Pubkey = MAGIC_CONTEXT_PUBKEY
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid magic context pubkey: {}", e))?;
    let magic_program: Pubkey = MAGIC_PROGRAM_PUBKEY
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid magic program pubkey: {}", e))?;

    let mut data = anchor_discriminator("undelegate_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(*session_pubkey, true),
            AccountMeta::new(magic_context, false),
            AccountMeta::new_readonly(magic_program, false),
        ],
        data,
    })
}

pub fn schedule_time_check_ix(
    program_id: &Pubkey,
    payer: &Pubkey,
    game_id: u64,
    white: &Pubkey,
    black: &Pubkey,
    check_interval_millis: u64,
) -> anyhow::Result<Instruction> {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let magic_program: Pubkey = MAGIC_PROGRAM_PUBKEY
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid magic program pubkey: {}", e))?;

    let mut data = anchor_discriminator("schedule_time_check").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes()); // task_id
    data.extend_from_slice(&check_interval_millis.to_le_bytes());
    let iterations = i64::MAX as u64; // effectively unlimited until cancelled
    data.extend_from_slice(&iterations.to_le_bytes());

    Ok(Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(*white, false),
            AccountMeta::new_readonly(*black, false),
            AccountMeta::new_readonly(magic_program, false),
        ],
        data,
    })
}

pub fn cancel_time_check_ix(
    program_id: &Pubkey,
    payer: &Pubkey,
    game_id: u64,
) -> anyhow::Result<Instruction> {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let magic_program: Pubkey = MAGIC_PROGRAM_PUBKEY
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid magic program pubkey: {}", e))?;

    let mut data = anchor_discriminator("cancel_time_check").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes()); // task_id

    Ok(Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(magic_program, false),
        ],
        data,
    })
}

/// `claim_timeout(game_id)`: permissionless; the program decides whether the
/// side to move has run out of time.
pub fn claim_timeout_ix(program_id: &Pubkey, caller: &Pubkey, game_id: u64) -> Instruction {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let mut data = anchor_discriminator("claim_timeout").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(*caller, true),
        ],
        data,
    }
}

pub fn finalize_game_ix(
    program_id: &Pubkey,
    game_id: u64,
    white: &Pubkey,
    black: &Pubkey,
    _winner: Option<&str>,
    fee_payer: &Pubkey,
) -> Instruction {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let white_profile = Pubkey::find_program_address(&[PROFILE_SEED, white.as_ref()], program_id).0;
    let black_profile = Pubkey::find_program_address(&[PROFILE_SEED, black.as_ref()], program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], program_id).0;
    let treasury_vault = Pubkey::find_program_address(&[b"treasury_vault"], program_id).0;

    let mut data = anchor_discriminator("finalize_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(white_profile, false),
            AccountMeta::new(black_profile, false),
            AccountMeta::new(*white, false),
            AccountMeta::new(*black, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(treasury_vault, false),
            AccountMeta::new(*fee_payer, false),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data,
    }
}

pub fn link_external_elo_ix(
    program_id: &Pubkey,
    link_authority: &Pubkey,
    player: &Pubkey,
    username: &str,
    blitz_rating: u32,
    rapid_rating: u32,
    bullet_rating: u32,
) -> Instruction {
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], program_id).0;
    let lichess_username_record_pda =
        Pubkey::find_program_address(&[b"lichess_username", username.as_bytes()], program_id).0;

    let mut data = anchor_discriminator("link_external_elo").to_vec();
    data.extend(borsh_string(username));
    data.extend_from_slice(&blitz_rating.to_le_bytes());
    data.extend_from_slice(&rapid_rating.to_le_bytes());
    data.extend_from_slice(&bullet_rating.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(player_profile_pda, false),
            AccountMeta::new_readonly(*player, false),
            AccountMeta::new(lichess_username_record_pda, false),
            AccountMeta::new(*link_authority, true),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data,
    }
}

pub fn verify_profile_ix(program_id: &Pubkey, admin: &Pubkey, player: &Pubkey) -> Instruction {
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], program_id).0;

    let data = anchor_discriminator("verify_profile").to_vec();

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(player_profile_pda, false),
            AccountMeta::new(*admin, true), // The KYC authority fee-payer
            AccountMeta::new_readonly(*player, false),
        ],
        data,
    }
}

pub fn withdraw_treasury_ix(
    program_id: &Pubkey,
    authority: &Pubkey,
    destination: &Pubkey,
    amount: u64,
) -> Instruction {
    let treasury_vault = Pubkey::find_program_address(&[b"treasury_vault"], program_id).0;

    let mut data = anchor_discriminator("withdraw_treasury").to_vec();
    data.extend_from_slice(&amount.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(treasury_vault, false),
            AccountMeta::new(*authority, true),
            AccountMeta::new(*destination, false),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data,
    }
}

pub fn leave_tournament_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    max_players: u16,
    player: &Pubkey,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        program_id,
    )
    .0;

    let present = required_shards(max_players);
    // Absent shards are passed as the program ID → Anchor resolves them to None.
    let shard = |idx: u8| {
        if idx < present {
            let pda = Pubkey::find_program_address(
                &[
                    TOURNAMENT_PLAYERS_SEED,
                    &[idx],
                    &tournament_id.to_le_bytes(),
                ],
                program_id,
            )
            .0;
            AccountMeta::new(pda, false)
        } else {
            AccountMeta::new_readonly(*program_id, false)
        }
    };

    let mut data = anchor_discriminator("leave_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Instruction {
        program_id: *program_id,
        // Account order must match `LeaveTournament`: tournament, shards 0-3,
        // player, escrow, system_program.
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            shard(0),
            shard(1),
            shard(2),
            shard(3),
            AccountMeta::new(*player, true),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data,
    }
}

pub fn initialize_tournament_ix(
    program_id: &Pubkey,
    admin: &Pubkey,
    input: TournamentInit<'_>,
) -> Instruction {
    let TournamentInit {
        tournament_id,
        name,
        entry_fee,
        platform_fee,
        max_players,
        tournament_type,
        swiss_rounds,
        elo_min,
        elo_max,
        min_players,
        prize_shares,
        winner_takes_all,
        host_treasury,
    } = input;
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;

    let mut data = anchor_discriminator("initialize_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend(borsh_string(name));
    data.extend_from_slice(&entry_fee.to_le_bytes());
    data.extend_from_slice(&max_players.to_le_bytes());

    // Borsh variants are Swiss=0 and SingleElimination=1, opposite to this function’s parameter convention.
    if tournament_type == 1 {
        data.push(0); // TournamentType::Swiss { rounds }
        data.push(swiss_rounds);
    } else {
        data.push(1); // TournamentType::SingleElimination
    }

    data.extend_from_slice(&elo_min.to_le_bytes());
    data.extend_from_slice(&elo_max.to_le_bytes());
    data.extend_from_slice(&min_players.to_le_bytes());
    for &share in prize_shares.iter() {
        data.extend_from_slice(&share.to_le_bytes());
    }
    data.extend_from_slice(&platform_fee.to_le_bytes());
    data.push(if winner_takes_all { 1 } else { 0 });
    data.extend_from_slice(host_treasury.as_ref());

    // Optional usdc_mint (None = 0)
    data.push(0);

    // Default time controls
    data.extend_from_slice(&600u64.to_le_bytes()); // 10 mins
    data.extend_from_slice(&0u16.to_le_bytes()); // 0 inc

    let usdc_prize_escrow_authority = Pubkey::find_program_address(
        &[TOURNAMENT_USDC_PRIZE_SEED, &tournament_id.to_le_bytes()],
        program_id,
    )
    .0;
    let token_program: Pubkey = solana_sdk::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    let associated_token_program: Pubkey =
        solana_sdk::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

    Instruction {
        program_id: *program_id,
        // Account order matches InitializeTournament. SOL-only tournaments use the
        // program ID as Anchor's None marker for the two optional USDC accounts.
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(usdc_prize_escrow_authority, false),
            AccountMeta::new_readonly(*program_id, false),
            AccountMeta::new_readonly(*program_id, false),
            AccountMeta::new(*admin, true),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data,
    }
}

pub fn initialize_escrow_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    authority: &Pubkey,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        program_id,
    )
    .0;

    let mut data = anchor_discriminator("initialize_tournament_escrow").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(*authority, true),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data,
    }
}

pub fn initialize_shards_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    max_players: u16,
    authority: &Pubkey,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;

    let shard = |idx: u8| {
        Pubkey::find_program_address(
            &[
                TOURNAMENT_PLAYERS_SEED,
                &[idx],
                &tournament_id.to_le_bytes(),
            ],
            program_id,
        )
        .0
    };

    let discriminator_name = if max_players <= 64 {
        "initialize_shards_small"
    } else if max_players <= 128 {
        "initialize_shards_medium"
    } else {
        "initialize_shards_large"
    };

    let mut data = anchor_discriminator(discriminator_name).to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: if max_players <= 64 {
            vec![
                AccountMeta::new_readonly(tournament_pda, false),
                AccountMeta::new(shard(0), false),
                AccountMeta::new(*authority, true),
                AccountMeta::new_readonly(solana_system_interface::program::id(), false),
            ]
        } else if max_players <= 128 {
            vec![
                AccountMeta::new_readonly(tournament_pda, false),
                AccountMeta::new(shard(0), false),
                AccountMeta::new(shard(1), false),
                AccountMeta::new(*authority, true),
                AccountMeta::new_readonly(solana_system_interface::program::id(), false),
            ]
        } else {
            vec![
                AccountMeta::new_readonly(tournament_pda, false),
                AccountMeta::new(shard(0), false),
                AccountMeta::new(shard(1), false),
                AccountMeta::new(shard(2), false),
                AccountMeta::new(shard(3), false),
                AccountMeta::new(*authority, true),
                AccountMeta::new_readonly(solana_system_interface::program::id(), false),
            ]
        },
        data,
    }
}

#[cfg(test)]
mod tournament_account_layout_tests {
    use super::*;

    #[test]
    fn tournament_init_account_lists_match_anchor_layout() {
        let program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let tournament_id = 1789850187401u64;

        let escrow_ix = initialize_escrow_ix(&program_id, tournament_id, &authority);
        assert_eq!(escrow_ix.accounts.len(), 4);
        assert!(escrow_ix
            .accounts
            .iter()
            .all(|meta| meta.pubkey != solana_sdk::sysvar::rent::id()));

        let shards_ix = initialize_shards_ix(&program_id, tournament_id, 64, &authority);
        assert_eq!(shards_ix.accounts.len(), 4);
        assert!(shards_ix
            .accounts
            .iter()
            .all(|meta| meta.pubkey != solana_sdk::sysvar::rent::id()));
    }
}

pub fn required_shards(max_players: u16) -> u8 {
    match max_players {
        0..=64 => 1,
        65..=128 => 2,
        _ => 4,
    }
}

pub fn bracket_position(max_players: u16, match_index: u16) -> (u8, Option<u16>, u8) {
    let total_matches = max_players.saturating_sub(1);
    let mut round_start = 0u16;
    let mut round_size = max_players / 2;
    let mut round = 0u8;
    while round_size > 1 && match_index >= round_start + round_size {
        round_start += round_size;
        round_size /= 2;
        round += 1;
    }
    let pos_in_round = match_index - round_start;
    let next = if match_index + 1 >= total_matches {
        None // the final
    } else {
        Some(round_start + round_size + pos_in_round / 2)
    };
    (round, next, (pos_in_round % 2) as u8)
}

pub fn start_tournament_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    max_players: u16,
    authority: &Pubkey,
    host_treasury: &Pubkey,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;

    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        program_id,
    )
    .0;

    let present = required_shards(max_players);
    // Present shards must be writable (start re-seeds players by ELO); absent
    // shards are passed as the program ID → Anchor resolves them to None.
    let shard = |idx: u8| {
        if idx < present {
            let pda = Pubkey::find_program_address(
                &[
                    TOURNAMENT_PLAYERS_SEED,
                    &[idx],
                    &tournament_id.to_le_bytes(),
                ],
                program_id,
            )
            .0;
            AccountMeta::new(pda, false)
        } else {
            AccountMeta::new_readonly(*program_id, false)
        }
    };

    let mut data = anchor_discriminator("start_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            shard(0),
            shard(1),
            shard(2),
            shard(3),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(*host_treasury, false),
            AccountMeta::new(*authority, true),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data,
    }
}

pub fn cancel_tournament_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    max_players: u16,
    authority: &Pubkey,
    host_treasury: &Pubkey,
    players: &[Pubkey],
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        program_id,
    )
    .0;
    let usdc_prize_escrow_authority = Pubkey::find_program_address(
        &[TOURNAMENT_USDC_PRIZE_SEED, &tournament_id.to_le_bytes()],
        program_id,
    )
    .0;
    let token_program: Pubkey = solana_sdk::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

    let present = required_shards(max_players);
    let shard = |idx: u8| {
        if idx < present {
            let pda = Pubkey::find_program_address(
                &[
                    TOURNAMENT_PLAYERS_SEED,
                    &[idx],
                    &tournament_id.to_le_bytes(),
                ],
                program_id,
            )
            .0;
            AccountMeta::new_readonly(pda, false)
        } else {
            AccountMeta::new_readonly(*program_id, false)
        }
    };

    let mut data = anchor_discriminator("cancel_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    let mut accounts = vec![
        AccountMeta::new(tournament_pda, false),
        shard(0),
        shard(1),
        shard(2),
        shard(3),
        AccountMeta::new_readonly(usdc_prize_escrow_authority, false),
        AccountMeta::new_readonly(*program_id, false), // usdc_prize_escrow: None
        AccountMeta::new_readonly(*program_id, false), // operator_usdc_ata: None
        AccountMeta::new_readonly(*program_id, false), // usdc_mint: None
        AccountMeta::new(escrow_pda, false),
        AccountMeta::new(*host_treasury, true),
        AccountMeta::new(*authority, true),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(solana_system_interface::program::id(), false),
    ];
    accounts.extend(players.iter().map(|p| AccountMeta::new(*p, false)));

    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

pub fn fund_sol_prize_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    operator: &Pubkey,
    amount_lamports: u64,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;

    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        program_id,
    )
    .0;

    let mut data = anchor_discriminator("fund_sol_prize").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&amount_lamports.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(*operator, true),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data,
    }
}

pub fn register_player_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    elo: u32,
    player: &Pubkey,
    max_players: u16,
    host_treasury: &Pubkey,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;
    let profile_pda = Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], program_id).0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        program_id,
    )
    .0;
    let shard_pda = |shard: u8| {
        Pubkey::find_program_address(
            &[
                TOURNAMENT_PLAYERS_SEED,
                &[shard],
                &tournament_id.to_le_bytes(),
            ],
            program_id,
        )
        .0
    };

    let mut data = anchor_discriminator("register_player").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&elo.to_le_bytes());

    let shard_meta = |shard: u8, present: bool| {
        if present {
            AccountMeta::new(shard_pda(shard), false)
        } else {
            AccountMeta::new_readonly(*program_id, false)
        }
    };
    let accounts = vec![
        AccountMeta::new(tournament_pda, false),
        AccountMeta::new_readonly(profile_pda, false),
        AccountMeta::new(*player, true),
        AccountMeta::new(escrow_pda, false),
        AccountMeta::new(shard_pda(0), false),
        shard_meta(1, max_players >= 128),
        shard_meta(2, max_players >= 256),
        shard_meta(3, max_players >= 256),
        AccountMeta::new(*host_treasury, false),
        AccountMeta::new_readonly(solana_system_interface::program::id(), false),
    ];

    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

/// Shard accounts 0-3 for the Swiss instructions. Shards beyond what the tournament
/// was initialised with are passed as the program ID, Anchor's `None` marker.
fn swiss_shard_metas(
    program_id: &Pubkey,
    tournament_id: u64,
    max_players: u16,
) -> Vec<AccountMeta> {
    let present = required_shards(max_players);
    (0..4u8)
        .map(|shard| {
            if shard < present {
                let pda = Pubkey::find_program_address(
                    &[
                        TOURNAMENT_PLAYERS_SEED,
                        &[shard],
                        &tournament_id.to_le_bytes(),
                    ],
                    program_id,
                )
                .0;
                AccountMeta::new(pda, false)
            } else {
                AccountMeta::new_readonly(*program_id, false)
            }
        })
        .collect()
}

/// Closes the current on-chain Swiss round; every one of its `boards_in_round`
/// boards must already be reported.
pub fn advance_round_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    boards_in_round: u16,
    authority: &Pubkey,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;
    let mut data = anchor_discriminator("advance_round").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&boards_in_round.to_le_bytes());
    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(*authority, true),
        ],
        data,
    }
}

/// Finalises a Swiss tournament with the off-chain final order (top 10 at most).
pub fn complete_swiss_tournament_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    max_players: u16,
    places: &[Pubkey],
    authority: &Pubkey,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;
    let mut data = anchor_discriminator("complete_swiss_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&(places.len() as u32).to_le_bytes());
    for place in places {
        data.extend_from_slice(place.as_ref());
    }
    let mut accounts = vec![AccountMeta::new(tournament_pda, false)];
    accounts.extend(
        swiss_shard_metas(program_id, tournament_id, max_players)
            .into_iter()
            .map(|meta| AccountMeta::new_readonly(meta.pubkey, false)),
    );
    accounts.push(AccountMeta::new_readonly(*authority, true));
    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

/// Seats a tournament pairing in an Active Game with both players' per-game move
/// sessions pointing at `session_key`. Signed (and rent paid) by the tournament
/// authority.
#[allow(clippy::too_many_arguments)]
pub fn create_tournament_game_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    max_players: u16,
    game_id: u64,
    white: &Pubkey,
    black: &Pubkey,
    session_key: &Pubkey,
    authority: &Pubkey,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let session_pda = |player: &Pubkey| {
        Pubkey::find_program_address(
            &[
                SESSION_DELEGATION_SEED,
                &game_id.to_le_bytes(),
                player.as_ref(),
            ],
            program_id,
        )
        .0
    };
    let mut data = anchor_discriminator("create_tournament_game").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(session_key.as_ref());

    let mut accounts = vec![AccountMeta::new_readonly(tournament_pda, false)];
    accounts.extend(
        swiss_shard_metas(program_id, tournament_id, max_players)
            .into_iter()
            .map(|meta| AccountMeta::new_readonly(meta.pubkey, false)),
    );
    accounts.extend([
        AccountMeta::new(game_pda, false),
        AccountMeta::new(session_pda(white), false),
        AccountMeta::new(session_pda(black), false),
        AccountMeta::new_readonly(*white, false),
        AccountMeta::new_readonly(*black, false),
        AccountMeta::new(*authority, true),
        AccountMeta::new_readonly(solana_system_interface::program::id(), false),
    ]);
    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

/// On-chain `SwissMatchResult` Borsh variant tags.
pub const SWISS_WHITE_WIN: u8 = 0;
pub const SWISS_BLACK_WIN: u8 = 1;
pub const SWISS_DRAW: u8 = 2;
/// A bye, reported as a pseudo-board with the bye player as both `white` and `black`.
pub const SWISS_BYE: u8 = 3;

/// Records one board of the current on-chain Swiss round. `round` and `board` are
/// the program's 0-based indices.
#[allow(clippy::too_many_arguments)]
pub fn record_swiss_result_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    max_players: u16,
    round: u8,
    board: u16,
    result_variant: u8,
    white: &Pubkey,
    black: &Pubkey,
    authority: &Pubkey,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;
    let mut data = anchor_discriminator("record_swiss_result").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.push(round);
    data.extend_from_slice(&board.to_le_bytes());
    data.push(result_variant);
    let mut accounts = vec![AccountMeta::new(tournament_pda, false)];
    accounts.extend(swiss_shard_metas(program_id, tournament_id, max_players));
    accounts.push(AccountMeta::new_readonly(*authority, true));
    accounts.push(AccountMeta::new_readonly(*white, false));
    accounts.push(AccountMeta::new_readonly(*black, false));
    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

pub fn distribute_tournament_prizes_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    cranker: &Pubkey,
    winners: &[Pubkey],
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        program_id,
    )
    .0;

    let mut data = anchor_discriminator("distribute_tournament_prizes").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    let mut accounts = vec![
        AccountMeta::new(tournament_pda, false),
        AccountMeta::new(escrow_pda, false),
        AccountMeta::new_readonly(*cranker, true),
    ];
    accounts.extend(winners.iter().map(|w| AccountMeta::new(*w, false)));

    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

pub fn initialize_match_ix(program_id: &Pubkey, input: MatchInit<'_>) -> Instruction {
    let MatchInit {
        tournament_id,
        match_index,
        round,
        player_white,
        player_black,
        next_match_for_winner,
        next_match_slot,
        authority,
    } = input;
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;
    let match_pda = Pubkey::find_program_address(
        &[
            TOURNAMENT_MATCH_SEED,
            &tournament_id.to_le_bytes(),
            &match_index.to_le_bytes(),
        ],
        program_id,
    )
    .0;

    let mut data = anchor_discriminator("initialize_match").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&match_index.to_le_bytes());
    data.push(round);
    // Option<Pubkey> Borsh encoding
    match player_white {
        Some(pk) => {
            data.push(1);
            data.extend_from_slice(pk.as_ref());
        }
        None => {
            data.push(0);
        }
    }
    match player_black {
        Some(pk) => {
            data.push(1);
            data.extend_from_slice(pk.as_ref());
        }
        None => {
            data.push(0);
        }
    }
    // Option<u16>
    match next_match_for_winner {
        Some(n) => {
            data.push(1);
            data.extend_from_slice(&n.to_le_bytes());
        }
        None => {
            data.push(0);
        }
    }
    data.push(next_match_slot);

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(match_pda, false),
            AccountMeta::new(*authority, true),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data,
    }
}

pub fn record_result_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    match_index: u16,
    winner: &Pubkey,
    loser: &Pubkey,
    authority: &Pubkey,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;
    let match_pda = Pubkey::find_program_address(
        &[
            TOURNAMENT_MATCH_SEED,
            &tournament_id.to_le_bytes(),
            &match_index.to_le_bytes(),
        ],
        program_id,
    )
    .0;

    let mut data = anchor_discriminator("record_match_result").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&match_index.to_le_bytes());
    data.extend_from_slice(winner.as_ref());
    data.extend_from_slice(loser.as_ref());

    Instruction {
        program_id: *program_id,
        // Account order must match `RecordMatchResult`: tournament, match, authority.
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(match_pda, false),
            AccountMeta::new(*authority, true),
        ],
        data,
    }
}

pub fn advance_winner_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    source_match_index: u16,
    target_match_index: u16,
    authority: &Pubkey,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;
    let match_pda = |idx: u16| {
        Pubkey::find_program_address(
            &[
                TOURNAMENT_MATCH_SEED,
                &tournament_id.to_le_bytes(),
                &idx.to_le_bytes(),
            ],
            program_id,
        )
        .0
    };

    let mut data = anchor_discriminator("advance_winner").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&source_match_index.to_le_bytes());
    data.extend_from_slice(&target_match_index.to_le_bytes());

    Instruction {
        program_id: *program_id,
        // Account order must match `AdvanceWinner`: tournament, source, target, authority.
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(match_pda(source_match_index), false),
            AccountMeta::new(match_pda(target_match_index), false),
            AccountMeta::new(*authority, true),
        ],
        data,
    }
}

pub fn claim_prize_ix(program_id: &Pubkey, tournament_id: u64, claimant: &Pubkey) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        program_id,
    )
    .0;
    let usdc_prize_authority =
        Pubkey::find_program_address(&[b"t_usdc_prize", &tournament_id.to_le_bytes()], program_id)
            .0;

    let mut data = anchor_discriminator("claim_tournament_prize").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(usdc_prize_authority, false),
            // usdc_prize_escrow — None (SOL-only path), omit optional accounts
            // claimant_usdc_ata — None
            // usdc_mint — None
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(*claimant, false),
            AccountMeta::new(*claimant, true), // claimant signer
            // SPL Token program (required by the program even on the SOL path)
            AccountMeta::new_readonly(
                solana_sdk::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
                false,
            ),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context as _;

    #[test]
    fn claim_timeout_matches_the_program_account_order() {
        let program_id = Pubkey::new_unique();
        let caller = Pubkey::new_unique();
        let ix = claim_timeout_ix(&program_id, &caller, 42);

        let game = Pubkey::find_program_address(&[GAME_SEED, &42u64.to_le_bytes()], &program_id).0;
        assert_eq!(ix.accounts.len(), 2);
        assert_eq!(
            (
                ix.accounts[0].pubkey,
                ix.accounts[0].is_writable,
                ix.accounts[0].is_signer
            ),
            (game, true, false)
        );
        assert_eq!(
            (
                ix.accounts[1].pubkey,
                ix.accounts[1].is_writable,
                ix.accounts[1].is_signer
            ),
            (caller, false, true)
        );
        assert_eq!(&ix.data[..8], &anchor_discriminator("claim_timeout"));
        assert_eq!(&ix.data[8..], &42u64.to_le_bytes());
    }

    #[test]
    fn bracket_position_two_players() {
        // One match: it is the final.
        assert_eq!(bracket_position(2, 0), (0, None, 0));
    }

    #[test]
    fn bracket_position_four_players() {
        // Two semifinals feeding the final at index 2.
        assert_eq!(bracket_position(4, 0), (0, Some(2), 0));
        assert_eq!(bracket_position(4, 1), (0, Some(2), 1));
        assert_eq!(bracket_position(4, 2), (1, None, 0));
    }

    #[test]
    fn bracket_position_eight_players() {
        // Round 1: indices 0-3 -> semifinals 4-5; semifinals -> final 6.
        assert_eq!(bracket_position(8, 0), (0, Some(4), 0));
        assert_eq!(bracket_position(8, 1), (0, Some(4), 1));
        assert_eq!(bracket_position(8, 2), (0, Some(5), 0));
        assert_eq!(bracket_position(8, 3), (0, Some(5), 1));
        assert_eq!(bracket_position(8, 4), (1, Some(6), 0));
        assert_eq!(bracket_position(8, 5), (1, Some(6), 1));
        assert_eq!(bracket_position(8, 6), (2, None, 0));
    }

    #[test]
    fn bracket_position_next_pointers_stay_in_bounds() {
        for max_players in [2u16, 4, 8, 16, 32, 64, 128, 256] {
            let total = max_players - 1;
            for i in 0..total {
                let (_, next, slot) = bracket_position(max_players, i);
                assert!(slot <= 1);
                match next {
                    Some(n) => {
                        assert!(n < total, "match {i} of {max_players}p points at {n}");
                        assert!(n > i);
                    }
                    None => assert_eq!(i, total - 1, "only the final has no successor"),
                }
            }
        }
    }

    #[test]
    fn required_shards_matches_program_tiers() {
        assert_eq!(required_shards(2), 1);
        assert_eq!(required_shards(4), 1);
        assert_eq!(required_shards(64), 1);
        assert_eq!(required_shards(65), 2);
        assert_eq!(required_shards(128), 2);
        assert_eq!(required_shards(256), 4);
    }

    #[test]
    fn schedule_time_check_ix_builds_expected_accounts_and_data() -> anyhow::Result<()> {
        let program_id = Pubkey::new_unique();
        let payer = Pubkey::new_unique();
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        let game_id = 42u64;
        let interval_ms = 30_000u64;

        let ix = schedule_time_check_ix(&program_id, &payer, game_id, &white, &black, interval_ms)
            .context("build schedule_time_check ix")?;

        assert_eq!(ix.program_id, program_id);
        assert_eq!(ix.accounts.len(), 5);
        assert_eq!(ix.accounts[0].pubkey, payer);
        assert!(ix.accounts[0].is_signer);
        let game_pda =
            Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
        assert_eq!(ix.accounts[1].pubkey, game_pda);
        assert_eq!(ix.accounts[2].pubkey, white);
        assert_eq!(ix.accounts[3].pubkey, black);
        assert_eq!(
            ix.accounts[4].pubkey,
            MAGIC_PROGRAM_PUBKEY
                .parse::<Pubkey>()
                .context("test setup failed")?
        );

        assert_eq!(&ix.data[..8], &anchor_discriminator("schedule_time_check"));
        assert_eq!(&ix.data[8..16], &game_id.to_le_bytes());
        assert_eq!(&ix.data[16..24], &interval_ms.to_le_bytes());
        assert_eq!(
            &ix.data[24..32],
            &(i64::MAX as u64).to_le_bytes(),
            "iterations must be positive and effectively unlimited"
        );
        Ok(())
    }

    #[test]
    fn cancel_time_check_ix_builds_expected_accounts_and_data() -> anyhow::Result<()> {
        let program_id = Pubkey::new_unique();
        let payer = Pubkey::new_unique();
        let game_id = 42u64;

        let ix = cancel_time_check_ix(&program_id, &payer, game_id)
            .context("build cancel_time_check ix")?;

        assert_eq!(ix.program_id, program_id);
        assert_eq!(ix.accounts.len(), 3);
        assert_eq!(ix.accounts[0].pubkey, payer);
        assert!(ix.accounts[0].is_signer);
        let game_pda =
            Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
        assert_eq!(ix.accounts[1].pubkey, game_pda);
        assert_eq!(
            ix.accounts[2].pubkey,
            MAGIC_PROGRAM_PUBKEY
                .parse::<Pubkey>()
                .context("test setup failed")?
        );

        assert_eq!(&ix.data[..8], &anchor_discriminator("cancel_time_check"));
        assert_eq!(
            &ix.data[8..16],
            &game_id.to_le_bytes(),
            "task_id must equal game_id"
        );
        Ok(())
    }
}

#[cfg(test)]
mod swiss_instruction_layout_tests {
    use super::*;

    fn placeholder_count(ix: &Instruction, program_id: &Pubkey) -> usize {
        ix.accounts
            .iter()
            .filter(|meta| meta.pubkey == *program_id)
            .count()
    }

    #[test]
    fn record_swiss_result_matches_the_authority_signed_layout() {
        let program_id = Pubkey::new_unique();
        let (white, black, authority) = (
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        );
        let ix = record_swiss_result_ix(
            &program_id,
            9,
            8,
            2,
            3,
            SWISS_DRAW,
            &white,
            &black,
            &authority,
        );
        // tournament, shards 0-3, authority, white, black
        assert_eq!(ix.accounts.len(), 8);
        assert!(ix.accounts[5].is_signer && ix.accounts[5].pubkey == authority);
        assert_eq!(ix.accounts[6].pubkey, white);
        assert_eq!(ix.accounts[7].pubkey, black);
        assert!(!ix.accounts[6].is_signer && !ix.accounts[7].is_signer);
        assert_eq!(&ix.data[8..16], &9u64.to_le_bytes());
        assert_eq!(&ix.data[16..], &[2, 3, 0, SWISS_DRAW]);
    }

    #[test]
    fn swiss_shards_follow_required_shards() {
        let program_id = Pubkey::new_unique();
        let any = Pubkey::new_unique();
        for (max_players, absent) in [(8u16, 3usize), (64, 3), (100, 2), (128, 2), (200, 0)] {
            let ix = record_swiss_result_ix(&program_id, 1, max_players, 0, 0, 0, &any, &any, &any);
            assert_eq!(
                placeholder_count(&ix, &program_id),
                absent,
                "{max_players} players"
            );
        }
    }

    #[test]
    fn advance_and_complete_encode_their_new_arguments() {
        let program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();

        let advance = advance_round_ix(&program_id, 5, 7, &authority);
        assert_eq!(&advance.data[8..], &[5, 0, 0, 0, 0, 0, 0, 0, 7, 0]);
        assert!(advance.accounts[1].is_signer);

        let places = [Pubkey::new_unique(), Pubkey::new_unique()];
        let complete = complete_swiss_tournament_ix(&program_id, 5, 8, &places, &authority);
        assert_eq!(&complete.data[16..20], &2u32.to_le_bytes());
        assert_eq!(&complete.data[20..52], places[0].as_ref());
        assert_eq!(complete.data.len(), 20 + 64);
        assert_eq!(complete.accounts.len(), 6);
        assert!(complete.accounts[5].is_signer && complete.accounts[5].pubkey == authority);
    }
}
