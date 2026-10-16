use super::super::common::*;
use anyhow::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_system_interface::program as system_program;

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

    // record_match_result encodes match_index as u16, then winner and loser.
    // Include the writable tournament_match account.
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

/// `SwissMatchResult` Borsh tags.
pub const SWISS_WHITE_WIN: u8 = 0;
pub const SWISS_BLACK_WIN: u8 = 1;
pub const SWISS_DRAW: u8 = 2;
/// A bye: one extra board slot after the round's real boards, bye player as both sides.
pub const SWISS_BYE: u8 = 3;

/// Authority-signed: records one board (0-based round and board) of the current round.
#[allow(clippy::too_many_arguments)]
pub fn record_swiss_result_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    max_players: u16,
    round: u8,
    board: u16,
    result_variant: u8,
    white: Pubkey,
    black: Pubkey,
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
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new_readonly(white, false),
            AccountMeta::new_readonly(black, false),
        ],
        data,
    })
}

/// Authority-signed: seats a pairing in an Active Game with both players' per-game
/// move sessions pointing at `session_key`.
#[allow(clippy::too_many_arguments)] // Mirrors the instruction's accounts and payload.
pub fn create_tournament_game_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    max_players: u16,
    game_id: u64,
    white: Pubkey,
    black: Pubkey,
    session_key: Pubkey,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let session_pda = |player: &Pubkey| {
        Pubkey::find_program_address(
            &[
                SESSION_DELEGATION_SEED,
                &game_id.to_le_bytes(),
                player.as_ref(),
            ],
            &program_id,
        )
        .0
    };

    let mut data = anchor_discriminator("create_tournament_game").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(session_key.as_ref());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new_readonly(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(game_pda, false),
            AccountMeta::new(session_pda(&white), false),
            AccountMeta::new(session_pda(&black), false),
            AccountMeta::new_readonly(white, false),
            AccountMeta::new_readonly(black, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

/// Authority-signed: closes the current Swiss round once all `boards_in_round` board
/// slots (pairings plus any bye) are recorded.
pub fn advance_round_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    boards_in_round: u16,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("advance_round").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&boards_in_round.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(authority, true),
        ],
        data,
    })
}

/// Authority-signed: completes Swiss with the final top places (tiebreak order from
/// the off-chain pairing engine, checked against on-chain scores). Optional absent
/// shards use the program-ID None sentinel.
pub fn complete_swiss_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    max_players: u16,
    places: &[Pubkey],
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("complete_swiss_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&(places.len() as u32).to_le_bytes());
    for place in places {
        data.extend_from_slice(place.as_ref());
    }

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new_readonly(authority, true),
        ],
        data,
    })
}

// Authority-only creation of a single-elimination bracket slot.
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

// Advance a completed match winner to their slot in the next round.
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
