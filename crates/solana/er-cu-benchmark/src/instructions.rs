//! Raw Anchor instruction builders for XFChess benchmark tests.

use anyhow::Result;
use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
#[allow(deprecated)]
use solana_system_interface::program as system_program;

const GAME_SEED: &[u8] = b"game";
const PROFILE_SEED: &[u8] = b"profile";
const USERNAME_SEED: &[u8] = b"username";
const WAGER_ESCROW_SEED: &[u8] = b"escrow";
const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";
const TOURNAMENT_SEED: &[u8] = b"tournament";
const TOURNAMENT_PLAYERS_SEED: &[u8] = b"tourney_players";
const TOURNAMENT_ESCROW_SEED: &[u8] = b"t_escrow";
const TOURNAMENT_MATCH_SEED: &[u8] = b"t_match";
const TREASURY_VAULT_SEED: &[u8] = b"treasury_vault";

/// MagicBlock delegation program ID (same one `delegate_game_ix` parses inline).
const DELEGATION_PROGRAM_ID: &str = "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh";

/// Parses `Game.fees_advanced` directly off raw account bytes fetched from
/// either the base layer or the ER (the field means the same thing in both
/// places — it's only reimbursed from the pot at `finalize_game` on the base
/// layer). Mirrors the offset walk in
/// `backend/src/signing/routes/main.rs::read_game_fee_breakdown` and
/// `tasks::settlement_worker::parse_game_account` — keep all three in sync if
/// `Game`'s field order ever changes. Returns `None` on unexpected layout.
pub fn parse_game_fees_advanced(data: &[u8]) -> Option<u64> {
    let mut o = 8usize; // discriminator
    o += 8; // game_id
    o += 32 + 32; // white + black
    o += 1; // status
    o += 8; // last_move_timestamp
    let bytes: [u8; 8] = data.get(o..o + 8)?.try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

fn anchor_discriminator(fn_name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", fn_name).as_bytes());
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

/// Number of `TournamentPlayersShard` PDAs that actually exist for a
/// tournament size — must mirror `initialize_shards.rs`'s three tiers
/// (`initialize_shards_small`/`_medium`/`initialize_tournament_shards`).
/// Ported from `src/solana/program_interface/instructions.rs::required_shards`
/// (this crate can't depend on the game client).
pub fn required_shards(max_players: u16) -> u8 {
    match max_players {
        0..=64 => 1,
        65..=128 => 2,
        _ => 4,
    }
}

/// AccountMeta for a `TournamentPlayersShard` slot: the real PDA when the
/// shard actually exists for this tournament size, otherwise the program ID
/// — Anchor's convention for a client-supplied `None` in an
/// `Option<Account<'info, T>>` slot (passing the *real but uninitialized* PDA
/// there instead fails with `AccountNotInitialized`/constraint errors, since
/// Anchor tries to deserialize it as `Some`).
fn shard_meta(program_id: &Pubkey, tournament_id: u64, idx: u8, max_players: u16) -> AccountMeta {
    if idx < required_shards(max_players) {
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
}

fn borsh_string(s: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + s.len());
    buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
    buf
}

// ---------------------------------------------------------------------------
// init_profile
// ---------------------------------------------------------------------------
pub fn init_profile_ix(
    program_id: Pubkey,
    player: Pubkey,
    username: String,
    country: String,
    date_of_birth: i64,
) -> Result<Instruction> {
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;
    let username_record_pda =
        Pubkey::find_program_address(&[USERNAME_SEED, username.as_bytes()], &program_id).0;

    let mut data = anchor_discriminator("init_profile").to_vec();
    data.extend(borsh_string(&username));
    data.extend(borsh_string(&country));
    data.extend(date_of_birth.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(player_profile_pda, false),
            AccountMeta::new(username_record_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// create_game
// ---------------------------------------------------------------------------
#[allow(clippy::too_many_arguments)]
pub fn create_game_ix(
    program_id: Pubkey,
    player: Pubkey,
    fee_payer: Pubkey,
    game_id: u64,
    wager_amount: u64,
    match_type: u8,
    platform_fee: u64,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    // create_game(game_id, wager_amount, match_type, platform_fee,
    // base_time_seconds, increment_seconds) — see programs/xfchess-game/src/
    // game_ix/create.rs. This used to take a `country: String` before the
    // program moved to a universal live-rate `platform_fee`; the old encoding
    // here silently produced a malformed instruction (4 zero bytes standing
    // in for half of platform_fee's 8) that would fail to deserialize on-chain.
    let mut data = anchor_discriminator("create_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager_amount.to_le_bytes());
    data.push(match_type);
    data.extend_from_slice(&platform_fee.to_le_bytes());
    data.extend_from_slice(&base_time_seconds.to_le_bytes());
    data.extend_from_slice(&increment_seconds.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new(fee_payer, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// join_game
// ---------------------------------------------------------------------------
pub fn join_game_ix(
    program_id: Pubkey,
    player: Pubkey,
    white_player: Pubkey,
    fee_payer: Pubkey,
    game_id: u64,
) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
    let white_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, white_player.as_ref()], &program_id).0;

    let mut data = anchor_discriminator("join_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(player_profile_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(white_profile_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new(fee_payer, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// record_move
// ---------------------------------------------------------------------------
pub fn record_move_ix(
    program_id: Pubkey,
    session_key: Pubkey,
    wallet_player: Pubkey,
    game_id: u64,
    move_uci: [u8; 5],
    next_board: [u8; 68],
    nonce: u64,
    signature: Option<Vec<u8>>,
) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let session_pda = Pubkey::find_program_address(
        &[
            b"session_delegation",
            &game_id.to_le_bytes(),
            wallet_player.as_ref(),
        ],
        &program_id,
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
    data.push(0);

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(session_key, true),
            AccountMeta::new_readonly(session_pda, false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// finalize_game
// ---------------------------------------------------------------------------
pub fn finalize_game_ix(
    program_id: Pubkey,
    game_id: u64,
    white_pubkey: Pubkey,
    black_pubkey: Pubkey,
    fee_payer: Pubkey,
) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let white_profile =
        Pubkey::find_program_address(&[PROFILE_SEED, white_pubkey.as_ref()], &program_id).0;
    let black_profile =
        Pubkey::find_program_address(&[PROFILE_SEED, black_pubkey.as_ref()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
    let treasury_vault = Pubkey::find_program_address(&[TREASURY_VAULT_SEED], &program_id).0;

    let mut data = anchor_discriminator("finalize_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(white_profile, false),
            AccountMeta::new(black_profile, false),
            AccountMeta::new(white_pubkey, false),
            AccountMeta::new(black_pubkey, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(treasury_vault, false),
            // fee_payer is a plain SystemAccount on-chain (game_ix/finalize.rs:39-40),
            // not a Signer — marking it `true` here worked only because the
            // current call site happens to pass the tx's own fee payer.
            AccountMeta::new(fee_payer, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// authorize_session_key
// ---------------------------------------------------------------------------
pub fn authorize_session_key_ix(
    program_id: Pubkey,
    player: Pubkey,
    game_id: u64,
    session_pubkey: Pubkey,
) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[
            SESSION_DELEGATION_SEED,
            &game_id.to_le_bytes(),
            player.as_ref(),
        ],
        &program_id,
    )
    .0;

    // authorize_session_key(game_id, session_pubkey) — see lib.rs:443-447.
    // A trailing `duration_seconds` used to be encoded here too; Anchor's
    // deserializer silently ignores extra trailing bytes so it never caused
    // a failure, but the value had no effect — session lifetime is fixed at
    // 2 hours in delegation_ix/session.rs:28.
    let mut data = anchor_discriminator("authorize_session_key").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(session_pubkey.as_ref());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(session_delegation_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// delegate_game (ER-specific)
// ---------------------------------------------------------------------------
pub fn delegate_game_ix(
    program_id: Pubkey,
    game_pda: Pubkey,
    payer: Pubkey,
    fee_payer: Pubkey,
    game_id: u64,
    valid_until: i64,
) -> Result<Instruction> {
    let buffer_pda = {
        let pda = ephemeral_rollups_sdk::pda::delegate_buffer_pda_from_delegated_account_and_owner_program(
            &game_pda.to_bytes().into(),
            &program_id.to_bytes().into(),
        );
        Pubkey::new_from_array(pda.to_bytes())
    };
    let delegation_record = {
        let pda = ephemeral_rollups_sdk::pda::delegation_record_pda_from_delegated_account(
            &game_pda.to_bytes().into(),
        );
        Pubkey::new_from_array(pda.to_bytes())
    };
    let delegation_metadata = {
        let pda = ephemeral_rollups_sdk::pda::delegation_metadata_pda_from_delegated_account(
            &game_pda.to_bytes().into(),
        );
        Pubkey::new_from_array(pda.to_bytes())
    };
    let delegation_program: Pubkey = "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh".parse()?;

    let mut data = anchor_discriminator("delegate_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&valid_until.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(program_id, false),
            AccountMeta::new(buffer_pda, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new_readonly(delegation_program, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(fee_payer, true),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// undelegate_game (ER-specific)
// ---------------------------------------------------------------------------
pub fn undelegate_game_ix(
    program_id: Pubkey,
    game_pda: Pubkey,
    payer: Pubkey,
    game_id: u64,
) -> Result<Instruction> {
    let magic_context: Pubkey = "MagicContext1111111111111111111111111111111".parse()?;
    let magic_program: Pubkey = "Magic11111111111111111111111111111111111111".parse()?;

    let mut data = anchor_discriminator("undelegate_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(payer, true),
            AccountMeta::new(magic_context, false),
            AccountMeta::new_readonly(magic_program, false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// schedule_time_check (ER crank)
// ---------------------------------------------------------------------------
pub fn schedule_time_check_ix(
    program_id: Pubkey,
    game_pda: Pubkey,
    payer: Pubkey,
    white: Pubkey,
    black: Pubkey,
    game_id: u64,
    interval_ms: u64,
) -> Result<Instruction> {
    let magic_program: Pubkey = "Magic11111111111111111111111111111111111111".parse()?;

    // ScheduleTimeCheckArgs { task_id: u64, check_interval_millis: u64, iterations: u64 }
    let mut data = anchor_discriminator("schedule_time_check").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&interval_ms.to_le_bytes());
    let iterations = 0u64; // unlimited
    data.extend_from_slice(&iterations.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(white, false),
            AccountMeta::new_readonly(black, false),
            AccountMeta::new_readonly(magic_program, false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// crank_time_check (ER crank)
// ---------------------------------------------------------------------------
pub fn crank_time_check_ix(
    program_id: Pubkey,
    game_pda: Pubkey,
    white: Pubkey,
    black: Pubkey,
) -> Result<Instruction> {
    let data = anchor_discriminator("crank_time_check").to_vec();

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(white, false),
            AccountMeta::new_readonly(black, false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// cancel_time_check (ER crank)
// ---------------------------------------------------------------------------
pub fn cancel_time_check_ix(
    program_id: Pubkey,
    payer: Pubkey,
    game_id: u64,
) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let magic_program: Pubkey = "Magic11111111111111111111111111111111111111".parse()?;

    let mut data = anchor_discriminator("cancel_time_check").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes()); // task_id

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(magic_program, false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// request_force_undelegate / force_undelegate_after_timeout / recover_stuck_delegation
// (ER-unavailability recovery drill — see recovery_drill.rs)
// ---------------------------------------------------------------------------

/// Starts the delegation program's ~60min no-validator-cooperation-needed
/// undelegation countdown. `payer` must be the same key that funded the
/// original `delegate_game` call for this game (checked against
/// `delegation_metadata.rent_payer` on-chain).
pub fn request_force_undelegate_ix(
    program_id: Pubkey,
    game_id: u64,
    payer: Pubkey,
) -> Result<Instruction> {
    use ephemeral_rollups_sdk::pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        undelegation_request_pda_from_delegated_account,
    };

    let delegation_program_id: Pubkey = DELEGATION_PROGRAM_ID.parse()?;
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;

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
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(program_id, false),
            AccountMeta::new(undelegation_request, false),
            AccountMeta::new_readonly(delegation_record, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new_readonly(delegation_program_id, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

/// Completes a forced undelegation once `request_force_undelegate_ix`'s
/// ~60min window has elapsed. **Data-loss warning:** wipes the `Game` PDA to
/// zero bytes (delegation program's own design, not a bug) — follow up with
/// `recover_stuck_delegation_ix` to release the escrow. `payer` must be the
/// same key passed to `request_force_undelegate_ix`.
pub fn force_undelegate_after_timeout_ix(
    program_id: Pubkey,
    game_id: u64,
    payer: Pubkey,
) -> Result<Instruction> {
    use ephemeral_rollups_sdk::pda::{
        commit_record_pda_from_delegated_account, commit_state_pda_from_delegated_account,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        undelegation_request_pda_from_delegated_account,
    };

    let delegation_program_id: Pubkey = DELEGATION_PROGRAM_ID.parse()?;
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;

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
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(program_id, false),
            AccountMeta::new(undelegation_request, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new(payer, false),
            AccountMeta::new(commit_state, false),
            AccountMeta::new(commit_record, false),
            AccountMeta::new(payer, false), // commit_reimbursement placeholder
            AccountMeta::new_readonly(delegation_program_id, false),
        ],
        data,
    })
}

/// Releases wager escrow (50/50 split) from a `Game` PDA that
/// `force_undelegate_after_timeout_ix` wiped to zero bytes. `dispute_authority`
/// must sign — same key `resolve_dispute`/`claim_stale_dispute` trust.
pub fn recover_stuck_delegation_ix(
    program_id: Pubkey,
    game_id: u64,
    white: Pubkey,
    black: Pubkey,
    dispute_authority: Pubkey,
) -> Instruction {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
    let treasury_vault = Pubkey::find_program_address(&[TREASURY_VAULT_SEED], &program_id).0;

    let mut data = anchor_discriminator("recover_stuck_delegation").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(treasury_vault, false),
            AccountMeta::new(white, false),
            AccountMeta::new(black, false),
            AccountMeta::new_readonly(dispute_authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// initialize_tournament
// ---------------------------------------------------------------------------
/// TournamentType enum discriminants (state/tournament.rs's
/// `TournamentType::Swiss { rounds: u8 }` / `TournamentType::SingleElimination`).
const TOURNAMENT_TYPE_SWISS: u8 = 0;
const TOURNAMENT_TYPE_SINGLE_ELIMINATION: u8 = 1;

/// Wrapped SOL mint — used as a placeholder "USDC" mint for benchmarks.
const WRAPPED_SOL_MINT: &str = "So11111111111111111111111111111111111111112";

/// `Some(rounds)` builds a Swiss tournament; `None` builds single-elimination
/// (which has no `rounds` field — the bracket size determines round count).
pub fn initialize_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    name: &str,
    entry_fee: u64,
    max_players: u16,
    rounds: Option<u8>,
    elo_min: u32,
    elo_max: u32,
    min_players: u16,
    prize_shares: [u16; 10],
    platform_fee: u64,
    winner_takes_all: bool,
    host_treasury: Pubkey,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let usdc_prize_escrow_authority = Pubkey::find_program_address(
        &[b"t_usdc_prize", &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let usdc_mint: Pubkey = WRAPPED_SOL_MINT.parse()?;
    let token_program: Pubkey = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".parse()?;
    let associated_token_program: Pubkey =
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".parse()?;
    let usdc_prize_escrow = Pubkey::find_program_address(
        &[
            &usdc_prize_escrow_authority.to_bytes(),
            &token_program.to_bytes(),
            &usdc_mint.to_bytes(),
        ],
        &associated_token_program,
    )
    .0;

    // Build instruction data matching handler signature
    let mut data = anchor_discriminator("initialize_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend(borsh_string(name));
    data.extend_from_slice(&entry_fee.to_le_bytes());
    data.extend_from_slice(&max_players.to_le_bytes());
    match rounds {
        Some(rounds) => {
            // TournamentType::Swiss { rounds }
            data.push(TOURNAMENT_TYPE_SWISS);
            data.push(rounds);
        }
        None => {
            // TournamentType::SingleElimination (no fields)
            data.push(TOURNAMENT_TYPE_SINGLE_ELIMINATION);
        }
    }
    data.extend_from_slice(&elo_min.to_le_bytes());
    data.extend_from_slice(&elo_max.to_le_bytes());
    data.extend_from_slice(&min_players.to_le_bytes());
    for share in &prize_shares {
        data.extend_from_slice(&share.to_le_bytes());
    }
    data.extend_from_slice(&platform_fee.to_le_bytes());
    data.push(if winner_takes_all { 1 } else { 0 });
    data.extend_from_slice(&host_treasury.to_bytes());
    // Option<Pubkey> — Some(wrapped_sol_mint)
    data.push(1);
    data.extend_from_slice(&usdc_mint.to_bytes());
    data.extend_from_slice(&base_time_seconds.to_le_bytes());
    data.extend_from_slice(&increment_seconds.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(usdc_prize_escrow_authority, false),
            AccountMeta::new(usdc_prize_escrow, false),
            AccountMeta::new_readonly(usdc_mint, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// initialize_shards_small / _medium / initialize_tournament_shards (large)
//
// The on-chain program has three separate shard-init instructions tiered by
// `max_players` (initialize_shards.rs's doc comment: ≤64 -> 1 shard, ≤128 ->
// 2 shards, 256 -> 4 shards) — each is its own instruction with its own
// Accounts struct and its own `max_players` range constraint (mapped to
// `GameErrorCode::InvalidGameStatus`), not one instruction that always
// creates 4. Calling the large-tier instruction (the only one that used to be
// wired up here) for anything smaller than 256 players fails that
// `tournament.max_players == 256` constraint every time.
// ---------------------------------------------------------------------------

fn shard_pda(program_id: &Pubkey, tournament_id: u64, idx: u8) -> Pubkey {
    Pubkey::find_program_address(
        &[
            TOURNAMENT_PLAYERS_SEED,
            &[idx],
            &tournament_id.to_le_bytes(),
        ],
        program_id,
    )
    .0
}

/// Small tier: ≤64 players, 1 shard.
pub fn initialize_shards_small_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("initialize_shards_small").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 0), false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

/// Medium tier: 65-128 players, 2 shards.
pub fn initialize_shards_medium_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("initialize_shards_medium").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 0), false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 1), false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

/// Large tier: exactly 256 players, 4 shards.
pub fn initialize_tournament_shards_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("initialize_tournament_shards").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            // `tournament` is read-only here (initialize_shards.rs:133-139) — the
            // shards are what get initialized, not the tournament record itself.
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 0), false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 1), false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 2), false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 3), false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

/// Picks the right shard-init instruction for `max_players` — small (≤64),
/// medium (65-128), or large (exactly 256). Callers no longer need to know
/// about the tiering themselves.
pub fn initialize_shards_for_size_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    max_players: u16,
) -> Result<Instruction> {
    match required_shards(max_players) {
        1 => initialize_shards_small_ix(program_id, authority, tournament_id),
        2 => initialize_shards_medium_ix(program_id, authority, tournament_id),
        _ => initialize_tournament_shards_ix(program_id, authority, tournament_id),
    }
}

// ---------------------------------------------------------------------------
// initialize_tournament_escrow
// ---------------------------------------------------------------------------
pub fn initialize_tournament_escrow_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("initialize_tournament_escrow").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// register_player
// ---------------------------------------------------------------------------
/// `max_players` determines which shards actually exist (see
/// `required_shards`) — shards beyond that are passed as the program-ID
/// `None` sentinel via `shard_meta`, matching `RegisterPlayer`'s
/// `Option<Box<Account<...>>>` fields for shards 1-3
/// (tournament_ix/registration/register.rs:50-70).
pub fn register_player_ix(
    program_id: Pubkey,
    player: Pubkey,
    host_treasury: Pubkey,
    tournament_id: u64,
    max_players: u16,
    elo: u32,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;

    let mut data = anchor_discriminator("register_player").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&elo.to_le_bytes());

    // RegisterPlayer is 10 accounts, in this exact order — see
    // programs/xfchess-game/src/tournament_ix/registration/register.rs:19-79.
    // `host_treasury` is a plain `UncheckedAccount` (constrained ==
    // tournament.host_treasury), not a signer — there used to be a second
    // `platform_treasury_vault` account here too, which doesn't exist on the
    // real struct and pushed every following account (just `system_program`)
    // one slot out of alignment.
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(player_profile_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(host_treasury, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// start_tournament
// ---------------------------------------------------------------------------
/// `max_players` determines which shards actually exist (see
/// `required_shards`) — shards beyond that are passed as the program-ID
/// `None` sentinel via `shard_meta`, matching `StartTournament`'s
/// `Option<Account<...>>` fields for shards 1-3
/// (tournament_ix/lifecycle/start.rs:27-48).
pub fn start_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    host_treasury: Pubkey,
    tournament_id: u64,
    max_players: u16,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("start_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    // StartTournament sweeps entry fees from escrow_pda to host_treasury —
    // both were missing here, which bound `authority`'s signature to
    // escrow_pda's slot and dropped host_treasury/authority/system_program
    // each one slot early. See tournament_ix/lifecycle/start.rs:12-66.
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(host_treasury, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// record_match_result
// ---------------------------------------------------------------------------
pub fn record_match_result_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    match_index: u16,
    winner: Pubkey,
    loser: Pubkey,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let tournament_match_pda = Pubkey::find_program_address(
        &[
            TOURNAMENT_MATCH_SEED,
            &tournament_id.to_le_bytes(),
            &match_index.to_le_bytes(),
        ],
        &program_id,
    )
    .0;

    // record_match_result(tournament_id, match_index: u16, winner, loser) —
    // see tournament_ix/matches/record_result.rs:11,32-37. `match_index` was
    // encoded as a single byte (u8) here, shifting `winner`/`loser` one byte
    // short; `tournament_match` (the account the result is actually written
    // to) was missing entirely; and the 4th arg is `loser`, not a game PDA.
    let mut data = anchor_discriminator("record_match_result").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&match_index.to_le_bytes());
    data.extend_from_slice(winner.as_ref());
    data.extend_from_slice(loser.as_ref());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(tournament_match_pda, false),
            AccountMeta::new(authority, true),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// authorize_tournament_session
// ---------------------------------------------------------------------------
/// `max_players` determines which shards actually exist (see
/// `required_shards`) — shards beyond that are passed as the program-ID
/// `None` sentinel via `shard_meta`, matching
/// `AuthorizeTournamentSessionCtx`'s `Option<Account<...>>` fields for
/// shards 1-3.
#[allow(clippy::too_many_arguments)]
pub fn authorize_tournament_session_ix(
    program_id: Pubkey,
    tournament_id: u64,
    max_players: u16,
    player: Pubkey,
    session_key: Pubkey,
    spending_limit: u64,
    max_wager: u64,
    duration_secs: i64,
    deposit_lamports: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[
            b"tournament_session",
            &tournament_id.to_le_bytes(),
            player.as_ref(),
        ],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("authorize_tournament_session").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    // AuthorizeTournamentSessionArgs Borsh serialization
    data.extend_from_slice(session_key.as_ref());
    data.push(1); // Option::Some for duration_secs
    data.extend_from_slice(&duration_secs.to_le_bytes());
    data.push(1); // Option::Some for spending_limit
    data.extend_from_slice(&spending_limit.to_le_bytes());
    data.push(1); // Option::Some for max_wager
    data.extend_from_slice(&max_wager.to_le_bytes());
    data.extend_from_slice(&deposit_lamports.to_le_bytes()); // deposit_lamports to pre-fund session rent

    Ok(Instruction {
        program_id,
        accounts: vec![
            // tournament + all 4 shards are read-only on-chain
            // (authorize_tournament_session.rs) — only session_delegation is written.
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new_readonly(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(session_delegation_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// session_create_game
// ---------------------------------------------------------------------------
#[allow(clippy::too_many_arguments)]
pub fn session_create_game_ix(
    program_id: Pubkey,
    tournament_id: u64,
    max_players: u16,
    game_id: u64,
    white_session: Pubkey,
    white_player: Pubkey,
    wager: u64,
    platform_fee: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[
            b"tournament_session",
            &tournament_id.to_le_bytes(),
            white_player.as_ref(),
        ],
        &program_id,
    )
    .0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    // session_create_game(tournament_id, game_id, wager_amount, match_type,
    // platform_fee, base_time_seconds, increment_seconds) — see
    // programs/xfchess-game/src/tournament_ix/session/session_create_game.rs.
    // This used to take a `country: String`; the old encoding here (4 zero
    // bytes standing in for half of platform_fee's 8) would fail to
    // deserialize on-chain today.
    let mut data = anchor_discriminator("session_create_game").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager.to_le_bytes());
    // MatchType::Free = 0
    data.push(0);
    data.extend_from_slice(&platform_fee.to_le_bytes());
    data.extend_from_slice(&60u64.to_le_bytes()); // base_time_seconds — 1 min, synthetic instant-move benchmark
    data.extend_from_slice(&0u16.to_le_bytes()); // increment_seconds

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(session_delegation_pda, false),
            AccountMeta::new_readonly(white_session, true),
            AccountMeta::new_readonly(white_player, false),
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// session_join_game
// ---------------------------------------------------------------------------
/// `max_players` determines which shards actually exist — see
/// `session_create_game_ix`'s identical note.
pub fn session_join_game_ix(
    program_id: Pubkey,
    tournament_id: u64,
    max_players: u16,
    game_id: u64,
    session_key: Pubkey,
    player: Pubkey,
    white_player: Pubkey,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[
            b"tournament_session",
            &tournament_id.to_le_bytes(),
            player.as_ref(),
        ],
        &program_id,
    )
    .0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
    let white_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, white_player.as_ref()], &program_id).0;
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;

    let mut data = anchor_discriminator("session_join_game").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            // tournament + all 4 shards are read-only on-chain (session_join_game.rs).
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new_readonly(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(session_delegation_pda, false),
            AccountMeta::new_readonly(session_key, true),
            AccountMeta::new_readonly(player, false),
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(white_profile_pda, false),
            AccountMeta::new_readonly(player_profile_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// record_swiss_result
// ---------------------------------------------------------------------------
/// `max_players` determines which shards actually exist (see
/// `required_shards`) — shards beyond that are passed as the program-ID
/// `None` sentinel via `shard_meta`, matching `RecordSwissResult`'s
/// `Option<Account<'info, TournamentPlayersShard>>` fields for shards 1-3.
pub fn record_swiss_result_ix(
    program_id: Pubkey,
    tournament_id: u64,
    max_players: u16,
    round: u8,
    board: u16,
    result_variant: u8, // 0 = Win, 1 = Loss, 2 = Draw (SwissMatchResult)
    player: Pubkey,
    opponent: Pubkey,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("record_swiss_result").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.push(round);
    data.extend_from_slice(&board.to_le_bytes());
    data.push(result_variant);

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(opponent, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// close_tournament — sweeps residual escrow to the treasury and marks the
// tournament Closed. Only callable once every funded prize place has already
// been claimed via `distribute_tournament_prizes`/`claim_tournament_prize`
// (tournament_ix/lifecycle/close_tournament.rs:33-70); this instruction never
// pays anyone directly and doesn't read `remaining_accounts` at all — a
// `prize_recipients` param used to be appended here as extra account metas,
// which the program simply ignored.
// ---------------------------------------------------------------------------
pub fn close_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let prize_escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let treasury_vault = Pubkey::find_program_address(&[TREASURY_VAULT_SEED], &program_id).0;

    let mut data = anchor_discriminator("close_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(prize_escrow_pda, false),
            AccountMeta::new(treasury_vault, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(authority, true),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// fund_sol_prize — locks a guaranteed SOL prize pool in escrow. Must be sent
// before the first player registers (tournament_ix/prizes/fund_sol_prize.rs:
// rejects once tournament.num_registered_players > 0 or prize_pool != 0).
// ---------------------------------------------------------------------------
pub fn fund_sol_prize_ix(
    program_id: Pubkey,
    operator: Pubkey,
    tournament_id: u64,
    amount_lamports: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("fund_sol_prize").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&amount_lamports.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// advance_round — permissionless Swiss round-advancement crank
// (tournament_ix/matches/advance_round.rs). Requires every board in the
// current round to have already reported via record_swiss_result.
// ---------------------------------------------------------------------------
pub fn advance_round_ix(
    program_id: Pubkey,
    cranker: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("advance_round").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(cranker, true),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// complete_swiss_tournament — permissionless crank that sorts final Swiss
// standings and marks the tournament Completed once `advance_round` has
// pushed `current_round` to `total_rounds`
// (tournament_ix/matches/complete_swiss.rs). `max_players` determines which
// shards actually exist (see `required_shards`); shards beyond that are
// passed as the program-ID `None` sentinel via `shard_meta`, matching
// `CompleteSwissTournament`'s `Option<Account<...>>` fields for shards 1-3.
// ---------------------------------------------------------------------------
pub fn complete_swiss_tournament_ix(
    program_id: Pubkey,
    cranker: Pubkey,
    tournament_id: u64,
    max_players: u16,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("complete_swiss_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new_readonly(cranker, true),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// distribute_tournament_prizes — permissionless crank that pushes each
// recorded place's SOL share directly to their wallet
// (tournament_ix/prizes/distribute.rs). `winners` are passed as writable
// remaining accounts in any order; places whose wallet is absent are skipped.
// ---------------------------------------------------------------------------
pub fn distribute_tournament_prizes_ix(
    program_id: Pubkey,
    cranker: Pubkey,
    tournament_id: u64,
    winners: &[Pubkey],
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("distribute_tournament_prizes").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    let mut accounts = vec![
        AccountMeta::new(tournament_pda, false),
        AccountMeta::new(escrow_pda, false),
        AccountMeta::new_readonly(cranker, true),
    ];
    accounts.extend(winners.iter().map(|w| AccountMeta::new(*w, false)));

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

// ---------------------------------------------------------------------------
// initialize_match — creates one single-elimination bracket slot
// (tournament_ix/matches/initialize_match.rs). Authority-only (the backend,
// or here, the benchmark's own admin/master keypair).
// ---------------------------------------------------------------------------
#[allow(clippy::too_many_arguments)]
pub fn initialize_match_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    match_index: u16,
    round: u8,
    player_white: Option<Pubkey>,
    player_black: Option<Pubkey>,
    next_match_for_winner: Option<u16>,
    next_match_slot: u8,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let match_pda = Pubkey::find_program_address(
        &[
            TOURNAMENT_MATCH_SEED,
            &tournament_id.to_le_bytes(),
            &match_index.to_le_bytes(),
        ],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("initialize_match").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&match_index.to_le_bytes());
    data.push(round);
    for player in [player_white, player_black] {
        match player {
            Some(pk) => {
                data.push(1);
                data.extend_from_slice(pk.as_ref());
            }
            None => data.push(0),
        }
    }
    match next_match_for_winner {
        Some(n) => {
            data.push(1);
            data.extend_from_slice(&n.to_le_bytes());
        }
        None => data.push(0),
    }
    data.push(next_match_slot);

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(match_pda, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// advance_winner — copies a completed match's winner into their slot in the
// next round's match (tournament_ix/matches/record_result.rs's
// `handler_advance_winner`).
// ---------------------------------------------------------------------------
pub fn advance_winner_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    source_match_index: u16,
    target_match_index: u16,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let match_pda = |idx: u16| {
        Pubkey::find_program_address(
            &[
                TOURNAMENT_MATCH_SEED,
                &tournament_id.to_le_bytes(),
                &idx.to_le_bytes(),
            ],
            &program_id,
        )
        .0
    };

    let mut data = anchor_discriminator("advance_winner").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&source_match_index.to_le_bytes());
    data.extend_from_slice(&target_match_index.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(match_pda(source_match_index), false),
            AccountMeta::new(match_pda(target_match_index), false),
            AccountMeta::new(authority, true),
        ],
        data,
    })
}

/// Computes a match's (round, next_match_for_winner, next_match_slot) in the
/// linear single-elimination bracket layout used on-chain: round-1 matches
/// occupy indices `0..max_players/2`, each later round follows, and the final
/// is always the last index. Ported from
/// `src/solana/program_interface/instructions.rs` (used by the game client's
/// own tournament e2e driver) since this benchmark crate has no dependency on
/// the game client.
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

// ---------------------------------------------------------------------------
// resign
// ---------------------------------------------------------------------------
pub fn resign_game_ix(program_id: Pubkey, game_id: u64, player: Pubkey) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = anchor_discriminator("resign").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    // ResignGame is exactly `game` (mut) + `player` (read-only Signer) — see
    // programs/xfchess-game/src/game_ix/resign.rs. The previous 6-account
    // version (escrow_pda, white, black, system_program that don't belong
    // here) bound `player`'s signature to slot 2 (escrow_pda) instead of
    // itself, failing on-chain with `AccountNotSigner`.
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(player, true),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// authorize_global_session / global_create_game / global_join_game
//
// The persistent, non-tournament session flow: one `authorize_global_session`
// covers up to 200 games, with `global_create_game`/`global_join_game` paying
// rent + wager out of the `GlobalSessionDelegation` PDA vault instead of the
// player's wallet. Current on-chain layout (see programs/xfchess-game/src/
// account_ix/global_session_ix.rs and game_ix/global_create.rs,
// global_join.rs) — `platform_fee: u64`, not the older `country: String` seen
// in `create_game_ix`/`session_create_game_ix` above.
// ---------------------------------------------------------------------------

fn global_session_pda(program_id: &Pubkey, player: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"global_session", player.as_ref()], program_id).0
}

#[allow(clippy::too_many_arguments)]
pub fn authorize_global_session_ix(
    program_id: Pubkey,
    player: Pubkey,
    session_key: Pubkey,
    duration_secs: Option<i64>,
    spending_limit: Option<u64>,
    max_wager: Option<u64>,
    games: Option<u16>,
    deposit_lamports: u64,
) -> Result<Instruction> {
    let session_pda = global_session_pda(&program_id, &player);

    let mut data = anchor_discriminator("authorize_global_session").to_vec();
    // AuthorizeGlobalSessionArgs (Borsh)
    data.extend_from_slice(session_key.as_ref());
    match duration_secs {
        Some(v) => {
            data.push(1);
            data.extend_from_slice(&v.to_le_bytes());
        }
        None => data.push(0),
    }
    match spending_limit {
        Some(v) => {
            data.push(1);
            data.extend_from_slice(&v.to_le_bytes());
        }
        None => data.push(0),
    }
    match max_wager {
        Some(v) => {
            data.push(1);
            data.extend_from_slice(&v.to_le_bytes());
        }
        None => data.push(0),
    }
    match games {
        Some(v) => {
            data.push(1);
            data.extend_from_slice(&v.to_le_bytes());
        }
        None => data.push(0),
    }
    data.extend_from_slice(&deposit_lamports.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(session_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn global_create_game_ix(
    program_id: Pubkey,
    session_signer: Pubkey,
    player: Pubkey,
    game_id: u64,
    wager_amount: u64,
    match_type: u8,
    platform_fee: u64,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<Instruction> {
    let session_pda = global_session_pda(&program_id, &player);
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = anchor_discriminator("global_create_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager_amount.to_le_bytes());
    data.push(match_type);
    data.extend_from_slice(&platform_fee.to_le_bytes());
    data.extend_from_slice(&base_time_seconds.to_le_bytes());
    data.extend_from_slice(&increment_seconds.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(session_pda, false),
            AccountMeta::new_readonly(session_signer, true),
            AccountMeta::new_readonly(player, false),
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn global_join_game_ix(
    program_id: Pubkey,
    session_signer: Pubkey,
    player: Pubkey,
    white_player: Pubkey,
    game_id: u64,
) -> Result<Instruction> {
    let session_pda = global_session_pda(&program_id, &player);
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;
    let white_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, white_player.as_ref()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = anchor_discriminator("global_join_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(session_pda, false),
            AccountMeta::new_readonly(session_signer, true),
            AccountMeta::new_readonly(player, false),
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(player_profile_pda, false),
            AccountMeta::new_readonly(white_profile_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}
