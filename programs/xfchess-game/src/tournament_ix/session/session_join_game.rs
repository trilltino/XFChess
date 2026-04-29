//! Session-signed variant of `join_game` for tournament play.
//!
//! Uses the tournament-scoped session key to co-sign joining a game,
//! drawing funds from the delegation PDA vault for cross-border fees and wagers.

use crate::constants::*;
use crate::errors::*;
use crate::state::*;
use anchor_lang::prelude::*;

fn get_country_fee(country: &str, match_type: MatchType) -> u64 {
    if match_type == MatchType::Free {
        return 0;
    }
    match country {
        "GB" => UK_FEE_LAMPORTS,
        "BR" => BRAZIL_FEE_LAMPORTS,
        "CA" => CANADA_FEE_LAMPORTS,
        "DE" => GERMANY_FEE_LAMPORTS,
        _ => 0,
    }
}

fn apply_cross_border_fee_logic(
    white_country: &str,
    black_country: &str,
    white_fee: u64,
    black_fee: u64,
) -> u64 {
    if white_country != black_country {
        white_fee.min(black_fee)
    } else {
        white_fee
    }
}

#[derive(Accounts)]
#[instruction(tournament_id: u64, game_id: u64)]
pub struct SessionJoinGame<'info> {
    #[account(
        seeds = [b"tournament", tournament_id.to_le_bytes().as_ref()],
        bump = tournament.bump,
    )]
    pub tournament: Box<Account<'info, Tournament>>,

    #[account(
        mut,
        seeds = [
            TournamentSessionDelegation::SEED,
            tournament_id.to_le_bytes().as_ref(),
            player.key().as_ref(),
        ],
        bump = session_delegation.bump,
        constraint = session_delegation.enabled @ XfchessGameError::SessionNotAuthorized,
        constraint = session_delegation.player == player.key() @ XfchessGameError::UnauthorizedAccess,
    )]
    pub session_delegation: Box<Account<'info, TournamentSessionDelegation>>,

    /// Session key signer (hot key, not the player wallet).
    pub session_signer: Signer<'info>,

    #[account(
        constraint = tournament.players.iter().any(|p| *p == player.key()) @ XfchessGameError::UnauthorizedAccess,
    )]
    /// CHECK: Verified against tournament player list and delegation PDA.
    pub player: UncheckedAccount<'info>,

    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Box<Account<'info, Game>>,

    /// CHECK: PDA for escrowing SOL.
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,

    /// White player profile for cross-border fee calculation.
    #[account(seeds = [PROFILE_SEED, game.white.as_ref()], bump)]
    pub white_profile: Box<Account<'info, PlayerProfile>>,

    /// Black player profile (the joining player).
    #[account(seeds = [PROFILE_SEED, player.key().as_ref()], bump)]
    pub player_profile: Box<Account<'info, PlayerProfile>>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<SessionJoinGame>,
    _tournament_id: u64,
    _game_id: u64,
) -> Result<()> {
    let delegation = &mut ctx.accounts.session_delegation;
    let game = &mut ctx.accounts.game;
    let player_key = ctx.accounts.player.key();

    let now = Clock::get()?.unix_timestamp;
    require!(delegation.is_valid(now), XfchessGameError::SessionExpired);
    require!(
        delegation.session_key == ctx.accounts.session_signer.key(),
        XfchessGameError::SessionNotAuthorized
    );

    require!(
        game.game_type == GameType::PvP,
        GameErrorCode::GameAlreadyFull
    );
    require!(
        game.status == GameStatus::WaitingForOpponent,
        GameErrorCode::GameAlreadyFull
    );
    require!(game.white != player_key, GameErrorCode::CannotPlaySelf);

    let white_country = &ctx.accounts.white_profile.country;
    let player_country = &ctx.accounts.player_profile.country;

    let white_fee = get_country_fee(white_country, game.match_type.clone());
    let black_fee = get_country_fee(player_country, game.match_type.clone());
    let final_fee = apply_cross_border_fee_logic(white_country, player_country, white_fee, black_fee);

    let total_cost = game.wager_amount.saturating_add(final_fee);

    require!(
        delegation.total_spent.saturating_add(total_cost) <= delegation.spending_limit,
        XfchessGameError::SessionSpendingLimitExceeded
    );

    game.black = player_key;
    game.status = GameStatus::Active;
    game.updated_at = Clock::get()?.unix_timestamp;
    game.country_fee = final_fee;

    // Transfer wager from delegation PDA vault to escrow
    if game.wager_amount > 0 && game.wager_token.is_none() {
        let tid_bytes = delegation.tournament_id.to_le_bytes();
        let player_bytes = delegation.player.to_bytes();
        let bump = [delegation.bump];
        let delegation_seeds: [&[u8]; 4] = [
            TournamentSessionDelegation::SEED,
            tid_bytes.as_ref(),
            player_bytes.as_ref(),
            bump.as_ref(),
        ];
        let signer_seeds: &[&[&[u8]]] = &[&delegation_seeds];

        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: delegation.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
                signer_seeds,
            ),
            game.wager_amount,
        )?;
    }

    // Update delegation spending (count wager + fee)
    delegation.total_spent = delegation.total_spent.saturating_add(total_cost);
    delegation.games_played = delegation.games_played.saturating_add(1);

    msg!(
        "Session-joined game {} in tournament {}. Total spent: {} lamports",
        game.game_id,
        delegation.tournament_id,
        total_cost
    );
    Ok(())
}
