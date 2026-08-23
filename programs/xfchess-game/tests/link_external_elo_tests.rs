//! Proves `LichessUsernameRecord` actually enforces one-Lichess-username-
//! per-wallet through the real compiled program: before this fix,
//! `link_external_elo` had no on-chain uniqueness protection at all, unlike
//! local in-game usernames (`UsernameRecord`) — the same Lichess handle
//! (and the external ELO it seeds) could be attached to any number of
//! different wallets' profiles. See `LichessUsernameRecord`'s own doc
//! comment for the full writeup.
//!
//! Prereq: `cargo build-sbf` (see docs/ER_TESTING.md).

mod common;

use anchor_lang::{InstructionData, Space, ToAccountMetas};
use common::*;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::PlayerProfile;

fn lichess_username_record_pda(username: &str) -> Pubkey {
    Pubkey::find_program_address(
        &[b"lichess_username", username.as_bytes()],
        &xfchess_game::ID,
    )
    .0
}

fn profile_account(authority: Pubkey) -> (Pubkey, solana_sdk::account::Account) {
    let pda = profile_pda(&authority).0;
    let profile = PlayerProfile {
        authority,
        ..Default::default()
    };
    (
        pda,
        program_account(&profile, 8 + PlayerProfile::INIT_SPACE),
    )
}

#[allow(clippy::too_many_arguments)]
fn link_external_elo_ix(
    player: Pubkey,
    link_authority: Pubkey,
    username: &str,
    blitz: u32,
    rapid: u32,
    bullet: u32,
) -> Instruction {
    let accounts = xfchess_game::__client_accounts_link_external_elo::LinkExternalElo {
        player_profile: profile_pda(&player).0,
        player,
        lichess_username_record: lichess_username_record_pda(username),
        link_authority,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::LinkExternalElo {
        username: username.to_string(),
        blitz_rating: blitz,
        rapid_rating: rapid,
        bullet_rating: bullet,
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

/// Loads the real `link_authority` keypair from the gitignored keyfile —
/// same pattern `treasury_tests.rs`/`tournament_registration_e2e_tests.rs`
/// already use for their own authorities. The instruction hard-constrains
/// `address = link_authority::ID` (`constants.rs`), so no other key can
/// sign this successfully; tests skip gracefully when the file isn't
/// present (e.g. a fresh clone/CI) rather than failing.
fn link_authority_keypair() -> Option<Keypair> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../keys/link_authority.json"
    );
    read_keypair_file(path).ok()
}

#[tokio::test]
async fn link_external_elo_rejects_a_second_wallet_claiming_the_same_lichess_username() {
    let Some(link_authority) = link_authority_keypair() else {
        eprintln!("skip: keys/link_authority.json not present");
        return;
    };
    let player_a = Keypair::new().pubkey();
    let player_b = Keypair::new().pubkey();
    let username = "MagnusCarlsenFan42";

    let (profile_a_pda, profile_a_data) = profile_account(player_a);
    let (profile_b_pda, profile_b_data) = profile_account(player_b);

    let mut ctx = start(vec![
        (profile_a_pda, profile_a_data),
        (profile_b_pda, profile_b_data),
        (link_authority.pubkey(), system_account(1_000_000_000)),
    ])
    .await;

    let ix_a = link_external_elo_ix(
        player_a,
        link_authority.pubkey(),
        username,
        1500,
        1500,
        1500,
    );
    send(&mut ctx, ix_a, &[&link_authority])
        .await
        .expect("first link for a fresh Lichess username should succeed");

    let profile_a_after = fetch_profile(&mut ctx, &player_a).await;
    assert_eq!(profile_a_after.lichess_username, username);

    // A DIFFERENT wallet claiming the SAME Lichess username must be rejected.
    let ix_b = link_external_elo_ix(
        player_b,
        link_authority.pubkey(),
        username,
        1400,
        1400,
        1400,
    );
    let err = send(&mut ctx, ix_b, &[&link_authority])
        .await
        .expect_err("a second wallet must not be able to claim an already-linked Lichess username");
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::UsernameTaken)),
        "expected UsernameTaken, got {err:?}"
    );

    // Player B's profile must NOT have been mutated by the rejected attempt.
    let profile_b_after = fetch_profile(&mut ctx, &player_b).await;
    assert_ne!(
        profile_b_after.lichess_username, username,
        "a rejected claim must not have written the Lichess username onto the wrong profile"
    );
}

#[tokio::test]
async fn link_external_elo_allows_the_same_wallet_to_re_sync_its_own_username() {
    let Some(link_authority) = link_authority_keypair() else {
        eprintln!("skip: keys/link_authority.json not present");
        return;
    };
    let player_a = Keypair::new().pubkey();
    let username = "SameWalletResync";

    let (profile_a_pda, profile_a_data) = profile_account(player_a);

    let mut ctx = start(vec![
        (profile_a_pda, profile_a_data),
        (link_authority.pubkey(), system_account(1_000_000_000)),
    ])
    .await;

    let ix_first = link_external_elo_ix(
        player_a,
        link_authority.pubkey(),
        username,
        1500,
        1500,
        1500,
    );
    send(&mut ctx, ix_first, &[&link_authority])
        .await
        .expect("first link should succeed");

    // Re-syncing updated ratings for the SAME wallet + SAME username must
    // still succeed (idempotent re-link), not be rejected as "taken".
    let ix_second = link_external_elo_ix(
        player_a,
        link_authority.pubkey(),
        username,
        1600,
        1550,
        1580,
    );
    send(&mut ctx, ix_second, &[&link_authority])
        .await
        .expect("the same wallet re-linking its own already-claimed username must succeed");

    let profile_after = fetch_profile(&mut ctx, &player_a).await;
    assert_eq!(profile_after.lichess_username, username);
    assert_eq!(profile_after.lichess_blitz, 160_000); // centiscale: 1600 * 100
}
