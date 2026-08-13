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

declare_id!("JBt1hnamsAzvtggRZcom6zT5kg1eYM2R2yokqnocXUD7");

#[program]
pub mod xfchess_game {
    use super::*;

    /// Creates a player's `PlayerProfile` PDA on first use (or re-initializes
    /// a legacy one, reallocating it to the current account size). Enforces
    /// username uniqueness via a `UsernameRecord` PDA and an 18+ age gate
    /// computed from `date_of_birth` — accounts under 18 years old are
    /// rejected outright. This is the entry point every player must call
    /// before creating or joining games.
    pub fn init_profile(
        ctx: Context<InitProfile>,
        username: String,
        country: String,
        date_of_birth: i64,
    ) -> Result<()> {
        crate::account_ix::profile::handler(ctx, username, country, date_of_birth)
    }

    /// Marks a player's profile `is_verified = true` (KYC verification).
    /// Restricted to the configured `kyc_authority` (the VPS master key) —
    /// players cannot self-verify.
    pub fn verify_profile(ctx: Context<VerifyProfile>) -> Result<()> {
        crate::account_ix::profile::verify_handler(ctx)
    }

    /// Claims a `UsernameRecord` PDA for `username` and sets it on the
    /// caller's existing profile. Unlike `init_profile`'s username field,
    /// this uses plain `init` rather than `init_if_needed`, so it fails
    /// outright if the username is already taken by another player.
    pub fn set_username(ctx: Context<SetUsername>, username: String) -> Result<()> {
        crate::account_ix::set_username::handler(ctx, username)
    }

    /// Writes a verified Lichess rating attestation (blitz/rapid/bullet) onto
    /// a player's profile. Only the configured `link_authority` (VPS backend
    /// signer, which verifies Lichess account ownership via a bio-nonce
    /// challenge off-chain) may call this — players cannot self-report
    /// ratings. On the very first link, also seeds the on-chain `elo_rating`
    /// from the external rating so new players don't start at a default ELO.
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

    /// Opens a `Friendship` PDA (status `Pending`) between the signer and the
    /// other party. Seeds are the two pubkeys in canonical (ascending) order,
    /// which doubles as the self-friendship guard. Either party may be the
    /// signing requester; the other becomes the addressee who must accept.
    pub fn send_friend_request(ctx: Context<SendFriendRequest>) -> Result<()> {
        crate::account_ix::friends_ix::send_request(ctx)
    }

    /// Accepts a pending friend request, flipping the `Friendship` PDA's
    /// status to `Accepted`. Only the addressee (not the original requester)
    /// may accept.
    pub fn accept_friend_request(ctx: Context<AcceptFriendRequest>) -> Result<()> {
        crate::account_ix::friends_ix::accept_request(ctx)
    }

    /// Closes a `Friendship` PDA, refunding its rent to the caller. Used to
    /// decline a pending request, cancel one's own outgoing request, or
    /// remove an already-accepted friend — the same instruction covers all
    /// three, since they're just "delete this edge" from different states.
    pub fn close_friendship(ctx: Context<CloseFriendship>) -> Result<()> {
        crate::account_ix::friends_ix::close_friendship(ctx)
    }

    /// Marks an existing `Friendship` edge as `Blocked`. Either party may
    /// block; unlike `close_friendship` this does not refund rent or delete
    /// the record, so the block persists as a record rather than resetting
    /// to a clean slate.
    pub fn block_user(ctx: Context<BlockUser>) -> Result<()> {
        crate::account_ix::friends_ix::block_user(ctx)
    }

    /// Creates a new `Game` PDA and its wager escrow, seeding the board at
    /// the starting position. `fee_payer` is the VPS relayer wallet that
    /// covers account rent (reimbursed later via `fees_advanced`), kept
    /// separate from `player` (white) so game creation can be sponsored
    /// without the player holding SOL. Validates the wager against
    /// `MIN_WAGER_LAMPORTS`/`MAX_WAGER_AMOUNT` and, for a non-zero wager,
    /// transfers the creator's stake into escrow.
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

    /// Joins an open PvP lobby as black, matching the wager already escrowed
    /// by white. Transitions the game from `WaitingForOpponent` to `Active`
    /// and, for a SOL wager, transfers the joiner's stake into the same
    /// escrow PDA (SPL-token wagers are funded through a separate path).
    /// Reads `white_profile` to support cross-border platform-fee
    /// calculation between the two players' jurisdictions.
    pub fn join_game(ctx: Context<JoinGame>, game_id: u64) -> Result<()> {
        crate::game_ix::join::handler(ctx, game_id)
    }

