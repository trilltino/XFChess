//! Main entry point mapping all Anchor program instructions to their handler functions.

use anchor_lang::prelude::*;

pub mod account_ix;
pub mod constants;
pub mod crank_ix;
pub mod delegation_ix;
pub mod elo;
pub mod errors;
pub mod game_ix;
pub mod governance_ix;
pub mod moves_ix;
pub mod state;
pub mod tournament_ix;

// Re-export account structs at crate root so Anchor's generated __private::__global handlers
// can find them via their `use super::*` chain.
pub use account_ix::{InitProfile, VerifyProfile, SetUsername, WithdrawExpiredWager,
    InitializeFeeVault, CollectFee, ClaimFees, CreateSession, RevokeSession, UpdateElo};
pub use crank_ix::{ScheduleTimeCheck, CrankTimeCheck, ScheduleTimeCheckArgs, crank_time_check, schedule_time_check_crank};
pub use delegation_ix::{
    AuthorizeSessionCtx, DelegateGameCtx, InitializeAfterUndelegation, RevokeSessionCtx,
    UndelegateGameCtx,
};
pub use game_ix::{CancelGame, CreateGame, EndGame, JoinGame, ResignGame, ClaimTimeout};
pub use governance_ix::{DisputeGame, ResolveDispute};
pub use moves_ix::{CommitMoveBatchCtx, RecordMove};
pub use tournament_ix::{
    AdvanceWinner, CancelTournament, ClaimTournamentPrize, InitializeMatch,
    InitializeTournament, RecordMatchResult, RegisterPlayer, StartTournament,
};

// Anchor 0.32 #[program] generates `pub use crate::__client_accounts_<snake>::*` at the crate
// root for every instruction accounts struct. The derive macro generates these as pub(crate)
// modules inside submodules; they cannot be pub use-d directly (E0365). Instead, create thin
// pub mod wrappers here that re-export the pub *contents* of each pub(crate) module.
pub mod __client_accounts_init_profile {
    pub use crate::account_ix::profile::__client_accounts_init_profile::*;
}
pub mod __client_accounts_verify_profile {
    pub use crate::account_ix::profile::__client_accounts_verify_profile::*;
}
pub mod __client_accounts_set_username {
    pub use crate::account_ix::set_username::__client_accounts_set_username::*;
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
pub mod __client_accounts_resign_game {
    pub use crate::game_ix::resign::__client_accounts_resign_game::*;
}
pub mod __client_accounts_claim_timeout {
    pub use crate::game_ix::timeout::__client_accounts_claim_timeout::*;
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
pub mod __client_accounts_initialize_tournament {
    pub use crate::tournament_ix::initialize::__client_accounts_initialize_tournament::*;
}
pub mod __client_accounts_register_player {
    pub use crate::tournament_ix::register::__client_accounts_register_player::*;
}
pub mod __client_accounts_start_tournament {
    pub use crate::tournament_ix::start::__client_accounts_start_tournament::*;
}
pub mod __client_accounts_record_match_result {
    pub use crate::tournament_ix::record_result::__client_accounts_record_match_result::*;
}
pub mod __client_accounts_advance_winner {
    pub use crate::tournament_ix::record_result::__client_accounts_advance_winner::*;
}
pub mod __client_accounts_initialize_match {
    pub use crate::tournament_ix::initialize_match::__client_accounts_initialize_match::*;
}
pub mod __client_accounts_claim_tournament_prize {
    pub use crate::tournament_ix::claim_prize::__client_accounts_claim_tournament_prize::*;
}
pub mod __client_accounts_cancel_tournament {
    pub use crate::tournament_ix::cancel::__client_accounts_cancel_tournament::*;
}
pub mod __client_accounts_initialize_fee_vault {
    pub use crate::account_ix::fee_vault_ix::__client_accounts_initialize_fee_vault::*;
}
pub mod __client_accounts_collect_fee {
    pub use crate::account_ix::fee_vault_ix::__client_accounts_collect_fee::*;
}
pub mod __client_accounts_claim_fees {
    pub use crate::account_ix::fee_vault_ix::__client_accounts_claim_fees::*;
}
pub mod __client_accounts_create_session {
    pub use crate::account_ix::fee_vault_ix::__client_accounts_create_session::*;
}
pub mod __client_accounts_revoke_session {
    pub use crate::account_ix::fee_vault_ix::__client_accounts_revoke_session::*;
}
pub mod __client_accounts_update_elo {
    pub use crate::account_ix::fee_vault_ix::__client_accounts_update_elo::*;
}
pub mod __client_accounts_schedule_time_check {
    pub use crate::crank_ix::schedule_time_check::__client_accounts_schedule_time_check::*;
}
pub mod __client_accounts_crank_time_check {
    pub use crate::crank_ix::crank_time_check::__client_accounts_crank_time_check::*;
}

#[allow(unused_imports)]
use ephemeral_rollups_sdk::anchor::MagicProgram;

declare_id!("FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX");

#[program]
pub mod xfchess_game {
    use super::*;
    use crate::account_ix::{
        InitProfile, VerifyProfile, SetUsername, WithdrawExpiredWager,
        InitializeFeeVault, CollectFee, ClaimFees, CreateSession, RevokeSession, UpdateElo,
    };
    use crate::crank_ix::{ScheduleTimeCheck, CrankTimeCheck, ScheduleTimeCheckArgs};
    use crate::delegation_ix::{
        AuthorizeSessionCtx, DelegateGameCtx, InitializeAfterUndelegation, RevokeSessionCtx,
        UndelegateGameCtx,
    };
    use crate::game_ix::{CancelGame, CreateGame, EndGame, JoinGame, ResignGame, ClaimTimeout};
    use crate::governance_ix::{DisputeGame, ResolveDispute};
    use crate::moves_ix::{CommitMoveBatchCtx, RecordMove};
    use ephemeral_rollups_sdk::cpi::undelegate_account;

