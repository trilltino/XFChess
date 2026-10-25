#[path = "common/agave.rs"]
pub mod agave;
use anyhow::Context as _;
pub mod common;

use anchor_lang::{AccountDeserialize, InstructionData, Space, ToAccountMetas};
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
async fn sponsored_profile_is_owned_by_player_not_backend() -> anyhow::Result<()> {
    let ctx = start(vec![]).await;

    // The player has ZERO SOL â€” the whole point of sponsorship.
    let player = Keypair::new();
    // A distinct "backend" fee payer, standing in for the real feepayer pool.
    let backend = Keypair::new();
    let fund_backend =
        system_instruction::transfer(&ctx.payer.pubkey(), &backend.pubkey(), 10_000_000_000);
    let mut fund_tx = Transaction::new_with_payer(&[fund_backend], Some(&ctx.payer.pubkey()));
    fund_tx.sign(&[&ctx.payer], ctx.last_blockhash);
    ctx.banks_client
        .process_transaction(fund_tx)
        .await
        .context("required operation value")?;

    let profile = profile_pda(&player.pubkey());
    let username = "sponsoredplayer";

    // Compute sponsored-profile rent from the program's actual INIT_SPACE constants.
    let rent = Rent::default();
    let profile_rent = rent.minimum_balance(8 + PlayerProfile::INIT_SPACE); // UsernameRecord allocation is 8 + LEN; LEN already includes its discriminator.
    let username_rent = rent.minimum_balance(8 + 48);
    let transfer_ix = system_instruction::transfer(
        &backend.pubkey(),
        &player.pubkey(),
        profile_rent + username_rent,
    );

    let dob = 631_152_000i64; // 1990-01-01T00:00:00Z â€” comfortably 18+ under the on-chain age check
    let init_ix = init_profile_ix(&player.pubkey(), username, "GB", dob);

    let mut tx = Transaction::new_with_payer(&[transfer_ix, init_ix], Some(&backend.pubkey()));
    tx.sign(&[&backend, &player], ctx.last_blockhash);
    ctx.banks_client
        .process_transaction(tx)
        .await
        .context("sponsored init_profile should succeed")?;

    // The profile PDA belongs to the PLAYER, not the backend that paid for it.
    let acc = ctx
        .banks_client
        .get_account(profile)
        .await
        .context("required operation value")?
        .context("profile account missing")?;
    let decoded =
        PlayerProfile::try_deserialize(&mut &acc.data[..]).context("required operation value")?;
    assert_eq!(decoded.authority, player.pubkey());
    assert_eq!(decoded.username, username);

    // The zero-funded player receives and spends exactly the account-creation rent.
    let player_balance = ctx
        .banks_client
        .get_balance(player.pubkey())
        .await
        .context("required operation value")?;
    assert_eq!(
        player_balance, 0,
        "player should not retain any of the sponsored funds"
    );

    // The backend â€” not the player â€” is the one who actually paid: it's out
    // the rent it fronted plus the transaction fee.
    let backend_balance = ctx
        .banks_client
        .get_balance(backend.pubkey())
        .await
        .context("required operation value")?;
    assert!(
        backend_balance < 10_000_000_000 - (profile_rent + username_rent),
        "backend must have paid the rent it fronted plus the tx fee"
    );
    Ok(())
}

#[tokio::test]
async fn profile_substitution_cannot_overwrite_game_or_create_username() -> anyhow::Result<()> {
    use xfchess_game::state::GameStatus;
    let player = Keypair::new();
    let (target, original) = game_account(
        42,
        player.pubkey(),
        Pubkey::new_unique(),
        start_board(),
        1,
        0,
        GameStatus::Active,
    )?;
    let mut ctx = start(vec![
        (target, original.clone()),
        (player.pubkey(), system_account(1_000_000_000)),
    ])
    .await;
    let username = "secureplayer";
    let mut ix = init_profile_ix(&player.pubkey(), username, "GB", 631_152_000);
    ix.accounts[0].pubkey = target;
    let err = send(&mut ctx, ix, &[&player])
        .await
        .err()
        .context("substitution must fail")?;
    assert_eq!(
        custom_code(&err),
        Some(anchor_lang::error::ErrorCode::ConstraintSeeds as u32)
    );
    let after = ctx
        .banks_client
        .get_account(target)
        .await?
        .context("Game must remain")?;
    assert_eq!(after, original);
    assert!(ctx
        .banks_client
        .get_account(username_record_pda(username))
        .await?
        .is_none());
    assert_eq!(
        ctx.banks_client.get_balance(player.pubkey()).await?,
        1_000_000_000
    );
    Ok(())
}

fn existing_profile(authority: Pubkey) -> PlayerProfile {
    PlayerProfile {
        authority,
        wins: 5,
        games_played: 9,
        elo_rating: 1_750.0,
        elo_bullet: 1_700.0,
        elo_blitz: 1_720.0,
        elo_rapid: 1_740.0,
        is_verified: true,
        created_at: 77,
        ..PlayerProfile::default()
    }
}

