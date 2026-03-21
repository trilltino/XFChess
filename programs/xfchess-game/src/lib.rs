use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod instructions;
pub mod state;

pub use instructions::*;
pub use state::*;

declare_id!("3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP");

#[program]
pub mod xfchess_game {
    use super::*;

    pub fn init_profile(ctx: Context<InitProfile>) -> Result<()> {
        instructions::init_profile::handler(ctx)
    }

    pub fn create_game(
        ctx: Context<CreateGame>,
        game_id: u64,
        wager_amount: u64,
        game_type: GameType,
    ) -> Result<()> {
        instructions::create_game::handler(ctx, game_id, wager_amount, game_type)
    }

    pub fn join_game(ctx: Context<JoinGame>, game_id: u64) -> Result<()> {
        instructions::join_game::handler(ctx, game_id)
    }

    pub fn record_move(
        ctx: Context<RecordMove>,
        game_id: u64,
        move_str: String,
        next_fen: String,
    ) -> Result<()> {
        instructions::record_move::handler(ctx, game_id, move_str, next_fen)
    }

    pub fn finalize_game(ctx: Context<EndGame>, game_id: u64, result: GameResult) -> Result<()> {
        instructions::finalize_game::handler(ctx, game_id, result)
    }

    pub fn withdraw_expired_wager(ctx: Context<WithdrawExpiredWager>, game_id: u64) -> Result<()> {
        instructions::withdraw_expired_wager::handler(ctx, game_id)
    }

    pub fn authorize_session_key(
        ctx: Context<AuthorizeSessionCtx>,
        game_id: u64,
        session_pubkey: Pubkey,
    ) -> Result<()> {
        instructions::session_delegation::handler_authorize_session_key(
            ctx,
            game_id,
            session_pubkey,
        )
    }

    pub fn revoke_session_key(ctx: Context<RevokeSessionCtx>, game_id: u64) -> Result<()> {
        instructions::session_delegation::handler_revoke_session_key(ctx, game_id)
    }

    pub fn commit_move_batch(
        ctx: Context<CommitMoveBatchCtx>,
        game_id: u64,
        moves: Vec<String>,
        next_fens: Vec<String>,
    ) -> Result<()> {
        instructions::commit_move_batch::handler_commit_move_batch(ctx, game_id, moves, next_fens)
    }

    pub fn delegate_game(
        ctx: Context<DelegateGameCtx>,
        game_id: u64,
        valid_until: i64,
    ) -> Result<()> {
        instructions::delegate_game::handler_delegate_game(ctx, game_id, valid_until)
    }

    pub fn undelegate_game(ctx: Context<UndelegateGameCtx>, game_id: u64) -> Result<()> {
        instructions::delegate_game::handler_undelegate_game(ctx, game_id)
    }
}
