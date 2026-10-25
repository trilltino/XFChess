use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

/// Anchor discriminator of `xfchess_pass::set_paused`, the first 8 bytes of
/// sha256("global:set_paused").
const SET_PAUSED_DISCRIMINATOR: [u8; 8] = [91, 60, 125, 192, 176, 225, 166, 218];

/// Lets the organizer pause or resume every transfer of the tournament's pass. The pass program
/// accepts that only from this program's registrar PDA, so without this instruction a pass could
/// never be paused. Pausing stops pass transfers, not redemption: registering burns the pass and
/// never invokes the transfer hook.
#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct SetPassPaused<'info> {
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
    /// CHECK: pinned to the mint recorded in the binding.
    #[account(address = pass_binding.pass_mint @ GameErrorCode::PassMintMismatch)]
    pub pass_mint: UncheckedAccount<'info>,
    /// CHECK: the pass program's config PDA for this mint; the pass program checks its contents.
    #[account(
        mut,
        seeds = [b"pass-config", pass_mint.key().as_ref()],
        seeds::program = XFCHESS_PASS_PROGRAM_ID,
        bump
    )]
    pub pass_config: UncheckedAccount<'info>,
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

pub fn handler(ctx: Context<SetPassPaused>, tournament_id: u64, is_paused: bool) -> Result<()> {
    require!(
        ctx.accounts.tournament.tournament_id == tournament_id
            && ctx.accounts.pass_binding.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    let config = ctx.accounts.pass_config.to_account_info();
    let registrar = ctx.accounts.registrar.to_account_info();
    let mut data = SET_PAUSED_DISCRIMINATOR.to_vec();
    data.push(u8::from(is_paused));
    let bump = [ctx.bumps.registrar];
    invoke_signed(
        &Instruction {
            program_id: XFCHESS_PASS_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(config.key(), false),
                AccountMeta::new_readonly(registrar.key(), true),
            ],
            data,
        },
        &[
            config,
            registrar,
            ctx.accounts.pass_program.to_account_info(),
        ],
        &[&[PASS_REGISTRAR_SEED, &bump]],
    )?;
    Ok(())
}
