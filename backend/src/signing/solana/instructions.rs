//! Solana instruction builders for XFChess program instructions.

use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use super::{
    GAME_SEED, MAGIC_CONTEXT_PUBKEY, MAGIC_PROGRAM_PUBKEY, PLATFORM_FEE_VAULT_SEED, PROFILE_SEED,
    SESSION_DELEGATION_SEED, TOURNAMENT_SEED, WAGER_ESCROW_SEED,
};

const TOURNAMENT_ESCROW_SEED: &[u8] = b"t_escrow";
const TOURNAMENT_PLAYERS_SEED: &[u8] = b"tourney_players";
const TOURNAMENT_MATCH_SEED: &[u8] = b"t_match";
const TOURNAMENT_USDC_PRIZE_SEED: &[u8] = b"t_usdc_prize";

/// Computes the Anchor discriminator for a given instruction name.
fn anchor_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", name));
    hasher.finalize()[..8]
        .try_into()
        .expect("SHA256 hash should be at least 8 bytes")
}

/// Borsh-encodes a string (length prefix + bytes).
fn borsh_string(s: &str) -> Vec<u8> {
    let mut v = (s.len() as u32).to_le_bytes().to_vec();
    v.extend_from_slice(s.as_bytes());
    v
}

/// Builds a `record_move` instruction for the Execution Rollup.
///
/// Records a chess move on the ER with optional signature for replay protection.
pub fn record_move_ix(
    program_id: &Pubkey,
    session_pubkey: &Pubkey,
    wallet_pubkey: &Pubkey,
    game_id: u64,
    move_uci: [u8; 5],
    next_board: [u8; 68],
    nonce: u64,
    signature: Option<Vec<u8>>,
    parent_nonce: Option<u64>,
) -> anyhow::Result<Instruction> {
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

/// Builds an `undelegate_game` instruction for the ER.
///
/// Commits the ER game state back to devnet and releases the delegated account.
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

/// Builds a `finalize_game` instruction for devnet.
///
/// Sets game.status = Finished, pays out the wager escrow, and updates ELO.
/// Winner: Some("white") | Some("black") | None (draw).
///
/// `fee_payer` is the ephemeral rollups relayer pubkey that gets reimbursed from escrow.
pub fn finalize_game_ix(
    program_id: &Pubkey,
    game_id: u64,
    white: &Pubkey,
    black: &Pubkey,
    winner: Option<&str>,
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

    // GameResult Borsh encoding: 1 = Winner(Pubkey), 2 = Draw
    match winner {
        Some("white") => {
            data.push(1);
            data.extend_from_slice(white.as_ref());
        }
        Some("black") => {
            data.push(1);
            data.extend_from_slice(black.as_ref());
        }
        _ => {
            data.push(2);
        }
    }

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

/// Builds a `link_external_elo` instruction for devnet.
///
/// Links a verified Lichess account to a player profile.
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
            AccountMeta::new(*link_authority, true),
        ],
        data,
    }
}

/// Builds a `verify_profile` instruction for devnet.
///
/// Marks a player as KYC-verified on-chain.
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

/// Builds a `claim_fees` instruction for the platform fee vault.
///
/// Transfers accumulated fees from the PlatformFeeVault to the host wallet.
/// This instruction is permissionless - anyone can trigger it.
///
/// # Arguments
/// * `program_id` - The XFChess program ID
/// * `caller` - The account triggering the claim (fee-payer)
/// * `host_wallet` - The wallet that receives the claimed fees (must match vault.host_wallet)
pub fn claim_fees_ix(program_id: &Pubkey, caller: &Pubkey, host_wallet: &Pubkey) -> Instruction {
    let fee_vault_pda = Pubkey::find_program_address(&[PLATFORM_FEE_VAULT_SEED], program_id).0;

    let data = anchor_discriminator("claim_fees").to_vec();

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*caller, true),
            AccountMeta::new(fee_vault_pda, false),
            AccountMeta::new(*host_wallet, false),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data,
    }
}

/// Builds a `withdraw_treasury` instruction.
///
/// Moves `amount` lamports from the system-owned platform treasury vault
/// (seeds `[b"treasury_vault"]`) to `destination`. Must be signed by
/// `authority`, which the program constrains to `treasury_authority::ID`.
/// Account order mirrors `WithdrawTreasury` in the program:
/// treasury_vault, authority (signer), destination, system_program.
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

/// Builds a `leave_tournament` instruction for devnet.
///
/// Removes a player from the tournament and triggers a refund.
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