    /// Validates and applies a single chess move, advancing `Game.board`,
    /// `move_count`, and clocks. Called via a per-game `SessionDelegation`
    /// key rather than the player's wallet directly, so moves during a fast
    /// game don't each need a wallet popup — `player` here is the session
    /// signer, and the real moving wallet is `session_delegation.player`.
    /// `parent_nonce`, when provided, must match `game.nonce` before the
    /// move — a causal-chain check that rejects moves built on stale state.
    /// Emits a `MoveEvent` so move history can be reconstructed from the
    /// transaction log instead of being stored on-chain (zero rent cost).
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

    /// Settles a finished game (checkmate, resignation, timeout, or accepted
    /// draw) through the single base-layer settlement path: splits the
    /// escrowed pot between winner/loser (or both, on a draw) net of the
    /// platform fee, updates both players' ELO (K=32) and stats, reimburses
    /// the recorded `fee_payer` relayer, and closes the `Game` PDA — rent
    /// goes back to `fee_payer`, never to whoever happens to call this, so
    /// an unconstrained caller can't steal the rent refund.
    pub fn finalize_game(ctx: Context<EndGame>, game_id: u64) -> Result<()> {
        crate::game_ix::finalize::handler(ctx, game_id)
    }

    /// Concedes the game as a loss for the calling player (must be white or
    /// black). Ends the game via `finish_by_resign`; actual payout/ELO/stats
    /// settlement still happens afterward through `finalize_game`.
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

    /// Permissionlessly awards victory to whichever player did NOT exceed
    /// their clock, once that player's remaining time has run out. `caller`
    /// need not be a participant — anyone (typically a crank) may trigger
    /// this once a clock has actually expired; it errors as a no-op
    /// otherwise. Settlement still happens afterward via `finalize_game`.
    pub fn claim_timeout(ctx: Context<ClaimTimeout>, game_id: u64) -> Result<()> {
        crate::game_ix::timeout::handler(ctx, game_id)
    }

    /// Lets a game's creator reclaim their escrowed wager from a lobby that
    /// never found an opponent within the 24h expiration window. Verifies
    /// the game is still `WaitingForOpponent`, the caller is `game.white`,
    /// and 24h have elapsed since creation, then refunds (SOL or SPL token)
    /// and marks the game `Expired`.
    pub fn withdraw_expired_wager(ctx: Context<WithdrawExpiredWager>, game_id: u64) -> Result<()> {
        crate::account_ix::withdraw::handler(ctx, game_id)
    }

    /// Cancels a game and refunds each joined player's escrowed wager in
    /// full (no fee taken). Allowed in three situations: the creator cancels
    /// their own still-open lobby; either player cancels an active game that
    /// has had zero moves played; or either player cancels a mid-game that's
    /// been stalled for 24h+ with no activity. Refuses to run while the game
    /// is delegated to an Ephemeral Rollup.
    pub fn cancel_game(ctx: Context<CancelGame>, game_id: u64) -> Result<()> {
        crate::game_ix::cancel::handler(ctx, game_id)
    }

    /// Opens a dispute (e.g. suspected cheating) on an `Active` or
    /// `Inactive` game. Moves the game to `Disputed` — freezing it out of
    /// normal settlement — creates a `DisputeRecord` PDA (`Pending`,
    /// `DISPUTE_TTL_SECS` to live) with the given reason and evidence hash,
    /// and requires the challenger to post a `DISPUTE_BOND_LAMPORTS` bond,
    /// which deters frivolous disputes since it's only refunded if the claim
    /// is upheld. Caller must be white or black in the game.
    pub fn dispute_game(
        ctx: Context<DisputeGame>,
        game_id: u64,
        reason: String,
        evidence_hash: [u8; 32],
    ) -> Result<()> {
        crate::governance_ix::dispute::handler(ctx, game_id, reason, evidence_hash)
    }

