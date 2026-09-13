use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct DisputeGame<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    #[account(
        init,
        payer = player,
        space = 8 + DisputeRecord::INIT_SPACE,
        seeds = [b"dispute".as_ref(), &game_id.to_le_bytes()],
        bump
    )]
    pub dispute_record: Account<'info, DisputeRecord>,
    #[account(
        mut,
        constraint = player.key() == game.white || player.key() == game.black
            @ GameErrorCode::NotInGame
    )]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<DisputeGame>,
    _game_id: u64,
    reason: String,
    evidence_hash: [u8; 32],
) -> Result<()> {
    crate::governance_ix::resolution::require_text_fits(&reason)?;
    let game = &mut ctx.accounts.game;
    let dispute = &mut ctx.accounts.dispute_record;

    require!(
        game.status == GameStatus::Active || game.status == GameStatus::Inactive,
        GameErrorCode::InvalidGameStatus
    );

    let now = Clock::get()?.unix_timestamp;
    game.status = GameStatus::Disputed;
    game.updated_at = now;

    dispute.game_id = _game_id;
    dispute.challenger = ctx.accounts.player.key();
    dispute.reason = reason;
    dispute.evidence_hash = evidence_hash;
    dispute.status = DisputeStatus::Pending;
    dispute.expires_at = now
        .checked_add(crate::constants::DISPUTE_TTL_SECS)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    dispute.created_at = now;
    dispute.bond_amount = DISPUTE_BOND_LAMPORTS;
    dispute.bump = ctx.bumps.dispute_record;

    anchor_lang::system_program::transfer(
        CpiContext::new(
            System::id(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.player.to_account_info(),
                to: ctx.accounts.dispute_record.to_account_info(),
            },
        ),
        DISPUTE_BOND_LAMPORTS,
    )?;

    Ok(())
}
