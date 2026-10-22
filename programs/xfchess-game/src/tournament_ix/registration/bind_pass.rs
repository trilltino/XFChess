use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use crate::tournament_ix::registration::pass_accounts;
use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022::{
    extension::{transfer_hook::get_program_id, StateWithExtensions},
    state::Mint as MintState,
};
use anchor_spl::token_interface::Mint;

/// Requires a tournament pass for every later registration. The mint must be a zero-decimal
/// Token-2022 mint whose transfer hook is the xfchess_pass program, so passes can only move
/// to wallets that program has approved. The mint authority must already be revoked and the supply
/// at most the player limit, so the organizer cannot mint extra passes. The pass program must
/// already be initialized for the mint with this program's registrar PDA as its only registrar and
/// a rating policy that covers the tournament's range, because neither can be changed afterwards.
/// Only the organizer can bind, before anyone registers.
#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct BindTournamentPass<'info> {
    #[account(
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(
        init,
        payer = authority,
        space = 8 + TournamentPassBinding::INIT_SPACE,
        seeds = [TOURNAMENT_PASS_BINDING_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub pass_binding: Account<'info, TournamentPassBinding>,
    pub pass_mint: InterfaceAccount<'info, Mint>,
    /// CHECK: the pass program's config PDA for this mint; owner, layout and fields are checked
    /// in the handler.
    #[account(
        seeds = [b"pass-config", pass_mint.key().as_ref()],
        seeds::program = XFCHESS_PASS_PROGRAM_ID,
        bump
    )]
    pub pass_config: UncheckedAccount<'info>,
    /// CHECK: this program's registrar PDA; compared with the config's registrar, never a signer.
    #[account(seeds = [PASS_REGISTRAR_SEED], bump)]
    pub registrar: UncheckedAccount<'info>,
    #[account(
        mut,
        constraint = authority.key() == tournament.authority @ GameErrorCode::NotTournamentAuthority
    )]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<BindTournamentPass>, tournament_id: u64) -> Result<()> {
    let tournament = &ctx.accounts.tournament;
    require!(
        tournament.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::TournamentNotInRegistration
    );
    // Players who registered before the binding would hold no pass.
    require!(
        tournament.num_registered_players == 0,
        GameErrorCode::InvalidTournamentStatus
    );
    require_pass_mint(
        &ctx.accounts.pass_mint.to_account_info(),
        tournament.max_players,
    )?;
    let config = pass_accounts::load_config(&ctx.accounts.pass_config.to_account_info())?;
    pass_accounts::check_config(
        &config,
        &ctx.accounts.pass_mint.key(),
        &ctx.accounts.registrar.key(),
        tournament.elo_min,
        tournament.elo_max,
    )?;

    ctx.accounts.pass_binding.set_inner(TournamentPassBinding {
        tournament_id,
        pass_mint: ctx.accounts.pass_mint.key(),
        bump: ctx.bumps.pass_binding,
    });
    Ok(())
}

fn require_pass_mint(mint: &AccountInfo, max_players: u16) -> Result<()> {
    require_keys_eq!(
        *mint.owner,
        anchor_spl::token_2022::ID,
        GameErrorCode::InvalidPassMint
    );
    let data = mint.try_borrow_data()?;
    let state = StateWithExtensions::<MintState>::unpack(&data)
        .map_err(|_| error!(GameErrorCode::InvalidPassMint))?;
    require!(
        state.base.decimals == 0 && get_program_id(&state) == Some(XFCHESS_PASS_PROGRAM_ID),
        GameErrorCode::InvalidPassMint
    );
    // A fixed supply the organizer cannot inflate, with no more passes than seats.
    require!(
        state.base.mint_authority.is_none(),
        GameErrorCode::PassMintAuthorityActive
    );
    require!(
        state.base.supply <= u64::from(max_players),
        GameErrorCode::PassSupplyExceedsPlayers
    );
    Ok(())
}