    /// Rules on a pending dispute. Restricted to the configured
    /// `dispute_authority`. Allocates the wager pot per the ruling (full pot
    /// to `winner`, or a 50/50 split on `None`/draw), then settles the
    /// challenger's bond: refunded if their claim was upheld or the game was
    /// ruled a draw, forfeited to the treasury if the ruling went against
    /// them. Sets the game to `Settled` so `finalize_game` cannot re-process
    /// it afterward.
    pub fn resolve_dispute(
        ctx: Context<ResolveDispute>,
        game_id: u64,
        resolution: String,
        winner: Option<Pubkey>,
    ) -> Result<()> {
        crate::governance_ix::resolve::handler(ctx, game_id, resolution, winner)
    }

    /// Permissionless fallback for a dispute the platform authority never
    /// reviewed: once `DISPUTE_TTL_SECS` (7 days) has elapsed past
    /// `dispute.expires_at`, any signer may call this to split the pot 50/50
    /// (each player gets their stake back), refund the challenger's bond
    /// (no fault was ruled), and settle the game as a draw — ensuring funds
    /// are never permanently locked by platform inaction.
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

    /// Creates a per-game `SessionDelegation` PDA for the calling player
    /// (must be white or black in `game_id`), authorizing `session_pubkey`
    /// to co-sign `record_move` on their behalf for the next 2 hours (max 10
    /// moves per batch). This is what lets a player play an entire fast game
    /// on the Ephemeral Rollup without a wallet popup per move. Distinct
    /// from both the standalone `create_session` (not tied to one game) and
    /// `authorize_global_session` (spans many games).
    pub fn authorize_session_key(
        ctx: Context<AuthorizeSessionCtx>,
        game_id: u64,
        session_pubkey: Pubkey,
    ) -> Result<()> {
        crate::delegation_ix::session::handler_authorize_session_key(ctx, game_id, session_pubkey)
    }

    /// Immediately disables the caller's own per-game `SessionDelegation`
    /// (`enabled = false`, `expires_at = now`), so the session key can no
    /// longer co-sign moves for this game.
    pub fn revoke_session_key(ctx: Context<RevokeSessionCtx>, game_id: u64) -> Result<()> {
        crate::delegation_ix::session::handler_revoke_session_key(ctx, game_id)
    }

    /// Hands the `Game` PDA's ownership over to the MagicBlock delegation
    /// program so subsequent `record_move` calls execute on the Ephemeral
    /// Rollup with sub-second latency instead of on the Solana base layer.
    /// The ER validator is chosen by the delegation program/magic-router
    /// (not pinned here) so games aren't all forced onto one region. Must be
    /// called before ER-side moves; `undelegate_game` reverses it.
    pub fn delegate_game(
        ctx: Context<DelegateGameCtx>,
        game_id: u64,
        valid_until: i64,
    ) -> Result<()> {
        crate::delegation_ix::delegate::handler_delegate_game(ctx, game_id, valid_until)
    }

    /// Commits the game's current state from the Ephemeral Rollup back to
    /// the base layer and undelegates the `Game` PDA, returning ownership to
    /// this program so mainnet/devnet instructions (like `finalize_game`)
    /// can act on it again. No payer-identity check, so the VPS session key
    /// can trigger this at game end without an extra wallet popup.
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

    /// CPI entry point invoked by the MagicBlock delegation program itself
    /// (not called directly by players or the backend) to finish committing
    /// an undelegated account's buffered state back onto its canonical PDA.
    /// Delegates to `ephemeral_rollups_sdk::cpi::undelegate_account`, which
    /// already rejects a buffer that isn't the account's own canonical
    /// undelegate-buffer PDA.
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

    /// Bootstraps a new tournament's `Tournament` PDA: validates player-count
    /// (must be one of 2/4/8/16/32/64/128/256), ELO range, and that prize
    /// shares sum to ≤10000 bps, then populates every field for the
    /// `Registration` phase. Restricted to the VPS authority. Does not
    /// create player shards or lock a prize — follow up with the matching
    /// `initialize_shards*` instruction and `fund_sol_prize`/
    /// `fund_usdc_prize` before opening registration to players.
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

    /// Initializes all 4 `TournamentPlayersShard` PDAs for the large tier
    /// (256-player tournaments only). Each shard holds up to 64 players'
    /// pubkeys, ELOs, and Swiss standings — sharded because a single PDA
    /// can't fit 256 players plus their metadata. For smaller tournaments
    /// use `initialize_shards_small`/`initialize_shards_medium` instead;
    /// this call fails if `tournament.max_players != 256`.
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

