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
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
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
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
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
    player: &Pubkey,
    host_treasury: &Pubkey,
) -> Instruction {
    let tournament_pda =
        Pubkey::find_program_address(&[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], program_id)
            .0;

    let data = anchor_discriminator("leave_tournament").to_vec();

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(*player, true),
            AccountMeta::new(*host_treasury, true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
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

    // TournamentType Borsh encoding
    data.push(tournament_type);
    if tournament_type == 1 {
        // Swiss
        data.push(swiss_rounds);
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

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(*admin, true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
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
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
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
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
            ]
        } else if max_players <= 128 {
            vec![
                AccountMeta::new_readonly(tournament_pda, false),
                AccountMeta::new(shard(0), false),
                AccountMeta::new(shard(1), false),
                AccountMeta::new(*authority, true),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
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
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
            ]
        },
        data,
    }
}

/// Builds a `start_tournament` instruction.
/// Locks registration, seeds players for bracket generation, and sweeps the
/// entry-fee deposits from the tournament escrow to `host_treasury` (operator
/// revenue — the guaranteed prize stays locked in escrow).
/// All 4 shard PDAs are always passed; the program ignores extra ones.
pub fn start_tournament_ix(
    program_id: &Pubkey,
    tournament_id: u64,
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

    let mut data = anchor_discriminator("start_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(shard(0), false),
            AccountMeta::new_readonly(shard(1), false),
            AccountMeta::new_readonly(shard(2), false),
            AccountMeta::new_readonly(shard(3), false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(*host_treasury, false),
            AccountMeta::new(*authority, true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
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
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
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
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
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
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data,
    }
}
