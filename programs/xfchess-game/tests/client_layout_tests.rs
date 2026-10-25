//! Compile the real desktop builders and compare their exact v2 wire interfaces.
#[allow(
    dead_code,
    clippy::too_many_arguments,
    reason = "Imports the complete client module with instruction-ABI-shaped builders"
)]
#[path = "../../../src/solana/program_interface/instructions.rs"]
mod desktop;
use anchor_lang::{InstructionData, ToAccountMetas};
use solana_sdk::pubkey::Pubkey;

#[test]
fn desktop_create_v2_matches_generated_arguments_and_account_order() -> anyhow::Result<()> {
    let program = xfchess_game::ID;
    let player = Pubkey::new_unique();
    let payer = Pubkey::new_unique();
    let ix = desktop::create_game_ix(program, player, payer, 42, 10_000_000, 1, 0, 300, 5)?;
    let expected = xfchess_game::instruction::CreateGameV2 {
        game_id: 42,
        wager_amount: 10_000_000,
        match_type: xfchess_game::state::MatchType::Rated,
        platform_fee: 0,
        base_time_seconds: 300,
        increment_seconds: 5,
    }
    .data();
    assert_eq!(ix.data, expected);
    let pda = |seed: &[u8]| Pubkey::find_program_address(&[seed, &42u64.to_le_bytes()], &program).0;
    let expected = xfchess_game::accounts::CreateGameV2 {
        legacy: xfchess_game::accounts::CreateGame {
            game: pda(b"game"),
            escrow_pda: pda(b"escrow"),
            player,
            fee_payer: payer,
            system_program: solana_system_interface::program::ID,
        },
        recovery_identity: pda(b"recovery_identity"),
    }
    .to_account_metas(None);
    assert_eq!(ix.accounts, expected);
    Ok(())
}

#[test]
fn desktop_join_v2_matches_generated_arguments_and_account_order() -> anyhow::Result<()> {
    let program = xfchess_game::ID;
    let player = Pubkey::new_unique();
    let white = Pubkey::new_unique();
    let payer = Pubkey::new_unique();
    let ix = desktop::join_game_ix(program, player, white, payer, 42)?;
    assert_eq!(
        ix.data,
        xfchess_game::instruction::JoinGameV2 { game_id: 42 }.data()
    );
    let pda = |seed: &[u8]| Pubkey::find_program_address(&[seed, &42u64.to_le_bytes()], &program).0;
    let profile =
        |wallet: &Pubkey| Pubkey::find_program_address(&[b"profile", wallet.as_ref()], &program).0;
    let expected = xfchess_game::accounts::JoinGameV2 {
        legacy: xfchess_game::accounts::JoinGame {
            game: pda(b"game"),
            player_profile: profile(&player),
            escrow_pda: pda(b"escrow"),
            white_profile: profile(&white),
            player,
            fee_payer: payer,
            system_program: solana_system_interface::program::ID,
        },
        recovery_identity: pda(b"recovery_identity"),
    }
    .to_account_metas(None);
    assert_eq!(ix.accounts, expected);
    Ok(())
}
