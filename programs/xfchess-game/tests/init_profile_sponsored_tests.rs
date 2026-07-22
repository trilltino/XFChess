//! Sponsored `init_profile`: XFChess (not the player) fronts the on-chain
//! rent for a player's first profile, so a brand-new wallet with zero SOL
//! can still get an on-chain identity. See
//! docs/plans/identity-implementation-plan.md — design decision #2: no
//! Anchor account-struct change. The backend prepends a plain system
//! transfer to the player for exactly the rent both PDAs need, then the
//! existing, unmodified `init_profile` instruction runs exactly as it does
//! for a self-funded player (its `create_account` CPIs debit `player`
//! regardless of who the *transaction's* fee payer is).
//!
//! This is the first instruction-level (`ProgramTest`) coverage of
//! `init_profile` at all — the only prior test (`profile_session_tests.rs`)
//! exercises the handler helper functions directly, with no PDA creation or
//! CPI involved.
//!
//! Prereq: `cargo build-sbf` (see docs/ER_TESTING.md).

mod common;

use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use common::*;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;
use xfchess_game::state::PlayerProfile;

fn profile_pda(player: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"profile", player.as_ref()], &xfchess_game::ID).0
}

fn username_record_pda(username: &str) -> Pubkey {
    Pubkey::find_program_address(&[b"username", username.as_bytes()], &xfchess_game::ID).0
}

fn init_profile_ix(
    player: &Pubkey,
    username: &str,
    country: &str,
    date_of_birth: i64,
) -> Instruction {
    let accounts = xfchess_game::__client_accounts_init_profile::InitProfile {
        player_profile: profile_pda(player),
        username_record: username_record_pda(username),
        player: *player,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::InitProfile {
        username: username.to_string(),
        country: country.to_string(),
        date_of_birth,
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

#[tokio::test]
async fn sponsored_profile_is_owned_by_player_not_backend() {
    let ctx = start(vec![]).await;

    // The player has ZERO SOL — the whole point of sponsorship.
    let player = Keypair::new();
    // A distinct "backend" fee payer, standing in for the real feepayer pool.
    let backend = Keypair::new();
    let fund_backend =
        system_instruction::transfer(&ctx.payer.pubkey(), &backend.pubkey(), 10_000_000_000);
    let mut fund_tx = Transaction::new_with_payer(&[fund_backend], Some(&ctx.payer.pubkey()));
    fund_tx.sign(&[&ctx.payer], ctx.last_blockhash);
    ctx.banks_client.process_transaction(fund_tx).await.unwrap();

    let profile = profile_pda(&player.pubkey());
    let username = "sponsoredplayer";

    // Same rent computation the backend route performs via
    // getMinimumBalanceForRentExemption before building this transaction.
    let rent = Rent::default();
    let profile_rent = rent.minimum_balance(8 + 257); // discriminator + PlayerProfile::INIT_SPACE
                                                      // The account struct constraint is `space = 8 + UsernameRecord::LEN`, and
                                                      // UsernameRecord::LEN (48) already includes its own discriminator — so
                                                      // the real allocated space is 56 bytes, not 48. Caught by this test
                                                      // failing on-chain with "insufficient lamports" before this fix.
    let username_rent = rent.minimum_balance(8 + 48);
    let transfer_ix = system_instruction::transfer(
        &backend.pubkey(),
        &player.pubkey(),
        profile_rent + username_rent,
    );

    let dob = 631_152_000i64; // 1990-01-01T00:00:00Z — comfortably 18+ under the on-chain age check
    let init_ix = init_profile_ix(&player.pubkey(), username, "GB", dob);

    let mut tx = Transaction::new_with_payer(&[transfer_ix, init_ix], Some(&backend.pubkey()));
    tx.sign(&[&backend, &player], ctx.last_blockhash);
    ctx.banks_client
        .process_transaction(tx)
        .await
        .expect("sponsored init_profile should succeed");

    // The profile PDA belongs to the PLAYER, not the backend that paid for it.
    let acc = ctx
        .banks_client
        .get_account(profile)
        .await
        .unwrap()
        .expect("profile account missing");
    let decoded = PlayerProfile::try_deserialize(&mut &acc.data[..]).unwrap();
    assert_eq!(decoded.authority, player.pubkey());
    assert_eq!(decoded.username, username);

    // The player's wallet, which started at zero, ends back at zero — it
    // received exactly the rent and immediately spent it on its own account
    // creation, never needing to pre-fund anything itself.
    let player_balance = ctx.banks_client.get_balance(player.pubkey()).await.unwrap();
    assert_eq!(
        player_balance, 0,
        "player should not retain any of the sponsored funds"
    );

    // The backend — not the player — is the one who actually paid: it's out
    // the rent it fronted plus the transaction fee.
    let backend_balance = ctx
        .banks_client
        .get_balance(backend.pubkey())
        .await
        .unwrap();
    assert!(
        backend_balance < 10_000_000_000 - (profile_rent + username_rent),
        "backend must have paid the rent it fronted plus the tx fee"
    );
}
