use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

/// Anchor discriminator of `xfchess_pass::revoke_eligibility`, the first 8 bytes of
/// sha256("global:revoke_eligibility").
const REVOKE_ELIGIBILITY_DISCRIMINATOR: [u8; 8] = [86, 156, 4, 240, 195, 239, 201, 42];

/// Lets the organizer revoke a player's pass eligibility. The pass program accepts revocation
/// only from this program's registrar PDA, so without this instruction a record could never be
/// revoked. A revoked wallet cannot receive the pass, and `issue_pass_eligibility` refuses to refresh
/// a revoked record.
#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct RevokePassEligibility<'info> {
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
    /// CHECK: only used as the eligibility key; the pass program checks the record it names.
    pub player: UncheckedAccount<'info>,
    /// CHECK: pinned to the mint recorded in the binding.
    #[account(address = pass_binding.pass_mint @ GameErrorCode::PassMintMismatch)]
    pub pass_mint: UncheckedAccount<'info>,
    /// CHECK: the pass program's config PDA for this mint; the pass program checks its contents.
    #[account(
        seeds = [b"pass-config", pass_mint.key().as_ref()],
        seeds::program = XFCHESS_PASS_PROGRAM_ID,
        bump
    )]
    pub pass_config: UncheckedAccount<'info>,
    /// CHECK: the pass program's eligibility PDA for this mint and player.
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
    #[account(
        constraint = authority.key() == tournament.authority @ GameErrorCode::NotTournamentAuthority
    )]
    pub authority: Signer<'info>,
    /// CHECK: pinned to the pass program ID.
    #[account(address = XFCHESS_PASS_PROGRAM_ID @ GameErrorCode::InvalidAccountOwner, executable)]
    pub pass_program: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<RevokePassEligibility>, tournament_id: u64) -> Result<()> {
    require!(
        ctx.accounts.tournament.tournament_id == tournament_id
            && ctx.accounts.pass_binding.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    let config = ctx.accounts.pass_config.to_account_info();
    let record = ctx.accounts.eligibility_record.to_account_info();
    let registrar = ctx.accounts.registrar.to_account_info();
    let bump = [ctx.bumps.registrar];
    invoke_signed(
        &Instruction {
            program_id: XFCHESS_PASS_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new_readonly(config.key(), false),
                AccountMeta::new(record.key(), false),
                AccountMeta::new_readonly(registrar.key(), true),
            ],
            data: REVOKE_ELIGIBILITY_DISCRIMINATOR.to_vec(),
        },
        &[
            config,
            record,
            registrar,
            ctx.accounts.pass_program.to_account_info(),
        ],
        &[&[PASS_REGISTRAR_SEED, &bump]],
    )?;
    Ok(())
}
