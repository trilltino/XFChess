use crate::constants::*;
use crate::elo::rating::display_rating;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

/// Anchor discriminators of `xfchess_pass::issue_eligibility` and `refresh_eligibility`,
/// the first 8 bytes of sha256("global:<name>").
const ISSUE_ELIGIBILITY_DISCRIMINATOR: [u8; 8] = [29, 88, 236, 137, 74, 17, 193, 144];
const REFRESH_ELIGIBILITY_DISCRIMINATOR: [u8; 8] = [18, 227, 118, 61, 5, 162, 20, 209];
/// Offset of `is_revoked` in the pass program's `EligibilityRecord`.
const REVOKED_FLAG_OFFSET: usize = 85;

/// Attests a player's rating to the tournament's pass program. This program is the pass
/// registrar: the rating comes from the player's own `PlayerProfile`, never from the caller, and
/// only a tournament in registration with a pass binding can be attested. Anyone may pay and
/// call it for any player; the first call issues the eligibility record and later calls refresh
/// it, so a stale record can be renewed before a pass transfer.
#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct IssuePassEligibility<'info> {
    #[account(
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(
        seeds = [TOURNAMENT_PASS_BINDING_SEED, &tournament_id.to_le_bytes()],
        bump = pass_binding.bump
    )]
    pub pass_binding: Account<'info, TournamentPassBinding>,
    /// CHECK: only used as the profile and eligibility key; no signature is needed to attest.
    pub player: UncheckedAccount<'info>,
    #[account(
        seeds = [PROFILE_SEED, player.key().as_ref()],
        bump
    )]
    pub player_profile: Account<'info, PlayerProfile>,
    /// CHECK: pinned to the mint recorded in the binding; the pass program validates its config.
    #[account(address = pass_binding.pass_mint @ GameErrorCode::PassMintMismatch)]
    pub pass_mint: UncheckedAccount<'info>,
    /// CHECK: the pass program's config PDA for this mint; the pass program checks its contents.
    #[account(
        seeds = [b"pass-config", pass_mint.key().as_ref()],
        seeds::program = XFCHESS_PASS_PROGRAM_ID,
        bump
    )]
    pub pass_config: UncheckedAccount<'info>,
    /// CHECK: the pass program's eligibility PDA for this mint and player; created or updated by
    /// the pass program.
    #[account(
        mut,
        seeds = [b"eligibility", pass_mint.key().as_ref(), player.key().as_ref()],
        seeds::program = XFCHESS_PASS_PROGRAM_ID,
        bump
    )]
    pub eligibility_record: UncheckedAccount<'info>,
    /// CHECK: this program's registrar PDA; it only signs the CPI below.
    #[account(seeds = [PASS_REGISTRAR_SEED], bump)]
    pub registrar: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: pinned to the pass program ID.
    #[account(address = XFCHESS_PASS_PROGRAM_ID @ GameErrorCode::InvalidAccountOwner, executable)]
    pub pass_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<IssuePassEligibility>, tournament_id: u64) -> Result<()> {
    let tournament = &ctx.accounts.tournament;
    require!(
        tournament.tournament_id == tournament_id
            && ctx.accounts.pass_binding.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::TournamentNotInRegistration
    );
    // Only a profile the KYC authority has verified can be attested; an open profile is free to
    // create, so without this any number of wallets could be attested.
    require!(
        ctx.accounts.player_profile.is_verified,
        GameErrorCode::ProfileNotVerified
    );
    let rating = display_rating(&ctx.accounts.player_profile, tournament.base_time_seconds);
    require!(
        (tournament.elo_min..=tournament.elo_max).contains(&rating),
        GameErrorCode::EloOutOfRange
    );

    let record = ctx.accounts.eligibility_record.to_account_info();
    let issuing = record.data_is_empty() && record.owner == &anchor_lang::system_program::ID;
    if !issuing {
        // The pass program clears a revocation on refresh, so a revoked record must not be
        // refreshed from here. Layout: discriminator, bump, mint, player, rating, slot, revoked.
        require_keys_eq!(
            *record.owner,
            XFCHESS_PASS_PROGRAM_ID,
            GameErrorCode::InvalidAccountOwner
        );
        let data = record.try_borrow_data()?;
        require!(
            data.get(REVOKED_FLAG_OFFSET) == Some(&0),
            GameErrorCode::PassEligibilityRevoked
        );
    }
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(if issuing {
        &ISSUE_ELIGIBILITY_DISCRIMINATOR
    } else {
        &REFRESH_ELIGIBILITY_DISCRIMINATOR
    });
    data.extend_from_slice(&rating.to_le_bytes());

    let registrar = ctx.accounts.registrar.to_account_info();
    let config = ctx.accounts.pass_config.to_account_info();
    let (accounts, infos) = if issuing {
        let player = ctx.accounts.player.to_account_info();
        let payer = ctx.accounts.payer.to_account_info();
        let system_program = ctx.accounts.system_program.to_account_info();
        (
            vec![
                AccountMeta::new_readonly(config.key(), false),
                AccountMeta::new(record.key(), false),
                AccountMeta::new_readonly(player.key(), false),
                AccountMeta::new_readonly(registrar.key(), true),
                AccountMeta::new(payer.key(), true),
                AccountMeta::new_readonly(system_program.key(), false),
            ],
            vec![config, record, player, registrar, payer, system_program],
        )
    } else {
        (
            vec![
                AccountMeta::new_readonly(config.key(), false),
                AccountMeta::new(record.key(), false),
                AccountMeta::new_readonly(registrar.key(), true),
            ],
            vec![config, record, registrar],
        )
    };

    let mut infos = infos;
    infos.push(ctx.accounts.pass_program.to_account_info());
    let bump = [ctx.bumps.registrar];
    invoke_signed(
        &Instruction {
            program_id: XFCHESS_PASS_PROGRAM_ID,
            accounts,
            data,
        },
        &infos,
        &[&[PASS_REGISTRAR_SEED, &bump]],
    )?;
    Ok(())
}
