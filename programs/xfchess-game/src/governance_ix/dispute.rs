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
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<DisputeGame>,
    _game_id: u64,
    reason: String,
    evidence_hash: [u8; 32],
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let dispute = &mut ctx.accounts.dispute_record;

    require!(
        game.status == GameStatus::Active || game.status == GameStatus::Inactive,
        GameErrorCode::InvalidGameStatus
    );

    game.status = GameStatus::Disputed;
    game.updated_at = Clock::get()?.unix_timestamp;

    dispute.game_id = _game_id;
    dispute.challenger = ctx.accounts.player.key();
    dispute.reason = reason;
    dispute.evidence_hash = evidence_hash;
    dispute.status = DisputeStatus::Pending;
    dispute.created_at = Clock::get()?.unix_timestamp;
    dispute.bump = ctx.bumps.dispute_record;

    msg!("Game {} disputed by {}", _game_id, dispute.challenger);

    Ok(())
}
