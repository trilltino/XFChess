#[path = "common/agave.rs"]
pub mod agave;
use anchor_lang::prelude::*;
use anyhow::Context as _;
use xfchess_game::state::{
    get_default_prize_shares, PayoutType, Tournament, TournamentStatus, TournamentType,
};
use xfchess_game::tournament_ix::prizes::ledger;
pub mod common;

fn tournament(winner: Pubkey, prizes_claimed: u16) -> Tournament {
    Tournament {
        tournament_id: 1,
        authority: Pubkey::new_unique(),
        name: String::new(),
        entry_fee: 0,
        platform_fee: 0,
        prize_pool: 1_000_000,
        max_players: 8,
        player_count: 0,
        num_registered_players: 0,
        status: TournamentStatus::Completed,
        start_time: None,
        end_time: None,
        fees_advanced: 0,
        fee_payer: Pubkey::new_unique(),
        tournament_type: TournamentType::SingleElimination,
        current_round: 0,
        total_rounds: 0,
        total_matches: 7,
        final_match_index: 6,
        elo_min: 0,
        elo_max: 4000,
        min_players: 2,
        winner: Some(winner),
        second_place: None,
        third_place: None,
        fourth_place: None,
        fifth_place: None,
        sixth_place: None,
        seventh_place: None,
        eighth_place: None,
        ninth_place: None,
        tenth_place: None,
        prize_shares: [10_000, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        created_at: 0,
        started_at: None,
        completed_at: None,
        bump: 0,
        prizes_claimed,
        platform_fee_pool: 0,
        usdc_prize_mint: None,
        usdc_prize_pool: 0,
        usdc_prize_funded: false,
        host_treasury: Pubkey::new_unique(),
        prize_token_mint: None,
        payout_type: PayoutType::LumpSum,
        vesting_params: None,
        base_time_seconds: 0,
        increment_seconds: 0,
        winner_takes_all: false,
        round_boards_reported: [0u8; 16],
    }
}

#[test]
fn prize_ledger_tracks_place_bits_and_amounts() -> anyhow::Result<()> {
    let winner = Pubkey::new_unique();
    let t = tournament(winner, 0);

    assert_eq!(ledger::find_place(&t, winner), Some((0, 10_000)));
    assert_eq!(ledger::place_bit(0).context("required operation value")?, 1);
    assert_eq!(
        ledger::prize_amount(1_000_000, 6000).context("required operation value")?,
        600_000
    );
    assert!(ledger::funded_place_unclaimed(&t, 0).context("required operation value")?);

    let t = tournament(winner, 1);
    assert!(!ledger::funded_place_unclaimed(&t, 0).context("required operation value")?);
    Ok(())
}

#[test]
fn default_prize_shares_keep_expected_boundaries() {
    assert_eq!(get_default_prize_shares(64, false)[..3], [6000, 3000, 1000]);
    assert_eq!(
        get_default_prize_shares(128, false)[..5],
        [5000, 2500, 1500, 500, 500]
    );
    assert_eq!(get_default_prize_shares(8, true)[0], 10_000);
}

#[tokio::test]
async fn sol_only_batch_cannot_consume_a_mixed_pool_claim() -> anyhow::Result<()> {
    use anchor_lang::{InstructionData, ToAccountMetas};
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_sdk::signature::Signer;
    use xfchess_game::{
        errors::GameErrorCode, tournament_ix::lifecycle::initialize_escrow::TournamentEscrow,
    };
    let winner = Pubkey::new_unique();
    let mut state = tournament(winner, 0);
    state.usdc_prize_mint = Some(Pubkey::new_unique());
    state.usdc_prize_pool = 25_000_000;
    state.usdc_prize_funded = true;
    let (key, bump) =
        Pubkey::find_program_address(&[b"tournament", &1u64.to_le_bytes()], &xfchess_game::ID);
    state.bump = bump;
    let escrow =
        Pubkey::find_program_address(&[b"t_escrow", &1u64.to_le_bytes()], &xfchess_game::ID).0;
    let initial = common::program_account(&state, 8 + Tournament::INIT_SPACE)?;
    let initial_escrow = common::program_account(&TournamentEscrow {}, 8)?;
    let wallet = common::system_account(1_000_000);
    let mut ctx = common::start(vec![
        (key, initial.clone()),
        (escrow, initial_escrow.clone()),
        (winner, wallet.clone()),
    ])
    .await;
    let mut accounts =
        xfchess_game::__client_accounts_distribute_tournament_prizes::DistributeTournamentPrizes {
            tournament: key,
            escrow_pda: escrow,
            cranker: ctx.payer.pubkey(),
        }
        .to_account_metas(None);
    accounts.push(AccountMeta::new(winner, false));
    let ix = Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data: xfchess_game::instruction::DistributeTournamentPrizes { tournament_id: 1 }.data(),
    };
    let err = common::send(&mut ctx, ix, &[])
        .await
        .err()
        .context("batch must preserve the unclaimed USDC entitlement")?;
    assert_eq!(
        common::custom_code(&err),
        Some(common::ec(GameErrorCode::MissingTokenAccounts))
    );
    assert_eq!(ctx.banks_client.get_account(key).await?, Some(initial));
    assert_eq!(
        ctx.banks_client.get_account(escrow).await?,
        Some(initial_escrow)
    );
    assert_eq!(ctx.banks_client.get_account(winner).await?, Some(wallet));
    Ok(())
}