/// Builds an `initialize_tournament` instruction for devnet.
pub fn initialize_tournament_ix(
    program_id: &Pubkey,
    admin: &Pubkey,
    tournament_id: u64,
    name: &str,
    entry_fee: u64,
    platform_fee: u64,
    max_players: u16,
    tournament_type: u8, // 0 = SingleElimination, 1 = Swiss
    swiss_rounds: u8,
    elo_min: u32,
    elo_max: u32,
    min_players: u16,
    prize_shares: [u16; 10],
    winner_takes_all: bool,
    host_treasury: &Pubkey,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;

    let mut data = anchor_discriminator("initialize_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend(borsh_string(name));
    data.extend_from_slice(&entry_fee.to_le_bytes());
    data.extend_from_slice(&max_players.to_le_bytes());

    // TournamentType Borsh encoding — the on-chain enum declares
    // `Swiss { rounds }` as variant 0 and `SingleElimination` as variant 1,
    // the reverse of this function's 0=SingleElimination param convention.
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
    let token_program: Pubkey = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        .parse()
        .expect("spl token id");
    let associated_token_program: Pubkey = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        .parse()
        .expect("spl associated token id");

    Instruction {
        program_id: *program_id,
        // Account order must match `InitializeTournament`: tournament,
        // usdc_prize_escrow_authority, usdc_prize_escrow (None), usdc_mint
        // (None), authority, token_program, associated_token_program,
        // system_program. SOL-only tournaments pass the program ID for the two
        // optional USDC accounts (Anchor's `None` marker).
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

/// Builds an `initialize_tournament_escrow` instruction.
/// Must be called after `initialize_tournament` and before `register_player`.
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
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
        ],
        data,
    }
}

/// Builds the correct `initialize_shards` instruction variant based on `max_players`.
///
/// - ≤ 64  → `initialize_shards_small`  (1 shard PDA)
/// - ≤ 128 → `initialize_shards_medium` (2 shard PDAs)
/// - 256   → `initialize_shards_large`  (4 shard PDAs)
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
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
            ]
        } else if max_players <= 128 {
            vec![
                AccountMeta::new_readonly(tournament_pda, false),
                AccountMeta::new(shard(0), false),
                AccountMeta::new(shard(1), false),
                AccountMeta::new(*authority, true),
                AccountMeta::new_readonly(solana_system_interface::program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
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
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
            ]
        },
        data,
    }
}

/// Number of TournamentPlayersShard PDAs that exist for a tournament size.
/// Must mirror `shards::required_shards` in the on-chain program.
pub fn required_shards(max_players: u16) -> u8 {
    match max_players {
        0..=64 => 1,
        65..=128 => 2,
        _ => 4,
    }
}

/// Computes a match's (round, next_match_for_winner, next_match_slot) in the
/// linear single-elimination layout used by the store and the on-chain program:
/// round-1 matches occupy indices 0..P/2, each later round follows, and the
/// final is the last index (`total_matches - 1`).
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

/// Builds a `start_tournament` instruction.
/// Locks registration, seeds players for bracket generation, and sweeps the
/// entry-fee deposits from the tournament escrow to `host_treasury` (operator
/// revenue — the guaranteed prize stays locked in escrow).
/// Shard PDAs that don't exist for this tournament size are passed as the
/// program ID (Anchor's `None` marker for optional accounts).
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

/// Builds a `cancel_tournament` instruction.
/// Halts a Registration- or Active-phase tournament: refunds entry fees to
/// `players` (from escrow during Registration, from `host_treasury` if the
/// tournament already started and swept fees there) and returns the
/// guaranteed SOL prize to the operator. `players` must be passed in the
/// same order they were registered on-chain — the handler matches each
/// remaining account positionally against the shard-recorded player list.
/// Shard PDAs absent for this tournament size are passed as the program ID
/// (Anchor's `None` marker), matching `start_tournament_ix`. USDC prize
/// accounts are also passed as the program ID — this builder only supports
/// SOL-only tournaments, matching `initialize_tournament_ix`'s default.
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
    let token_program: Pubkey = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        .parse()
        .expect("spl token id");

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

/// Builds a `fund_sol_prize` instruction.
/// Locks the guaranteed SOL prize in the tournament escrow PDA. Must be sent
/// before the first player registers — the program rejects it afterwards, and
/// rejects registrations on paid tournaments until a prize is funded.
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

/// Builds a `distribute_tournament_prizes` instruction.
///
/// Push-based payout: pays every unclaimed place its SOL share directly, so
/// winners never have to sign a claim. `winners` are passed as writable
/// remaining accounts; the program only pays wallets that match the places
/// recorded on the Tournament account.
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

/// Builds an `initialize_match` instruction for a single bracket slot.
/// `round`: 0-indexed. `next_match_for_winner`: None for the final.
/// `next_match_slot`: 0 = white side, 1 = black side in the next match.
pub fn initialize_match_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    match_index: u16,
    round: u8,
    player_white: Option<&Pubkey>,
    player_black: Option<&Pubkey>,
    next_match_for_winner: Option<u16>,
    next_match_slot: u8,
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

/// Builds a `record_match_result` instruction (VPS-signed).
/// Resolves the on-chain `TournamentMatch` PDA and advances the bracket.
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

/// Builds an `advance_winner` instruction (VPS-signed).
/// Copies the completed source match's winner into their slot in the target
/// match so the next round can start.
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

/// Builds a `claim_tournament_prize` instruction (player-signed).
/// Pulls the claimant's share from the SOL escrow PDA. The program validates
/// that the claimant matches a finishing position and prevents double-claim.
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
                "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
                    .parse()
                    .expect("spl token id"),
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
}