    /// Creates the tournament's SOL escrow PDA — a zero-data account that
    /// just holds lamports for entry-fee deposits and (later) the
    /// operator-funded guaranteed SOL prize. Must be called after
    /// `initialize_tournament` and before `register_player` or
    /// `fund_sol_prize`, both of which pay into this PDA.
    pub fn initialize_tournament_escrow(
        ctx: Context<InitializeTournamentEscrow>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::lifecycle::initialize_escrow::handler(ctx, tournament_id)
    }

    /// Creates a single `TournamentMatch` PDA (one bracket slot) with its
    /// round, seeded player(s), and where its winner advances to next.
    /// Called by the backend to lay out the single-elimination bracket after
    /// `start_tournament` locks in seeding. Restricted to the tournament's
    /// own authority; fails unless the tournament is `Active` and
    /// `match_index` is within `total_matches`.
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

    /// Opts a player into a tournament in `Registration` phase, checking ELO
    /// eligibility (`elo_min..=elo_max`), capacity, and no duplicate entry
    /// across shards, then slots them into the correct
    /// `TournamentPlayersShard` by registration order. For paid tournaments,
    /// the guaranteed prize must already be locked (`fund_sol_prize`/
    /// `fund_usdc_prize`) before anyone can register. The entry fee is
    /// deposited into the tournament escrow PDA as a refundable deposit — it
    /// is NOT prize money; it becomes operator revenue only once the
    /// tournament actually starts (see `start_tournament`).
    pub fn register_player(
        ctx: Context<RegisterPlayer>,
        tournament_id: u64,
        elo: u32,
    ) -> Result<()> {
        crate::tournament_ix::registration::register::handler(ctx, tournament_id, elo)
    }

    /// Withdraws a player from a tournament still in `Registration`,
    /// removing them from their shard and refunding their entry-fee deposit
    /// in full from the tournament escrow PDA — the operator's wallet is
    /// never touched, since fees haven't been swept there yet at this phase.
    pub fn leave_tournament(ctx: Context<LeaveTournament>, tournament_id: u64) -> Result<()> {
        crate::tournament_ix::registration::leave::handler(ctx, tournament_id)
    }

    /// Locks registration and transitions the tournament to `Active`: sorts
    /// all registered players by ELO descending to seed the bracket (or
    /// initializes Swiss standings), then sweeps every entry-fee deposit
    /// from escrow to `host_treasury` as operator revenue — what remains in
    /// escrow after this is exactly the guaranteed prize. Single-elimination
    /// requires a full bracket (`num_registered_players == max_players`);
    /// Swiss only requires `min_players`. Below that threshold, call
    /// `cancel_tournament` instead. Match/bracket PDAs are created
    /// separately via `initialize_match`.
    pub fn start_tournament(ctx: Context<StartTournament>, tournament_id: u64) -> Result<()> {
        crate::tournament_ix::lifecycle::start::handler(ctx, tournament_id)
    }

    /// Records the outcome of a single-elimination bracket match: sets
    /// `tournament_match.winner` and status `Completed`, and — if this was
    /// the final — marks the tournament `Completed` with `winner`/
    /// `second_place` set (and `third_place`/`fourth_place` inferred for the
    /// two semifinal losers). The tournament authority is the trusted source
    /// of truth here; results are not cross-checked against an on-chain
    /// `Game` account. Follow up with `advance_winner` to seed the winner
    /// into their next bracket slot (unless this was the final).
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

    /// Places a completed match's winner into the correct player slot
    /// (white or black, per `source_match.next_match_slot`) of the next
    /// round's `TournamentMatch`, and flips that match to `Pending` once
    /// both slots are filled. Called by the backend right after
    /// `record_match_result` to propagate winners through the bracket.
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

    /// Pull-based prize claim for a `Completed` tournament: looks up which
    /// of the (up to 10) finishing places the caller holds, checks it hasn't
    /// already been claimed (tracked via a bitflag), and pays out their %
    /// share from whichever pools are funded — the operator-guaranteed USDC
    /// pool (`usdc_prize_pool`), the operator-guaranteed SOL pool
    /// (`prize_pool`), or both. Either player or a permissionless crank
    /// (`distribute_tournament_prizes`) can trigger a given place's payout,
    /// but never both — the shared bitflag prevents double-paying.
    pub fn claim_tournament_prize(
        ctx: Context<ClaimTournamentPrize>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::prizes::claim_prize::handler(ctx, tournament_id)
    }