    pub fn init_profile(ctx: Context<InitProfile>, username: String) -> Result<()> {
        crate::account_ix::profile::handler(ctx, username)
    }

    pub fn verify_profile(ctx: Context<VerifyProfile>) -> Result<()> {
        crate::account_ix::profile::verify_handler(ctx)
    }

    pub fn set_username(ctx: Context<SetUsername>, username: String) -> Result<()> {
        crate::account_ix::set_username::handler(ctx, username)
    }

    pub fn create_game(
        ctx: Context<CreateGame>,
        game_id: u64,
        wager_amount: u64,
        game_type: state::GameType,
        match_type: state::MatchType,
        country: String,
        time_per_move: u16,
    ) -> Result<()> {
        crate::game_ix::create::handler(ctx, game_id, wager_amount, game_type, match_type, country, time_per_move)
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

    pub fn finalize_game(ctx: Context<EndGame>, game_id: u64) -> Result<()> {
        crate::game_ix::finalize::handler(ctx, game_id)
    }

    pub fn resign(ctx: Context<ResignGame>, game_id: u64) -> Result<()> {
        crate::game_ix::resign::handler(ctx, game_id)
    }

    pub fn claim_timeout(ctx: Context<ClaimTimeout>, game_id: u64) -> Result<()> {
        crate::game_ix::timeout::handler(ctx, game_id)
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

    pub fn initialize_tournament(
        ctx: Context<InitializeTournament>,
        tournament_id: u64,
        name: String,
        entry_fee: u64,
        max_players: u16,
        prize_shares: [u16; 4],
    ) -> Result<()> {
        crate::tournament_ix::initialize::handler(ctx, tournament_id, name, entry_fee, max_players, prize_shares)
    }

    pub fn initialize_match(
        ctx: Context<InitializeMatch>,
        tournament_id: u64,
        match_index: u16,
        round: u8,
        player_white: Option<Pubkey>,
        player_black: Option<Pubkey>,
        next_match_for_winner: Option<u16>,
        next_match_slot: u8,
    ) -> Result<()> {
        crate::tournament_ix::initialize_match::handler(
            ctx, tournament_id, match_index, round, player_white, player_black,
            next_match_for_winner, next_match_slot
        )
    }

    pub fn register_player(ctx: Context<RegisterPlayer>, tournament_id: u64) -> Result<()> {
        crate::tournament_ix::register::handler(ctx, tournament_id)
    }

    pub fn start_tournament(ctx: Context<StartTournament>, tournament_id: u64) -> Result<()> {
        crate::tournament_ix::start::handler(ctx, tournament_id)
    }

    pub fn record_match_result(
        ctx: Context<RecordMatchResult>,
        tournament_id: u64,
        match_index: u16,
        winner: Pubkey,
        loser: Pubkey,
    ) -> Result<()> {
        crate::tournament_ix::record_result::handler(ctx, tournament_id, match_index, winner, loser)
    }

    pub fn advance_winner(
        ctx: Context<AdvanceWinner>,
        tournament_id: u64,
        source_match_index: u16,
        _target_match_index: u16,
    ) -> Result<()> {
        crate::tournament_ix::record_result::handler_advance_winner(ctx, tournament_id, source_match_index)
    }

    pub fn claim_tournament_prize(
        ctx: Context<ClaimTournamentPrize>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::claim_prize::handler(ctx, tournament_id)
    }

    pub fn cancel_tournament<'a, 'b, 'c, 'info>(
        ctx: Context<'a, 'b, 'c, 'info, CancelTournament<'info>>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::cancel::handler(ctx, tournament_id)
    }

    // ── Fee Vault ──────────────────────────────────────────────────────────────

    pub fn initialize_fee_vault(
        ctx: Context<InitializeFeeVault>,
        host_wallet: Pubkey,
    ) -> Result<()> {
        crate::account_ix::fee_vault_ix::handler_initialize_fee_vault(ctx, host_wallet)
    }

    pub fn collect_fee(ctx: Context<CollectFee>, amount: u64) -> Result<()> {
        crate::account_ix::fee_vault_ix::handler_collect_fee(ctx, amount)
    }

    pub fn claim_fees(ctx: Context<ClaimFees>) -> Result<u64> {
        crate::account_ix::fee_vault_ix::handler_claim_fees(ctx)
    }

    // ── Player Session ────────────────────────────────────────────────────────

    pub fn create_session(
        ctx: Context<CreateSession>,
        session_key: Pubkey,
        duration: Option<i64>,
        spending_limit: Option<u64>,
        max_wager: Option<u64>,
    ) -> Result<()> {
        crate::account_ix::fee_vault_ix::handler_create_session(
            ctx, session_key, duration, spending_limit, max_wager,
        )
    }

    pub fn revoke_session(ctx: Context<RevokeSession>) -> Result<()> {
        crate::account_ix::fee_vault_ix::handler_revoke_session(ctx)
    }

    // ── ELO Update ────────────────────────────────────────────────────────────

    pub fn update_elo(
        ctx: Context<UpdateElo>,
        opponent_rating: u32,
        opponent_rd: u32,
        outcome: u32,
        is_ranked: bool,
        wager: u64,
        won_amount: u64,
    ) -> Result<()> {
        crate::account_ix::fee_vault_ix::handler_update_elo(
            ctx, opponent_rating, opponent_rd, outcome, is_ranked, wager, won_amount,
        )
    }

    // ── Crank (Scheduled Tasks) ─────────────────────────────────────────────────

    /// Schedule an automatic time check crank for a game
    pub fn schedule_time_check(
        ctx: Context<ScheduleTimeCheck>,
        args: ScheduleTimeCheckArgs,
    ) -> Result<()> {
        crate::crank_ix::schedule_time_check_crank(ctx, args)
    }

    /// Automatic time check called by the scheduled crank
    pub fn crank_time_check(ctx: Context<CrankTimeCheck>) -> Result<()> {
        crate::crank_ix::crank_time_check(ctx)
    }
}