#[tokio::test]
async fn foreign_owned_profile_with_genuine_discriminator_is_rejected() -> anyhow::Result<()> {
    let player = Keypair::new();
    let target = profile_pda(&player.pubkey());
    let mut forged = program_account(
        &existing_profile(player.pubkey()),
        8 + PlayerProfile::INIT_SPACE,
    )?;
    forged.owner = Pubkey::new_unique();
    let mut ctx = start(vec![
        (target, forged.clone()),
        (player.pubkey(), system_account(1_000_000_000)),
    ])
    .await;
    let username = "foreignowned";
    let ix = init_profile_ix(&player.pubkey(), username, "GB", 631_152_000);
    let err = send(&mut ctx, ix, &[&player])
        .await
        .err()
        .context("foreign-owned profile must be rejected")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(xfchess_game::errors::GameErrorCode::UnauthorizedAccess))
    );
    assert_eq!(
        ctx.banks_client
            .get_account(target)
            .await?
            .context("account must remain")?,
        forged
    );
    assert!(ctx
        .banks_client
        .get_account(username_record_pda(username))
        .await?
        .is_none());
    assert_eq!(
        ctx.banks_client.get_balance(player.pubkey()).await?,
        1_000_000_000
    );
    Ok(())
}

#[tokio::test]
async fn wrong_player_cannot_initialise_another_players_profile() -> anyhow::Result<()> {
    let victim = Keypair::new();
    let attacker = Keypair::new();
    let target = profile_pda(&victim.pubkey());
    let original = program_account(
        &existing_profile(victim.pubkey()),
        8 + PlayerProfile::INIT_SPACE,
    )?;
    let mut ctx = start(vec![
        (target, original.clone()),
        (attacker.pubkey(), system_account(1_000_000_000)),
    ])
    .await;
    let username = "attackername";
    let mut ix = init_profile_ix(&attacker.pubkey(), username, "GB", 631_152_000);
    ix.accounts[0].pubkey = target;
    let err = send(&mut ctx, ix, &[&attacker])
        .await
        .err()
        .context("signer/profile mismatch must fail")?;
    assert_eq!(
        custom_code(&err),
        Some(anchor_lang::error::ErrorCode::ConstraintSeeds as u32)
    );
    assert_eq!(
        ctx.banks_client
            .get_account(target)
            .await?
            .context("profile must remain")?,
        original
    );
    assert!(ctx
        .banks_client
        .get_account(username_record_pda(username))
        .await?
        .is_none());
    Ok(())
}

#[tokio::test]
async fn profile_with_other_authority_at_canonical_pda_is_rejected() -> anyhow::Result<()> {
    let player = Keypair::new();
    let target = profile_pda(&player.pubkey());
    let original = program_account(
        &existing_profile(Pubkey::new_unique()),
        8 + PlayerProfile::INIT_SPACE,
    )?;
    let mut ctx = start(vec![
        (target, original.clone()),
        (player.pubkey(), system_account(1_000_000_000)),
    ])
    .await;
    let ix = init_profile_ix(&player.pubkey(), "otherauthority", "GB", 631_152_000);
    let err = send(&mut ctx, ix, &[&player])
        .await
        .err()
        .context("authority mismatch must fail")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(xfchess_game::errors::GameErrorCode::UnauthorizedAccess))
    );
    assert_eq!(
        ctx.banks_client
            .get_account(target)
            .await?
            .context("profile must remain")?,
        original
    );
    Ok(())
}

#[tokio::test]
async fn malformed_profile_bytes_with_genuine_discriminator_fail_without_mutation(
) -> anyhow::Result<()> {
    use anchor_lang::Discriminator;
    let player = Keypair::new();
    let target = profile_pda(&player.pubkey());
    let mut data = PlayerProfile::DISCRIMINATOR.to_vec();
    data.extend_from_slice(&[0xFF; 24]);
    let malformed = solana_sdk::account::Account {
        lamports: 1_000_000_000,
        data,
        owner: xfchess_game::ID,
        executable: false,
        rent_epoch: 0,
    };
    let mut ctx = start(vec![
        (target, malformed.clone()),
        (player.pubkey(), system_account(1_000_000_000)),
    ])
    .await;
    let username = "malformedbytes";
    let ix = init_profile_ix(&player.pubkey(), username, "GB", 631_152_000);
    send(&mut ctx, ix, &[&player])
        .await
        .err()
        .context("malformed profile must fail")?;
    assert_eq!(
        ctx.banks_client
            .get_account(target)
            .await?
            .context("account must remain")?,
        malformed
    );
    assert!(ctx
        .banks_client
        .get_account(username_record_pda(username))
        .await?
        .is_none());
    Ok(())
}