#[tokio::test]
async fn individual_mixed_pool_claim_pays_both_and_blocks_a_second_claim() -> anyhow::Result<()> {
    use anchor_lang::solana_program::{program_option::COption, program_pack::Pack};
    use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
    use anchor_spl::token::spl_token::state::{Account as TokenAccount, AccountState, Mint};
    use crate::agave::ProgramTestBanksClientExt;
    use solana_sdk::{
        account::Account,
        instruction::Instruction,
        signature::{Keypair, Signer},
    };
    use xfchess_game::tournament_ix::lifecycle::initialize_escrow::TournamentEscrow;
    let winner = Keypair::new();
    let mint = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let (key, bump) =
        Pubkey::find_program_address(&[b"tournament", &1u64.to_le_bytes()], &xfchess_game::ID);
    let escrow =
        Pubkey::find_program_address(&[b"t_escrow", &1u64.to_le_bytes()], &xfchess_game::ID).0;
    let token_authority =
        Pubkey::find_program_address(&[b"t_usdc_prize", &1u64.to_le_bytes()], &xfchess_game::ID).0;
    let token_escrow =
        anchor_spl::associated_token::get_associated_token_address(&token_authority, &mint);
    let tokens = 25_000_000;
    let mut state = tournament(winner.pubkey(), 0);
    state.bump = bump;
    state.usdc_prize_mint = Some(mint);
    state.usdc_prize_pool = tokens;
    state.usdc_prize_funded = true;
    let token_account = |owner, amount| -> anyhow::Result<Account> {
        let mut data = vec![0; TokenAccount::LEN];
        TokenAccount::pack(
            TokenAccount {
                mint,
                owner,
                amount,
                delegate: COption::None,
                state: AccountState::Initialized,
                is_native: COption::None,
                delegated_amount: 0,
                close_authority: COption::None,
            },
            &mut data,
        )?;
        Ok(Account {
            lamports: 10_000_000,
            data,
            owner: anchor_spl::token::ID,
            executable: false,
            rent_epoch: 0,
        })
    };
    let mut mint_data = vec![0; Mint::LEN];
    Mint::pack(
        Mint {
            mint_authority: COption::None,
            supply: tokens,
            decimals: 6,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        &mut mint_data,
    )?;
    let mut ctx = common::start(vec![
        (
            key,
            common::program_account(&state, 8 + Tournament::INIT_SPACE)?,
        ),
        (escrow, common::program_account(&TournamentEscrow {}, 8)?),
        (winner.pubkey(), common::system_account(1_000_000)),
        (
            mint,
            Account {
                lamports: 10_000_000,
                data: mint_data,
                owner: anchor_spl::token::ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (token_escrow, token_account(token_authority, tokens)?),
        (destination, token_account(winner.pubkey(), 0)?),
    ])
    .await;
    let ix = Instruction {
        program_id: xfchess_game::ID,
        accounts: xfchess_game::__client_accounts_claim_tournament_prize::ClaimTournamentPrize {
            tournament: key,
            usdc_prize_escrow_authority: token_authority,
            usdc_prize_escrow: Some(token_escrow),
            claimant_usdc_ata: Some(destination),
            usdc_mint: Some(mint),
            escrow_pda: escrow,
            claimant_wallet: winner.pubkey(),
            claimant: winner.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: solana_system_interface::program::ID,
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::ClaimTournamentPrize { tournament_id: 1 }.data(),
    };
    common::send(&mut ctx, ix.clone(), &[&winner]).await?;
    assert_eq!(
        ctx.banks_client.get_balance(winner.pubkey()).await?,
        2_000_000
    );
    for (address, expected) in [(destination, tokens), (token_escrow, 0)] {
        let account = ctx
            .banks_client
            .get_account(address)
            .await?
            .context("token account missing")?;
        assert_eq!(TokenAccount::unpack(&account.data)?.amount, expected);
    }
    let before = ctx
        .banks_client
        .get_account(key)
        .await?
        .context("tournament missing")?;
    assert_eq!(
        Tournament::try_deserialize(&mut before.data.as_slice())?.prizes_claimed,
        1
    );
    ctx.last_blockhash = ctx
        .banks_client
        .get_new_latest_blockhash(&ctx.last_blockhash)
        .await?;
    let err = common::send(&mut ctx, ix, &[&winner])
        .await
        .err()
        .context("second claim must reject")?;
    assert_eq!(
        common::custom_code(&err),
        Some(common::ec(
            xfchess_game::errors::GameErrorCode::PrizeAlreadyClaimed
        ))
    );
    assert_eq!(ctx.banks_client.get_account(key).await?, Some(before));
    assert_eq!(
        ctx.banks_client.get_balance(winner.pubkey()).await?,
        2_000_000
    );
    Ok(())
}
