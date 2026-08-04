//! Solana Friends — on-chain friendship lifecycle.
//!
//! One [`Friendship`] PDA per undirected pair, seeds:
//!   `["friendship", party_a, party_b]`  where `party_a < party_b` (canonical).
//! The client passes the two parties already sorted; the handler enforces the
//! ordering (which also rules out self-friendship). Either party may be the
//! signing `requester`; the other becomes the `addressee`.
//!
//! These instructions are intentionally small and may be co-signed by the
//! player's global session key so adding/accepting a friend costs no wallet
//! popup (see `account_ix/global_session_ix.rs`).

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

// ── send_friend_request ──────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct SendFriendRequest<'info> {
    #[account(
        init,
        payer = requester,
        space = 8 + Friendship::INIT_SPACE,
        seeds = [FRIENDSHIP_SEED, party_a.key().as_ref(), party_b.key().as_ref()],
        bump
    )]
    pub friendship: Account<'info, Friendship>,
    /// CHECK: lower-ordered party pubkey (canonical seed); validated in handler.
    pub party_a: AccountInfo<'info>,
    /// CHECK: higher-ordered party pubkey (canonical seed); validated in handler.
    pub party_b: AccountInfo<'info>,
    #[account(mut)]
    pub requester: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn send_request(ctx: Context<SendFriendRequest>) -> Result<()> {
    let a = ctx.accounts.party_a.key();
    let b = ctx.accounts.party_b.key();
    // Strict canonical ordering also rules out self-friendship (a == b).
    require!(a < b, GameErrorCode::InvalidFriendPair);

    let signer = ctx.accounts.requester.key();
    require!(
        signer == a || signer == b,
        GameErrorCode::UnauthorizedAccess
    );
    let addressee = if signer == a { b } else { a };

    let now = Clock::get()?.unix_timestamp;
    let f = &mut ctx.accounts.friendship;
    f.requester = signer;
    f.addressee = addressee;
    f.status = FriendStatus::Pending;
    f.created_at = now;
    f.accepted_at = 0;
    f.bump = ctx.bumps.friendship;
    Ok(())
}

// ── accept_friend_request ────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct AcceptFriendRequest<'info> {
    #[account(
        mut,
        seeds = [FRIENDSHIP_SEED, party_a.key().as_ref(), party_b.key().as_ref()],
        bump = friendship.bump,
    )]
    pub friendship: Account<'info, Friendship>,
    /// CHECK: canonical seed.
    pub party_a: AccountInfo<'info>,
    /// CHECK: canonical seed.
    pub party_b: AccountInfo<'info>,
    pub addressee: Signer<'info>,
}

pub fn accept_request(ctx: Context<AcceptFriendRequest>) -> Result<()> {
    let f = &mut ctx.accounts.friendship;
    require!(
        f.status == FriendStatus::Pending,
        GameErrorCode::FriendNotPending
    );
    require!(
        ctx.accounts.addressee.key() == f.addressee,
        GameErrorCode::UnauthorizedAccess
    );
    f.status = FriendStatus::Accepted;
    f.accepted_at = Clock::get()?.unix_timestamp;
    Ok(())
}

// ── close_friendship (decline / cancel / remove) ─────────────────────────────

#[derive(Accounts)]
pub struct CloseFriendship<'info> {
    #[account(
        mut,
        seeds = [FRIENDSHIP_SEED, party_a.key().as_ref(), party_b.key().as_ref()],
        bump = friendship.bump,
        close = signer,
    )]
    pub friendship: Account<'info, Friendship>,
    /// CHECK: canonical seed.
    pub party_a: AccountInfo<'info>,
    /// CHECK: canonical seed.
    pub party_b: AccountInfo<'info>,
    #[account(mut)]
    pub signer: Signer<'info>,
}

/// Declines a pending request, cancels one's own request, or removes an
/// accepted friend — all by closing the edge and refunding rent to the signer.
pub fn close_friendship(ctx: Context<CloseFriendship>) -> Result<()> {
    let f = &ctx.accounts.friendship;
    let s = ctx.accounts.signer.key();
    require!(
        s == f.requester || s == f.addressee,
        GameErrorCode::UnauthorizedAccess
    );
    Ok(())
}

// ── block_user ───────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct BlockUser<'info> {
    #[account(
        mut,
        seeds = [FRIENDSHIP_SEED, party_a.key().as_ref(), party_b.key().as_ref()],
        bump = friendship.bump,
    )]
    pub friendship: Account<'info, Friendship>,
    /// CHECK: canonical seed.
    pub party_a: AccountInfo<'info>,
    /// CHECK: canonical seed.
    pub party_b: AccountInfo<'info>,
    pub signer: Signer<'info>,
}

/// Marks an existing edge as `Blocked`. Either party may block.
pub fn block_user(ctx: Context<BlockUser>) -> Result<()> {
    let s = ctx.accounts.signer.key();
    let f = &mut ctx.accounts.friendship;
    require!(
        s == f.requester || s == f.addressee,
        GameErrorCode::UnauthorizedAccess
    );
    f.status = FriendStatus::Blocked;
    Ok(())
}
