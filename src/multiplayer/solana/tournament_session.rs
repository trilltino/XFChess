//! Tournament registration instruction building for client-side use.

#[allow(deprecated)]
use solana_system_interface::program as system_program;
use solana_sdk::{instruction::AccountMeta, pubkey::Pubkey};

/// Derive the `Tournament` PDA.
fn find_tournament_pda(program_id: &Pubkey, tournament_id: u64) -> (Pubkey, u8) {
    let tid_bytes = tournament_id.to_le_bytes();
    let seeds: &[&[u8]] = &[b"tournament", tid_bytes.as_ref()];
    Pubkey::find_program_address(seeds, program_id)
}

/// Number of `TournamentPlayersShard` PDAs actually initialized on-chain for
/// a tournament of this size (each shard holds up to 64 players). Must match
/// `programs/xfchess-game/src/tournament_ix/shards.rs::required_shards`.
fn required_shards(max_players: u16) -> u8 {
    match max_players {
        0..=64 => 1,
        65..=128 => 2,
        _ => 4,
    }
}

/// Derive the `PlayerProfile` PDA — must already exist (created at
/// first-time wallet setup / `init_profile`) before a player can register
/// for any tournament.
pub fn find_player_profile_pda(program_id: &Pubkey, player: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"profile", player.as_ref()], program_id)
}

/// Derive the tournament escrow PDA (holds the guaranteed prize plus, during
/// Registration, entry-fee deposits).
pub fn find_tournament_escrow_pda(program_id: &Pubkey, tournament_id: u64) -> (Pubkey, u8) {
    let tid_bytes = tournament_id.to_le_bytes();
    Pubkey::find_program_address(&[b"t_escrow", tid_bytes.as_ref()], program_id)
}

/// Derive shard `idx`'s PDA (0-3).
fn find_shard_pda(program_id: &Pubkey, tournament_id: u64, idx: u8) -> Pubkey {
    let tid_bytes = tournament_id.to_le_bytes();
    Pubkey::find_program_address(&[b"tourney_players", &[idx], tid_bytes.as_ref()], program_id).0
}

/// Build a real `register_player` instruction: this is the transaction that
/// actually deposits the entry fee into escrow and adds the player to their
/// tournament shard on-chain. Matches
/// `programs/xfchess-game/src/tournament_ix/registration/register.rs`'s
/// `RegisterPlayer` accounts struct exactly — shards past what `max_players`
/// requires are passed as the program ID, Anchor's sentinel for `None` on an
/// `Option<Account<..>>` field (same convention the backend's own
/// `cancel_tournament_ix`/`start_tournament_ix` use).
pub fn build_register_player_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    max_players: u16,
    player_pubkey: &Pubkey,
    host_treasury: &Pubkey,
    elo: u32,
) -> solana_sdk::instruction::Instruction {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(b"global:register_player");
    let discriminator: [u8; 8] = hasher.finalize()[..8].try_into().expect("sha256 >= 8 bytes");

    let mut data = Vec::with_capacity(20);
    data.extend_from_slice(&discriminator);
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&elo.to_le_bytes());

    let (tournament_pda, _) = find_tournament_pda(program_id, tournament_id);
    let (player_profile_pda, _) = find_player_profile_pda(program_id, player_pubkey);
    let (escrow_pda, _) = find_tournament_escrow_pda(program_id, tournament_id);
    let present = required_shards(max_players);
    let shard_meta = |idx: u8| {
        if idx < present {
            AccountMeta::new(find_shard_pda(program_id, tournament_id, idx), false)
        } else {
            AccountMeta::new_readonly(*program_id, false)
        }
    };

    let accounts = vec![
        AccountMeta::new(tournament_pda, false),
        AccountMeta::new_readonly(player_profile_pda, false),
        AccountMeta::new(*player_pubkey, true),
        AccountMeta::new(escrow_pda, false),
        shard_meta(0),
        shard_meta(1),
        shard_meta(2),
        shard_meta(3),
        AccountMeta::new(*host_treasury, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    solana_sdk::instruction::Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

