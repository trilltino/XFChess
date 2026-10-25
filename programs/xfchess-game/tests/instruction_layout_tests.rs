use anchor_lang::prelude::Pubkey;
use anchor_lang::InstructionData;
use xfchess_game::state::TournamentType;

#[test]
fn recovery_versioned_creation_keeps_argument_bytes_and_account_prefix() {
    use anchor_lang::ToAccountMetas;
    let player = Pubkey::new_unique();
    let payer = Pubkey::new_unique();
    let game = Pubkey::new_unique();
    let escrow = Pubkey::new_unique();
    let record = Pubkey::new_unique();
    let legacy = xfchess_game::accounts::CreateGame {
        game,
        escrow_pda: escrow,
        player,
        fee_payer: payer,
        system_program: anchor_lang::system_program::ID,
    };
    let mut expected = legacy.to_account_metas(None);
    expected.push(anchor_lang::solana_program::instruction::AccountMeta::new(
        record, false,
    ));
    let versioned = xfchess_game::accounts::CreateGameV2 {
        legacy,
        recovery_identity: record,
    };
    assert_eq!(versioned.to_account_metas(None), expected);
    let old = xfchess_game::instruction::CreateGame {
        game_id: 42,
        wager_amount: 10_000_000,
        match_type: xfchess_game::state::MatchType::Rated,
        platform_fee: 0,
        base_time_seconds: 300,
        increment_seconds: 5,
    }
    .data();
    let new = xfchess_game::instruction::CreateGameV2 {
        game_id: 42,
        wager_amount: 10_000_000,
        match_type: xfchess_game::state::MatchType::Rated,
        platform_fee: 0,
        base_time_seconds: 300,
        increment_seconds: 5,
    }
    .data();
    assert_ne!(&old[..8], &new[..8]);
    assert_eq!(&old[8..], &new[8..]);
}

#[test]
fn recovery_errors_append_after_existing_error_codes() {
    use xfchess_game::errors::GameErrorCode;
    assert_eq!(
        GameErrorCode::RecoveryIdentityUnavailable as u32,
        GameErrorCode::PassPolicyMismatch as u32 + 1
    );
    assert_eq!(
        GameErrorCode::RecoveryIdentityMismatch as u32,
        GameErrorCode::RecoveryIdentityUnavailable as u32 + 1
    );
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

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

#[test]
fn initialize_tournament_deserializes_captured_devnet_bytes() -> anyhow::Result<()> {
    let hex = "4bda5650317f9bbaffd5586a000000000a0000004532452043757020327040420f000000000002000100000000ffffffff0200581bb80b00000000000000000000000000000000000000000000000000a3ad5c77f852da8b757c967a26f5fa0b3757d5dce0a9ea9a53b060e4741a37680058020000000000000500";
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect::<Result<_, _>>()?;
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
    Ok(())
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