    /// Permissionless crank that pushes each unclaimed place's SOL prize
    /// share directly to its winner's wallet in one transaction, so winners
    /// receive funds without having to sign a claim themselves. Winner
    /// wallets are passed via `remaining_accounts`; a cranker cannot
    /// redirect funds because destinations are constrained to the place
    /// pubkeys already recorded on the `Tournament` account. Only handles
    /// lump-sum SOL payouts — USDC pools and vesting schedules stay on the
    /// pull-based `claim_tournament_prize` path. Idempotent: re-cranking
    /// after full distribution is a no-op.
    pub fn distribute_tournament_prizes<'info>(
        ctx: Context<'info, DistributeTournamentPrizes<'info>>,
        tournament_id: u64,
    ) -> Result<()> {
        crate::tournament_ix::prizes::distribute::handler(ctx, tournament_id)
    }

    /// Halts a tournament still in `Registration` or `Active` and refunds
    /// everyone. Entry-fee refund source depends on phase: during
    /// `Registration` the deposits still sit in escrow; once `Active` they
    /// were already swept to `host_treasury` at `start_tournament`, so
    /// `host_treasury` must co-sign to authorize those refunds. The
    /// operator-funded guaranteed prize (SOL and/or USDC, whichever was
    /// locked) is returned to the operator in full. This is also the correct
    /// path when `start_tournament` would fail with `MinPlayersNotReached`.
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

    /// Operator deposits `amount` USDC into the tournament's USDC prize
    /// escrow ATA, locking the guaranteed USDC prize pool. Must run while
    /// the tournament is still `Registration` and before any player has
    /// registered (`num_registered_players == 0`) and can only be called
    /// once (`usdc_prize_funded` flips permanently) — this keeps the prize
    /// provably fixed and independent of how many players eventually enter.
    pub fn fund_usdc_prize(
        ctx: Context<FundUsdcPrize>,
        tournament_id: u64,
        amount: u64,
    ) -> Result<()> {
        crate::tournament_ix::prizes::fund_prize::handler(ctx, tournament_id, amount)
    }

    /// Operator deposits `amount` lamports into the tournament escrow PDA,
    /// locking the guaranteed SOL prize pool (`tournament.prize_pool`).
    /// Same one-time, pre-registration constraint as `fund_usdc_prize`: must
    /// run during `Registration` before any player registers, and
    /// `prize_pool` can only be set once. Entry fees paid later by players
    /// never mix into this pool — they're operator revenue, not prize money.
    pub fn fund_sol_prize(
        ctx: Context<FundSolPrize>,
        tournament_id: u64,
        amount: u64,
    ) -> Result<()> {
        crate::tournament_ix::prizes::fund_sol_prize::handler(ctx, tournament_id, amount)
    }

    /// Records one Swiss-system board result for the tournament's
    /// `current_round` and updates both players' standings (score,
    /// Buchholz/Sonneborn-Berger tiebreakers, color balance) across
    /// whichever shard(s) they live in. Marks the board's bit in
    /// `round_boards_reported` so `advance_round` can verify the round is
    /// fully reported purely from on-chain state, without trusting an
    /// off-chain caller. A single call can only set its own board's bit —
    /// it can never skip or fake completion of the round.
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

    /// Creates a standalone `PlayerSession` PDA: a time-bounded (24h by
    /// default, or a custom `duration`/`spending_limit`/`max_wager`) session
    /// key that can sign game transactions without a wallet popup. Distinct
    /// from the per-game `authorize_session_key` (tied to one `Game`) and
    /// the multi-game `authorize_global_session` — this one has its own
    /// independent lifetime and spending cap, not scoped to any single game.
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

    /// Deactivates a `PlayerSession` and closes the PDA, refunding its rent
    /// to the player.
    pub fn revoke_session(ctx: Context<RevokeSession>) -> Result<()> {
        crate::account_ix::fee_vault_ix::handler_revoke_session(ctx)
    }

    // ── ELO Update ────────────────────────────────────────────────────────────

    /// Updates a player's lifetime stats after a game — wins/losses/draws,
    /// win streak, `games_played`, and (for ranked games) wagered/won
    /// totals. Restricted to the VPS backend authority. Does NOT touch the
    /// on-chain `elo_rating` itself, which is updated exclusively by
    /// `finalize_game` (K=32); `opponent_rating`/`opponent_rd` are accepted
    /// but unused, kept only for ABI compatibility with older callers.
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
