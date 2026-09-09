use solana_sdk::{instruction::AccountMeta, pubkey::Pubkey};
#[allow(deprecated)]
use solana_system_interface::program as system_program;

fn find_tournament_pda(program_id: &Pubkey, tournament_id: u64) -> (Pubkey, u8) {
    let tid_bytes = tournament_id.to_le_bytes();
    let seeds: &[&[u8]] = &[b"tournament", tid_bytes.as_ref()];
    Pubkey::find_program_address(seeds, program_id)
}

fn required_shards(max_players: u16) -> u8 {
    match max_players {
        0..=64 => 1,
        65..=128 => 2,
        _ => 4,
    }
}

pub fn find_player_profile_pda(program_id: &Pubkey, player: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"profile", player.as_ref()], program_id)
}

pub fn find_tournament_escrow_pda(program_id: &Pubkey, tournament_id: u64) -> (Pubkey, u8) {
    let tid_bytes = tournament_id.to_le_bytes();
    Pubkey::find_program_address(&[b"t_escrow", tid_bytes.as_ref()], program_id)
}

fn find_shard_pda(program_id: &Pubkey, tournament_id: u64, idx: u8) -> Pubkey {
    let tid_bytes = tournament_id.to_le_bytes();
    Pubkey::find_program_address(
        &[b"tourney_players", &[idx], tid_bytes.as_ref()],
        program_id,
    )
    .0
}

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
    let discriminator: [u8; 8] = hasher.finalize()[..8]
        .try_into()
        .expect("sha256 >= 8 bytes");

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