#[tokio::test]
async fn existing_profile_update_preserves_stats_and_verification() -> anyhow::Result<()> {
    let player = Keypair::new();
    let target = profile_pda(&player.pubkey());
    let original = program_account(
        &existing_profile(player.pubkey()),
        8 + PlayerProfile::INIT_SPACE,
    )?;
    let mut ctx = start(vec![
        (target, original),
        (player.pubkey(), system_account(1_000_000_000)),
    ])
    .await;
    let ix = init_profile_ix(&player.pubkey(), "renamedplayer", "DE", 631_152_000);
    send(&mut ctx, ix, &[&player])
        .await
        .context("existing profile update should succeed")?;
    let after = fetch_profile(&mut ctx, &player.pubkey()).await?;
    assert_eq!(after.authority, player.pubkey());
    assert_eq!(after.username, "renamedplayer");
    assert_eq!(after.country, "DE");
    assert!(after.is_verified);
    assert_eq!(
        (after.wins, after.games_played, after.created_at),
        (5, 9, 77)
    );
    assert_eq!(after.elo_rating, 1_750.0);
    Ok(())
}

/// A third party can pre-fund the profile PDA address with a plain transfer
/// (no signature from the recipient is required).
#[tokio::test]
async fn prefunded_empty_profile_pda_does_not_block_creation() -> anyhow::Result<()> {
    let player = Keypair::new();
    let target = profile_pda(&player.pubkey());
    let mut ctx = start(vec![
        (target, system_account(1)),
        (player.pubkey(), system_account(1_000_000_000)),
    ])
    .await;
    let ix = init_profile_ix(&player.pubkey(), "prefundedpda", "GB", 631_152_000);
    send(&mut ctx, ix, &[&player])
        .await
        .context("pre-funded PDA must not block profile creation")?;
    let after = fetch_profile(&mut ctx, &player.pubkey()).await?;
    assert_eq!(after.authority, player.pubkey());
    Ok(())
}

#[tokio::test]
async fn age_gate_is_exact_at_the_boundary_and_safe_at_signed_extremes() -> anyhow::Result<()> {
    use solana_sdk::clock::Clock;
    const NOW: i64 = 1_800_000_000;
    const EIGHTEEN_YEARS: i64 = 567_648_000;
    let cases: [(i64, bool); 8] = [
        (NOW - EIGHTEEN_YEARS, true),
        (NOW - EIGHTEEN_YEARS + 1, false),
        (1, true),
        (0, false),
        (-1, false),
        (i64::MIN, false),
        (i64::MAX, false),
        (NOW, false),
    ];
    for (dob, accepted) in cases {
        let player = Keypair::new();
        let mut ctx = start(vec![(player.pubkey(), system_account(1_000_000_000))]).await;
        ctx.set_sysvar(&Clock {
            unix_timestamp: NOW,
            ..Clock::default()
        });
        let username = "agegateplayer";
        let result = send(
            &mut ctx,
            init_profile_ix(&player.pubkey(), username, "GB", dob),
            &[&player],
        )
        .await;
        if accepted {
            result.with_context(|| format!("dob {dob} is 18+"))?;
        } else {
            let err = result.err().context("underage must fail")?;
            assert_eq!(
                custom_code(&err),
                Some(ec(xfchess_game::errors::GameErrorCode::UnderagePlayer)),
                "dob {dob}"
            );
            assert!(ctx
                .banks_client
                .get_account(profile_pda(&player.pubkey()))
                .await?
                .is_none());
            assert!(ctx
                .banks_client
                .get_account(username_record_pda(username))
                .await?
                .is_none());
            assert_eq!(
                ctx.banks_client.get_balance(player.pubkey()).await?,
                1_000_000_000
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn a_username_owned_by_someone_else_is_rejected_without_creating_a_profile(
) -> anyhow::Result<()> {
    use xfchess_game::state::UsernameRecord;
    let player = Keypair::new();
    let username = "alreadytaken";
    let taken = program_account(
        &UsernameRecord {
            owner: Pubkey::new_unique(),
            created_at: 5,
        },
        8 + UsernameRecord::LEN,
    )?;
    let mut ctx = start(vec![
        (username_record_pda(username), taken.clone()),
        (player.pubkey(), system_account(1_000_000_000)),
    ])
    .await;
    let err = send(
        &mut ctx,
        init_profile_ix(&player.pubkey(), username, "GB", 631_152_000),
        &[&player],
    )
    .await
    .err()
    .context("taken username must fail")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(xfchess_game::errors::GameErrorCode::UsernameTaken))
    );
    assert_eq!(
        ctx.banks_client
            .get_account(username_record_pda(username))
            .await?
            .context("record must remain")?,
        taken
    );
    assert!(ctx
        .banks_client
        .get_account(profile_pda(&player.pubkey()))
        .await?
        .is_none());
    Ok(())
}
