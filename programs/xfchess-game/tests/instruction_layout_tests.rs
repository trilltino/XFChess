//! Locks the borsh layout of `initialize_tournament` args against the
//! hand-built encodings in the external builders (backend
//! `signing/solana/instructions.rs` and the game client's
//! `solana/program_interface/instructions.rs`). Those builders construct
//! instruction data manually, so an enum reorder or added arg on-chain
//! silently breaks them with InstructionDidNotDeserialize (error 102) —
//! this test catches the drift at `cargo test` time.

use anchor_lang::prelude::Pubkey;
use anchor_lang::InstructionData;
use xfchess_game::state::TournamentType;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Replicates the client/backend builders' byte construction for a
/// SOL-only single-elimination tournament.
fn hand_built(discriminator: &[u8], host_treasury: &Pubkey) -> Vec<u8> {
    let name = "E2E Cup 2p";
    let mut hand = discriminator.to_vec();
    hand.extend_from_slice(&42u64.to_le_bytes()); // tournament_id
    hand.extend_from_slice(&(name.len() as u32).to_le_bytes());
    hand.extend_from_slice(name.as_bytes());
    hand.extend_from_slice(&1_000_000u64.to_le_bytes()); // entry_fee
    hand.extend_from_slice(&2u16.to_le_bytes()); // max_players
    hand.push(1); // TournamentType::SingleElimination (Swiss { rounds } is variant 0)
    hand.extend_from_slice(&0u32.to_le_bytes()); // elo_min
    hand.extend_from_slice(&u32::MAX.to_le_bytes()); // elo_max
    hand.extend_from_slice(&2u16.to_le_bytes()); // min_players
    for share in [7000u16, 3000, 0, 0, 0, 0, 0, 0, 0, 0] {
        hand.extend_from_slice(&share.to_le_bytes());
    }
    hand.extend_from_slice(&0u64.to_le_bytes()); // platform_fee
    hand.push(0); // winner_takes_all = false
    hand.extend_from_slice(host_treasury.as_ref());
    hand.push(0); // usdc_mint = None
    hand.extend_from_slice(&600u64.to_le_bytes()); // base_time_seconds
    hand.extend_from_slice(&5u16.to_le_bytes()); // increment_seconds
    hand
}

/// Exact bytes captured from a failing devnet run (tournament 1784206847);
/// deserializes them the way the on-chain dispatch does.
#[test]
fn initialize_tournament_deserializes_captured_devnet_bytes() {
    let hex = "4bda5650317f9bbaffd5586a000000000a0000004532452043757020327040420f000000000002000100000000ffffffff0200581bb80b00000000000000000000000000000000000000000000000000a3ad5c77f852da8b757c967a26f5fa0b3757d5dce0a9ea9a53b060e4741a37680058020000000000000500";
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    assert_eq!(bytes.len(), 123);

    use anchor_lang::Discriminator;
    assert_eq!(
        &bytes[..8],
        xfchess_game::instruction::InitializeTournament::DISCRIMINATOR,
        "discriminator mismatch"
    );

    let args = anchor_lang::AnchorDeserialize::deserialize(&mut &bytes[8..])
        .map(|ix: xfchess_game::instruction::InitializeTournament| ix);
    match args {
        Ok(ix) => {
            assert_eq!(ix.tournament_id, 1784206847);
            assert_eq!(ix.max_players, 2);
            assert_eq!(ix.tournament_type, TournamentType::SingleElimination);
            assert_eq!(ix.increment_seconds, 5);
        }
        Err(e) => panic!("captured devnet bytes failed to deserialize: {e}"),
    }
}

#[test]
fn initialize_tournament_matches_hand_built_layout() {
    let host_treasury = Pubkey::new_unique();
    let expected = xfchess_game::instruction::InitializeTournament {
        tournament_id: 42,
        name: "E2E Cup 2p".to_string(),
        entry_fee: 1_000_000,
        max_players: 2,
        tournament_type: TournamentType::SingleElimination,
        elo_min: 0,
        elo_max: u32::MAX,
        min_players: 2,
        prize_shares: [7000, 3000, 0, 0, 0, 0, 0, 0, 0, 0],
        platform_fee: 0,
        winner_takes_all: false,
        host_treasury,
        usdc_mint: None,
        base_time_seconds: 600,
        increment_seconds: 5,
    }
    .data();

    let hand = hand_built(&expected[..8], &host_treasury);
    assert_eq!(
        hex(&expected),
        hex(&hand),
        "anchor layout (left) diverged from hand-built builder layout (right)"
    );
}
