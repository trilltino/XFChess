use sha2::{Digest, Sha256};
use solana_sdk::{instruction::AccountMeta, pubkey::Pubkey};

pub(super) const GAME_SEED: &[u8] = b"game";
pub(super) const PROFILE_SEED: &[u8] = b"profile";
pub(super) const USERNAME_SEED: &[u8] = b"username";
pub(super) const WAGER_ESCROW_SEED: &[u8] = b"escrow";
pub(super) const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";
pub(super) const TOURNAMENT_SEED: &[u8] = b"tournament";
pub(super) const TOURNAMENT_PLAYERS_SEED: &[u8] = b"tourney_players";
pub(super) const TOURNAMENT_ESCROW_SEED: &[u8] = b"t_escrow";
pub(super) const TOURNAMENT_MATCH_SEED: &[u8] = b"t_match";
pub(super) const TREASURY_VAULT_SEED: &[u8] = b"treasury_vault";

pub(super) const DELEGATION_PROGRAM_ID: &str = "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh";

pub fn parse_game_fees_advanced(data: &[u8]) -> Option<u64> {
    let mut o = 8usize; // discriminator
    o += 8; // game_id
    o += 32 + 32; // white + black
    o += 1; // status
    o += 8; // last_move_timestamp
    let bytes: [u8; 8] = data.get(o..o + 8)?.try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

pub(super) fn anchor_discriminator(fn_name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", fn_name).as_bytes());
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

pub fn required_shards(max_players: u16) -> u8 {
    match max_players {
        0..=64 => 1,
        65..=128 => 2,
        _ => 4,
    }
}

pub(super) fn shard_meta(
    program_id: &Pubkey,
    tournament_id: u64,
    idx: u8,
    max_players: u16,
) -> AccountMeta {
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

pub(super) fn borsh_string(s: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + s.len());
    buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
    buf
}

pub(super) fn shard_pda(program_id: &Pubkey, tournament_id: u64, idx: u8) -> Pubkey {
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

pub(super) const TOURNAMENT_TYPE_SWISS: u8 = 0;
pub(super) const TOURNAMENT_TYPE_SINGLE_ELIMINATION: u8 = 1;
pub(super) const WRAPPED_SOL_MINT: &str = "So11111111111111111111111111111111111111112";
