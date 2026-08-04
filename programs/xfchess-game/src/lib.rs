//! Main entry point mapping all Anchor program instructions to their handler functions.

use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::cpi::undelegate_account;

pub mod account_ix;
pub mod common;
pub mod constants;
#[cfg(feature = "cranks")]
pub mod crank_ix;
pub mod delegation_ix;
pub mod elo;
pub mod errors;
pub mod events;
pub mod game_ix;
pub mod governance_ix;
pub mod lifecycle;
pub mod magicblock;
pub mod moves_ix;
pub mod state;
pub mod tournament_ix;

// Re-export account structs at crate root so Anchor's generated __private::__global handlers
// can find them via their `use super::*` chain.
pub use account_ix::{
    AcceptFriendRequest, AuthorizeGlobalSessionArgs, AuthorizeGlobalSessionCtx, BlockUser,
    CloseFriendship, CreateSession, InitProfile, LinkExternalElo, RevokeGlobalSessionCtx,
    RevokeSession, SendFriendRequest, SetUsername, UpdateElo, VerifyProfile, WithdrawExpiredWager,
    WithdrawGlobalSessionCtx, WithdrawTreasury,
};
#[cfg(feature = "cranks")]
pub use crank_ix::{
    cancel_time_check_crank, crank_time_check, crank_time_check::CrankTimeCheckData,
    schedule_time_check_crank, CancelTimeCheck, CancelTimeCheckArgs, CrankTimeCheck,
    ScheduleTimeCheck, ScheduleTimeCheckArgs,
};
pub use delegation_ix::{
    AuthorizeSessionCtx, DelegateGameCtx, ForceUndelegateAfterTimeoutCtx,
    InitializeAfterUndelegation, RequestForceUndelegateCtx, RevokeSessionCtx, UndelegateGameCtx,
};
pub use game_ix::{
    AcceptDraw, CancelGame, ClaimTimeout, CreateGame, EndGame, GlobalCreateGame, GlobalJoinGame,
    JoinGame, OfferDraw, ResignGame,
};
pub use governance_ix::{ClaimStaleDispute, DisputeGame, RecoverStuckDelegation, ResolveDispute};
pub use moves_ix::{GlobalRecordMove, RecordMove};
pub use tournament_ix::{
    AdvanceRound, AdvanceWinner, AuthorizeTournamentSessionArgs, AuthorizeTournamentSessionCtx,
    CancelTournament, ClaimTournamentPrize, CloseTournament, CompleteSwissTournament,
    DistributeTournamentPrizes, FundSolPrize, FundUsdcPrize, InitializeMatch,
    InitializeShardsMedium, InitializeShardsSmall, InitializeTournament,
    InitializeTournamentEscrow, InitializeTournamentShards, LeaveTournament, RecordMatchResult,
    RecordSwissResult, RegisterPlayer, RevokeTournamentSessionCtx, SessionCreateGame,
    SessionJoinGame, StartTournament, SwissMatchResult,
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
pub mod __client_accounts_link_external_elo {
    pub use crate::account_ix::link_external_elo::__client_accounts_link_external_elo::*;
}
pub mod __client_accounts_send_friend_request {
    pub use crate::account_ix::friends_ix::__client_accounts_send_friend_request::*;
}
pub mod __client_accounts_accept_friend_request {
    pub use crate::account_ix::friends_ix::__client_accounts_accept_friend_request::*;
}
pub mod __client_accounts_close_friendship {
    pub use crate::account_ix::friends_ix::__client_accounts_close_friendship::*;
}
pub mod __client_accounts_block_user {
    pub use crate::account_ix::friends_ix::__client_accounts_block_user::*;
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
pub mod __client_accounts_request_force_undelegate_ctx {
    pub use crate::delegation_ix::force_recovery::__client_accounts_request_force_undelegate_ctx::*;
}
pub mod __client_accounts_force_undelegate_after_timeout_ctx {
    pub use crate::delegation_ix::force_recovery::__client_accounts_force_undelegate_after_timeout_ctx::*;
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
pub mod __client_accounts_offer_draw {
    pub use crate::game_ix::offer_draw::__client_accounts_offer_draw::*;
}
pub mod __client_accounts_accept_draw {
    pub use crate::game_ix::accept_draw::__client_accounts_accept_draw::*;
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
pub mod __client_accounts_record_move {
    pub use crate::moves_ix::record::__client_accounts_record_move::*;
}
pub mod __client_accounts_global_record_move {
    pub use crate::moves_ix::global_record::__client_accounts_global_record_move::*;
}
pub mod __client_accounts_initialize_tournament {
    pub use crate::tournament_ix::lifecycle::initialize::__client_accounts_initialize_tournament::*;
}
pub mod __client_accounts_initialize_tournament_shards {
    pub use crate::tournament_ix::lifecycle::initialize_shards::__client_accounts_initialize_tournament_shards::*;
}
pub mod __client_accounts_initialize_shards_small {
    pub use crate::tournament_ix::lifecycle::initialize_shards::__client_accounts_initialize_shards_small::*;
}
pub mod __client_accounts_initialize_shards_medium {
    pub use crate::tournament_ix::lifecycle::initialize_shards::__client_accounts_initialize_shards_medium::*;
}
pub mod __client_accounts_initialize_tournament_escrow {
    pub use crate::tournament_ix::lifecycle::initialize_escrow::__client_accounts_initialize_tournament_escrow::*;
}
pub mod __client_accounts_register_player {
    pub use crate::tournament_ix::registration::register::__client_accounts_register_player::*;
}
pub mod __client_accounts_leave_tournament {
    pub use crate::tournament_ix::registration::leave::__client_accounts_leave_tournament::*;
}
pub mod __client_accounts_start_tournament {
    pub use crate::tournament_ix::lifecycle::start::__client_accounts_start_tournament::*;
}
pub mod __client_accounts_record_match_result {
    pub use crate::tournament_ix::matches::record_result::__client_accounts_record_match_result::*;
}
pub mod __client_accounts_advance_winner {
    pub use crate::tournament_ix::matches::record_result::__client_accounts_advance_winner::*;
}
pub mod __client_accounts_initialize_match {
    pub use crate::tournament_ix::matches::initialize_match::__client_accounts_initialize_match::*;
}
pub mod __client_accounts_claim_tournament_prize {
    pub use crate::tournament_ix::prizes::claim_prize::__client_accounts_claim_tournament_prize::*;
}
pub mod __client_accounts_distribute_tournament_prizes {
    pub use crate::tournament_ix::prizes::distribute::__client_accounts_distribute_tournament_prizes::*;
}
pub mod __client_accounts_cancel_tournament {
    pub use crate::tournament_ix::lifecycle::cancel::__client_accounts_cancel_tournament::*;
}
pub mod __client_accounts_close_tournament {
    pub use crate::tournament_ix::lifecycle::close_tournament::__client_accounts_close_tournament::*;
}
pub mod __client_accounts_fund_usdc_prize {
    pub use crate::tournament_ix::prizes::fund_prize::__client_accounts_fund_usdc_prize::*;
}
pub mod __client_accounts_fund_sol_prize {
    pub use crate::tournament_ix::prizes::fund_sol_prize::__client_accounts_fund_sol_prize::*;
}
pub mod __client_accounts_record_swiss_result {
    pub use crate::tournament_ix::matches::record_swiss_result::__client_accounts_record_swiss_result::*;
}
pub mod __client_accounts_advance_round {
    pub use crate::tournament_ix::matches::advance_round::__client_accounts_advance_round::*;
}
pub mod __client_accounts_complete_swiss_tournament {
    pub use crate::tournament_ix::matches::complete_swiss::__client_accounts_complete_swiss_tournament::*;
}
pub mod __client_accounts_authorize_tournament_session_ctx {
    pub use crate::tournament_ix::session::authorize_tournament_session::__client_accounts_authorize_tournament_session_ctx::*;
}
pub mod __client_accounts_revoke_tournament_session_ctx {
    pub use crate::tournament_ix::session::authorize_tournament_session::__client_accounts_revoke_tournament_session_ctx::*;
}
pub mod __client_accounts_session_create_game {
    pub use crate::tournament_ix::session::session_create_game::__client_accounts_session_create_game::*;
}
pub mod __client_accounts_session_join_game {
    pub use crate::tournament_ix::session::session_join_game::__client_accounts_session_join_game::*;
}
pub mod __client_accounts_withdraw_treasury {
    pub use crate::account_ix::treasury::__client_accounts_withdraw_treasury::*;
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
#[cfg(feature = "cranks")]
pub mod __client_accounts_schedule_time_check {
    pub use crate::crank_ix::schedule_time_check::__client_accounts_schedule_time_check::*;
}
#[cfg(feature = "cranks")]
pub mod __client_accounts_crank_time_check {
    pub use crate::crank_ix::crank_time_check::__client_accounts_crank_time_check::*;
}
#[cfg(feature = "cranks")]
pub mod __client_accounts_cancel_time_check {
    pub use crate::crank_ix::cancel_time_check::__client_accounts_cancel_time_check::*;
}
pub mod __client_accounts_claim_stale_dispute {
    pub use crate::governance_ix::claim_stale_dispute::__client_accounts_claim_stale_dispute::*;
}
pub mod __client_accounts_recover_stuck_delegation {
    pub use crate::governance_ix::recover_stuck_delegation::__client_accounts_recover_stuck_delegation::*;
}
pub mod __client_accounts_authorize_global_session_ctx {
    pub use crate::account_ix::global_session_ix::__client_accounts_authorize_global_session_ctx::*;
}
pub mod __client_accounts_revoke_global_session_ctx {
    pub use crate::account_ix::global_session_ix::__client_accounts_revoke_global_session_ctx::*;
}
pub mod __client_accounts_withdraw_global_session_ctx {
    pub use crate::account_ix::global_session_ix::__client_accounts_withdraw_global_session_ctx::*;
}
pub mod __client_accounts_global_create_game {
    pub use crate::game_ix::global_create::__client_accounts_global_create_game::*;
}
pub mod __client_accounts_global_join_game {
    pub use crate::game_ix::global_join::__client_accounts_global_join_game::*;
}

#[allow(unused_imports)]
use ephemeral_rollups_sdk::anchor::MagicProgram;

declare_id!("8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU");

#[program]
pub mod xfchess_game {
    use super::*;

    pub fn init_profile(
        ctx: Context<InitProfile>,
        username: String,
        country: String,
        date_of_birth: i64,
    ) -> Result<()> {
        crate::account_ix::profile::handler(ctx, username, country, date_of_birth)
    }

    pub fn verify_profile(ctx: Context<VerifyProfile>) -> Result<()> {
        crate::account_ix::profile::verify_handler(ctx)
    }

    pub fn set_username(ctx: Context<SetUsername>, username: String) -> Result<()> {
        crate::account_ix::set_username::handler(ctx, username)
    }

    pub fn link_external_elo(
        ctx: Context<LinkExternalElo>,
        username: String,
        blitz_rating: u32,
        rapid_rating: u32,
        bullet_rating: u32,
    ) -> Result<()> {
        crate::account_ix::link_external_elo::handler(
            ctx,
            username,
            blitz_rating,
            rapid_rating,
            bullet_rating,
        )
    }

    // ── Solana Friends ────────────────────────────────────────────────────────

    pub fn send_friend_request(ctx: Context<SendFriendRequest>) -> Result<()> {
        crate::account_ix::friends_ix::send_request(ctx)
    }

    pub fn accept_friend_request(ctx: Context<AcceptFriendRequest>) -> Result<()> {
        crate::account_ix::friends_ix::accept_request(ctx)
    }

    pub fn close_friendship(ctx: Context<CloseFriendship>) -> Result<()> {
        crate::account_ix::friends_ix::close_friendship(ctx)
    }

    pub fn block_user(ctx: Context<BlockUser>) -> Result<()> {
        crate::account_ix::friends_ix::block_user(ctx)
    }

    pub fn create_game(
        ctx: Context<CreateGame>,
        game_id: u64,
        wager_amount: u64,
        match_type: state::MatchType,
        platform_fee: u64,
        base_time_seconds: u64,
        increment_seconds: u16,
    ) -> Result<()> {
        crate::game_ix::create::handler(
            ctx,
            game_id,
            wager_amount,
            match_type,
            platform_fee,
            base_time_seconds,
            increment_seconds,
        )
    }

    pub fn join_game(ctx: Context<JoinGame>, game_id: u64) -> Result<()> {
        crate::game_ix::join::handler(ctx, game_id)
    }

    pub fn record_move(
        ctx: Context<RecordMove>,
        game_id: u64,
        move_uci: [u8; 5],
        next_board: [u8; 68],
        nonce: u64,
        signature: Option<Vec<u8>>,
        parent_nonce: Option<u64>,
    ) -> Result<()> {
        crate::moves_ix::record::handler(
            ctx,
            game_id,
            move_uci,
            next_board,
            nonce,
            signature,
            parent_nonce,
        )
    }

    /// Same as `record_move`, but for games created via
    /// `global_create_game`/`global_join_game` — checks the mover's
    /// `GlobalSessionDelegation` instead of a per-game `SessionDelegation`
    /// (which those games never get, that's what makes create/join
    /// popup-free for them).
    pub fn global_record_move(
        ctx: Context<GlobalRecordMove>,
        game_id: u64,
        move_uci: [u8; 5],
        next_board: [u8; 68],
        nonce: u64,
        signature: Option<Vec<u8>>,
        parent_nonce: Option<u64>,
    ) -> Result<()> {
        crate::moves_ix::global_record::handler(
            ctx,
            game_id,
            move_uci,
            next_board,
            nonce,
            signature,
            parent_nonce,
        )
    }

    pub fn finalize_game(ctx: Context<EndGame>, game_id: u64) -> Result<()> {
        crate::game_ix::finalize::handler(ctx, game_id)
    }

    pub fn resign(ctx: Context<ResignGame>, game_id: u64) -> Result<()> {
        crate::game_ix::resign::handler(ctx, game_id)
    }

    /// Offers a draw to the opponent — does not end the game by itself. The
    /// opponent must call `accept_draw` to finalize it.
    pub fn offer_draw(ctx: Context<OfferDraw>, game_id: u64) -> Result<()> {
        crate::game_ix::offer_draw::handler(ctx, game_id)
    }

    /// Accepts the opponent's pending draw offer, ending the game as a draw.
    /// Settlement (pot split, ELO, stats) happens afterward via `finalize_game`.
    pub fn accept_draw(ctx: Context<AcceptDraw>, game_id: u64) -> Result<()> {
        crate::game_ix::accept_draw::handler(ctx, game_id)
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

    pub fn claim_stale_dispute(ctx: Context<ClaimStaleDispute>, game_id: u64) -> Result<()> {
        crate::governance_ix::claim_stale_dispute::handler(ctx, game_id)
    }

    /// Releases wager escrow from a `Game` PDA left wiped by
    /// `force_undelegate_after_timeout` — see
    /// `governance_ix::recover_stuck_delegation` for the full rationale.
    pub fn recover_stuck_delegation(
        ctx: Context<RecoverStuckDelegation>,
        game_id: u64,
    ) -> Result<()> {
        crate::governance_ix::recover_stuck_delegation::handler(ctx, game_id)
    }

    pub fn authorize_session_key(
        ctx: Context<AuthorizeSessionCtx>,
        game_id: u64,
        session_pubkey: Pubkey,
    ) -> Result<()> {
        crate::delegation_ix::session::handler_authorize_session_key(ctx, game_id, session_pubkey)
    }

    pub fn revoke_session_key(ctx: Context<RevokeSessionCtx>, game_id: u64) -> Result<()> {
        crate::delegation_ix::session::handler_revoke_session_key(ctx, game_id)
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

    /// Starts the non-admin, owner-program-authorized forced-undelegation
    /// countdown for a `Game` PDA whose ER validator has gone unreachable.
    /// See `delegation_ix::force_recovery` and MAGICBLOCK.md's
    /// "Failure Mode: ER Unavailability" section.
    pub fn request_force_undelegate(
        ctx: Context<RequestForceUndelegateCtx>,
        game_id: u64,
    ) -> Result<()> {
        crate::delegation_ix::force_recovery::handler_request_force_undelegate(ctx, game_id)
    }

    /// Completes a forced undelegation once the `request_force_undelegate`
    /// timeout has elapsed — hands the `Game` PDA back wiped to zero bytes
    /// (data-loss by design; see the handler's doc comment). Follow up with
    /// `recover_stuck_delegation` to release the escrow.
    pub fn force_undelegate_after_timeout(
        ctx: Context<ForceUndelegateAfterTimeoutCtx>,
        game_id: u64,
    ) -> Result<()> {
        crate::delegation_ix::force_recovery::handler_force_undelegate_after_timeout(ctx, game_id)
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
        // ephemeral_rollups_sdk::cpi::undelegate_account (0.16.2, our pinned
        // version) already rejects a buffer that isn't this specific account's
        // own canonical undelegate-buffer PDA internally (see
        // is_canonical_undelegation_buffer in the SDK's cpi.rs) — no need to
        // duplicate that check here.
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
        tournament_type: state::TournamentType,
        elo_min: u32,
        elo_max: u32,
        min_players: u16,
        prize_shares: [u16; 10],
        platform_fee: u64,
        winner_takes_all: bool,
        host_treasury: Pubkey,
        usdc_mint: Option<Pubkey>,
        base_time_seconds: u64,
        increment_seconds: u16,
    ) -> Result<()> {
        crate::tournament_ix::lifecycle::initialize::handler(
            ctx,
            tournament_id,
            name,
            entry_fee,
            max_players,
            tournament_type,
            elo_min,
            elo_max,
            min_players,
            prize_shares,
            platform_fee,
            winner_takes_all,
            host_treasury,
            usdc_mint,
            base_time_seconds,
            increment_seconds,
        )
    }

    pub fn initialize_tournament_shards(
        ctx: Context<InitializeTournamentShards>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::lifecycle::initialize_shards::handler(ctx, tournament_id)
    }

    /// Initialize shards for tournaments with ≤ 64 players (1 shard, ~0.034 SOL).
    pub fn initialize_shards_small(
        ctx: Context<InitializeShardsSmall>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::lifecycle::initialize_shards::handler_small(ctx, tournament_id)
    }

    /// Initialize shards for tournaments with ≤ 128 players (2 shards, ~0.068 SOL).
    pub fn initialize_shards_medium(
        ctx: Context<InitializeShardsMedium>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::lifecycle::initialize_shards::handler_medium(ctx, tournament_id)
    }

    pub fn initialize_tournament_escrow(
        ctx: Context<InitializeTournamentEscrow>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::lifecycle::initialize_escrow::handler(ctx, tournament_id)
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
        crate::tournament_ix::matches::initialize_match::handler(
            ctx,
            tournament_id,
            match_index,
            round,
            player_white,
            player_black,
            next_match_for_winner,
            next_match_slot,
        )
    }

    pub fn register_player(
        ctx: Context<RegisterPlayer>,
        tournament_id: u64,
        elo: u32,
    ) -> Result<()> {
        crate::tournament_ix::registration::register::handler(ctx, tournament_id, elo)
    }

    pub fn leave_tournament(ctx: Context<LeaveTournament>, tournament_id: u64) -> Result<()> {
        crate::tournament_ix::registration::leave::handler(ctx, tournament_id)
    }

    pub fn start_tournament(ctx: Context<StartTournament>, tournament_id: u64) -> Result<()> {
        crate::tournament_ix::lifecycle::start::handler(ctx, tournament_id)
    }

    pub fn record_match_result(
        ctx: Context<RecordMatchResult>,
        tournament_id: u64,
        match_index: u16,
        winner: Pubkey,
        loser: Pubkey,
    ) -> Result<()> {
        crate::tournament_ix::matches::record_result::handler(
            ctx,
            tournament_id,
            match_index,
            winner,
            loser,
        )
    }

    pub fn advance_winner(
        ctx: Context<AdvanceWinner>,
        tournament_id: u64,
        source_match_index: u16,
        _target_match_index: u16,
    ) -> Result<()> {
        crate::tournament_ix::matches::record_result::handler_advance_winner(
            ctx,
            tournament_id,
            source_match_index,
        )
    }

    pub fn claim_tournament_prize(
        ctx: Context<ClaimTournamentPrize>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::prizes::claim_prize::handler(ctx, tournament_id)
    }

    pub fn distribute_tournament_prizes<'info>(
        ctx: Context<'info, DistributeTournamentPrizes<'info>>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::prizes::distribute::handler(ctx, tournament_id)
    }

    pub fn cancel_tournament<'info>(
        ctx: Context<'info, CancelTournament<'info>>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::lifecycle::cancel::handler(ctx, tournament_id)
    }

    /// Finalize a completed tournament: transition it to `Closed` and sweep any
    /// residual escrow to the platform treasury. Payouts happen exclusively via
    /// `distribute_tournament_prizes` / `claim_tournament_prize`; this refuses to
    /// run until every funded prize place has already been claimed.
    pub fn close_tournament(ctx: Context<CloseTournament>, tournament_id: u64) -> Result<()> {
        crate::tournament_ix::lifecycle::close_tournament::handler(ctx, tournament_id)
    }

    pub fn fund_usdc_prize(
        ctx: Context<FundUsdcPrize>,
        tournament_id: u64,
        amount: u64,
    ) -> Result<()> {
        crate::tournament_ix::prizes::fund_prize::handler(ctx, tournament_id, amount)
    }

    pub fn fund_sol_prize(
        ctx: Context<FundSolPrize>,
        tournament_id: u64,
        amount: u64,
    ) -> Result<()> {
        crate::tournament_ix::prizes::fund_sol_prize::handler(ctx, tournament_id, amount)
    }

    pub fn record_swiss_result(
        ctx: Context<RecordSwissResult>,
        tournament_id: u64,
        round: u8,
        board: u16,
        result: SwissMatchResult,
    ) -> Result<()> {
        crate::tournament_ix::matches::record_swiss_result::handler(
            ctx,
            tournament_id,
            round,
            board,
            result,
        )
    }

    /// Advance a Swiss tournament to its next round once every board in the
    /// current round has reported (see `advance_round::handler` for why this
    /// is permissionless — the point is a tournament can progress without
    /// the backend scheduler alive to decide "the round is over").
    pub fn advance_round(ctx: Context<AdvanceRound>, tournament_id: u64) -> Result<()> {
        crate::tournament_ix::matches::advance_round::handler(ctx, tournament_id)
    }

    /// Finalize a Swiss tournament once `advance_round` has pushed
    /// `current_round` to `total_rounds`: sorts final standings and marks the
    /// tournament `Completed` so prize distribution/close can proceed. See
    /// `complete_swiss::handler` for why this instruction exists — Swiss
    /// otherwise had no terminal step equivalent to single-elimination's
    /// final-match auto-completion.
    pub fn complete_swiss_tournament(
        ctx: Context<CompleteSwissTournament>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::matches::complete_swiss::handler(ctx, tournament_id)
    }

    // ── Tournament-scoped session delegation ───────────────────────────────

    /// Authorize a `session_key` to co-sign game and swiss-result ixs for
    /// `tournament_id` on behalf of the registered player, without a wallet
    /// popup per match. See [`AuthorizeTournamentSessionArgs`] for caps.
    pub fn authorize_tournament_session(
        ctx: Context<AuthorizeTournamentSessionCtx>,
        tournament_id: u64,
        args: AuthorizeTournamentSessionArgs,
    ) -> Result<()> {
        crate::tournament_ix::session::authorize_tournament_session::handler_authorize_tournament_session(
            ctx,
            tournament_id,
            args,
        )
    }

    /// Disable an existing tournament session delegation immediately.
    pub fn revoke_tournament_session(
        ctx: Context<RevokeTournamentSessionCtx>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::session::authorize_tournament_session::handler_revoke_tournament_session(
            ctx,
            tournament_id,
        )
    }

    /// Session-signed variant of `create_game` for tournament matches.
    /// The session key (not the player wallet) co-signs; wager and rent
    /// are drawn from the delegation PDA vault.
    pub fn session_create_game(
        ctx: Context<SessionCreateGame>,
        tournament_id: u64,
        game_id: u64,
        wager_amount: u64,
        match_type: state::MatchType,
        platform_fee: u64,
        base_time_seconds: u64,
        increment_seconds: u16,
    ) -> Result<()> {
        crate::tournament_ix::session::session_create_game::handler(
            ctx,
            tournament_id,
            game_id,
            wager_amount,
            match_type,
            platform_fee,
            base_time_seconds,
            increment_seconds,
        )
    }

    /// Session-signed variant of `join_game` for tournament matches.
    /// The session key (not the player wallet) co-signs; wager and fees
    /// are drawn from the delegation PDA vault.
    pub fn session_join_game(
        ctx: Context<SessionJoinGame>,
        tournament_id: u64,
        game_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::session::session_join_game::handler(ctx, tournament_id, game_id)
    }

    // ── Global persistent session delegation ──────────────────────────────────

    /// Create (or refresh) a global persistent session key for `player`.
    /// After this call the session key can co-sign `global_create_game` and
    /// `global_join_game` without a wallet popup — for the configured number
    /// of games and spending budget.
    pub fn authorize_global_session(
        ctx: Context<AuthorizeGlobalSessionCtx>,
        args: AuthorizeGlobalSessionArgs,
    ) -> Result<()> {
        crate::account_ix::global_session_ix::handler_authorize_global_session(ctx, args)
    }

    /// Immediately disable a global session key.
    pub fn revoke_global_session(ctx: Context<RevokeGlobalSessionCtx>) -> Result<()> {
        crate::account_ix::global_session_ix::handler_revoke_global_session(ctx)
    }

    /// Return the unspent balance of the global-session vault to the player.
    pub fn withdraw_global_session(ctx: Context<WithdrawGlobalSessionCtx>) -> Result<()> {
        crate::account_ix::global_session_ix::handler_withdraw_global_session(ctx)
    }

    /// Session-signed `create_game`. The session key co-signs; wager and rent
    /// are drawn from the [`GlobalSessionDelegation`] vault — zero wallet popup.
    pub fn global_create_game(
        ctx: Context<GlobalCreateGame>,
        game_id: u64,
        wager_amount: u64,
        match_type: state::MatchType,
        platform_fee: u64,
        base_time_seconds: u64,
        increment_seconds: u16,
    ) -> Result<()> {
        crate::game_ix::global_create::handler(
            ctx,
            game_id,
            wager_amount,
            match_type,
            platform_fee,
            base_time_seconds,
            increment_seconds,
        )
    }

    /// Session-signed `join_game`. The session key co-signs; wager is drawn
    /// from the [`GlobalSessionDelegation`] vault — zero wallet popup.
    pub fn global_join_game(ctx: Context<GlobalJoinGame>, game_id: u64) -> Result<()> {
        crate::game_ix::global_join::handler(ctx, game_id)
    }

    /// Withdraw accumulated platform fees from the treasury vault to a
    /// destination wallet. Only the treasury authority may call this.
    pub fn withdraw_treasury(ctx: Context<WithdrawTreasury>, amount: u64) -> Result<()> {
        crate::account_ix::treasury::handler(ctx, amount)
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
            ctx,
            session_key,
            duration,
            spending_limit,
            max_wager,
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
            ctx,
            opponent_rating,
            opponent_rd,
            outcome,
            is_ranked,
            wager,
            won_amount,
        )
    }

    // ── Crank (Scheduled Tasks) ─────────────────────────────────────────────────

    /// Schedule an automatic time check crank for a game
    #[cfg(feature = "cranks")]
    pub fn schedule_time_check(
        ctx: Context<ScheduleTimeCheck>,
        args: ScheduleTimeCheckArgs,
    ) -> Result<()> {
        crate::crank_ix::schedule_time_check_crank(ctx, args)
    }

    /// Automatic time check called by the scheduled crank
    #[cfg(feature = "cranks")]
    pub fn crank_time_check(ctx: Context<CrankTimeCheck>, _data: CrankTimeCheckData) -> Result<()> {
        crate::crank_ix::crank_time_check::crank_time_check(ctx, _data)
    }

    /// Cancel a previously scheduled time check crank
    #[cfg(feature = "cranks")]
    pub fn cancel_time_check(
        ctx: Context<CancelTimeCheck>,
        args: CancelTimeCheckArgs,
    ) -> Result<()> {
        crate::crank_ix::cancel_time_check_crank(ctx, args)
    }
}
