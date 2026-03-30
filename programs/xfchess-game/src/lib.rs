use anchor_lang::prelude::*;

pub mod account_ix;
pub mod constants;
pub mod delegation_ix;
pub mod errors;
pub mod game_ix;
pub mod governance_ix;
pub mod moves_ix;
pub mod state;

// Re-export account structs at crate root so Anchor's generated __private::__global handlers
// can find them via their `use super::*` chain.
pub use account_ix::{InitProfile, WithdrawExpiredWager};
pub use delegation_ix::{
    AuthorizeSessionCtx, DelegateGameCtx, InitializeAfterUndelegation, RevokeSessionCtx,
    UndelegateGameCtx,
};
pub use game_ix::{CancelGame, CreateGame, EndGame, JoinGame};
pub use governance_ix::{DisputeGame, ResolveDispute};
pub use moves_ix::{CommitMoveBatchCtx, RecordMove};

// Anchor 0.32 #[program] generates `pub use crate::__client_accounts_<snake>::*` at the crate
// root for every instruction accounts struct. The derive macro generates these as pub(crate)
// modules inside submodules; they cannot be pub use-d directly (E0365). Instead, create thin
// pub mod wrappers here that re-export the pub *contents* of each pub(crate) module.
pub mod __client_accounts_init_profile {
    pub use crate::account_ix::profile::__client_accounts_init_profile::*;
}
pub mod __client_accounts_withdraw_expired_wager {
    pub use crate::account_ix::withdraw::__client_accounts_withdraw_expired_wager::*;
}
pub mod __client_accounts_delegate_game_ctx {
    pub use crate::delegation_ix::delegate::__client_accounts_delegate_game_ctx::*;
}
pub mod __client_accounts_undelegate_game_ctx {
    pub use crate::delegation_ix::delegate::__client_accounts_undelegate_game_ctx::*;
}
pub mod __client_accounts_authorize_session_ctx {
    pub use crate::delegation_ix::session::__client_accounts_authorize_session_ctx::*;
}
pub mod __client_accounts_revoke_session_ctx {
    pub use crate::delegation_ix::session::__client_accounts_revoke_session_ctx::*;
}
pub mod __client_accounts_initialize_after_undelegation {
    pub use crate::delegation_ix::undelegation::__client_accounts_initialize_after_undelegation::*;
}
pub mod __client_accounts_cancel_game {
    pub use crate::game_ix::cancel::__client_accounts_cancel_game::*;
}
pub mod __client_accounts_create_game {
    pub use crate::game_ix::create::__client_accounts_create_game::*;
}
pub mod __client_accounts_end_game {
    pub use crate::game_ix::finalize::__client_accounts_end_game::*;
}
pub mod __client_accounts_join_game {
    pub use crate::game_ix::join::__client_accounts_join_game::*;
}
pub mod __client_accounts_dispute_game {
    pub use crate::governance_ix::dispute::__client_accounts_dispute_game::*;
}
pub mod __client_accounts_resolve_dispute {
    pub use crate::governance_ix::resolve::__client_accounts_resolve_dispute::*;
}
pub mod __client_accounts_commit_move_batch_ctx {
    pub use crate::moves_ix::commit_batch::__client_accounts_commit_move_batch_ctx::*;
}
pub mod __client_accounts_record_move {
    pub use crate::moves_ix::record::__client_accounts_record_move::*;
}

#[allow(unused_imports)]
use ephemeral_rollups_sdk::anchor::MagicProgram;

declare_id!("FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX");

#[program]
pub mod xfchess_game {
    use super::*;
    use crate::account_ix::{InitProfile, WithdrawExpiredWager};
    use crate::delegation_ix::{
        AuthorizeSessionCtx, DelegateGameCtx, InitializeAfterUndelegation, RevokeSessionCtx,
        UndelegateGameCtx,
    };
    use crate::game_ix::{CancelGame, CreateGame, EndGame, JoinGame};
    use crate::governance_ix::{DisputeGame, ResolveDispute};
    use crate::moves_ix::{CommitMoveBatchCtx, RecordMove};
    use ephemeral_rollups_sdk::cpi::undelegate_account;

    pub fn init_profile(ctx: Context<InitProfile>) -> Result<()> {
        crate::account_ix::profile::handler(ctx)
    }

    pub fn create_game(
        ctx: Context<CreateGame>,
        game_id: u64,
        wager_amount: u64,
        game_type: state::GameType,
    ) -> Result<()> {
        crate::game_ix::create::handler(ctx, game_id, wager_amount, game_type)
    }

    pub fn join_game(ctx: Context<JoinGame>, game_id: u64) -> Result<()> {
        crate::game_ix::join::handler(ctx, game_id)
    }

    pub fn record_move(
        ctx: Context<RecordMove>,
        game_id: u64,
        move_str: String,
        next_fen: String,
        nonce: u64,
        signature: Option<Vec<u8>>,
    ) -> Result<()> {
        crate::moves_ix::record::handler(ctx, game_id, move_str, next_fen, nonce, signature)
    }

    pub fn finalize_game(ctx: Context<EndGame>, game_id: u64, result: state::GameResult) -> Result<()> {
        crate::game_ix::finalize::handler(ctx, game_id, result)
    }

    pub fn withdraw_expired_wager(ctx: Context<WithdrawExpiredWager>, game_id: u64) -> Result<()> {
        crate::account_ix::withdraw::handler(ctx, game_id)
    }

    pub fn cancel_game(ctx: Context<CancelGame>, game_id: u64) -> Result<()> {
        crate::game_ix::cancel::handler(ctx, game_id)
    }

    pub fn dispute_game(
        ctx: Context<DisputeGame>,
        game_id: u64,
        reason: String,
        evidence_hash: [u8; 32],
    ) -> Result<()> {
        crate::governance_ix::dispute::handler(ctx, game_id, reason, evidence_hash)
    }

    pub fn resolve_dispute(
        ctx: Context<ResolveDispute>,
        game_id: u64,
        resolution: String,
        winner: Option<Pubkey>,
    ) -> Result<()> {
        crate::governance_ix::resolve::handler(ctx, game_id, resolution, winner)
    }

    pub fn authorize_session_key(
        ctx: Context<AuthorizeSessionCtx>,
        game_id: u64,
        session_pubkey: Pubkey,
    ) -> Result<()> {
        crate::delegation_ix::session::handler_authorize_session_key(
            ctx,
            game_id,
            session_pubkey,
        )
    }

    pub fn revoke_session_key(ctx: Context<RevokeSessionCtx>, game_id: u64) -> Result<()> {
        crate::delegation_ix::session::handler_revoke_session_key(ctx, game_id)
    }

    pub fn commit_move_batch(
        ctx: Context<CommitMoveBatchCtx>,
        game_id: u64,
        moves: Vec<String>,
        next_fens: Vec<String>,
    ) -> Result<()> {
        crate::moves_ix::commit_batch::handler_commit_move_batch(
            ctx,
            game_id,
            moves,
            next_fens,
        )
    }

    pub fn delegate_game(
        ctx: Context<DelegateGameCtx>,
        game_id: u64,
        valid_until: i64,
    ) -> Result<()> {
        crate::delegation_ix::delegate::handler_delegate_game(ctx, game_id, valid_until)
    }

    pub fn undelegate_game(ctx: Context<UndelegateGameCtx>, game_id: u64) -> Result<()> {
        crate::delegation_ix::delegate::handler_undelegate_game(ctx, game_id)
    }

    pub fn process_undelegation(
        ctx: Context<InitializeAfterUndelegation>,
        account_seeds: Vec<Vec<u8>>,
    ) -> Result<()> {
        let [delegated_account, buffer, payer, system_program] = [
            &ctx.accounts.base_account,
            &ctx.accounts.buffer,
            &ctx.accounts.payer,
            &ctx.accounts.system_program,
        ];
        undelegate_account(
            delegated_account,
            &id(),
            buffer,
            payer,
            system_program,
            account_seeds,
        )?;
        Ok(())
    }
}
