//! Account structure mapping unique linked Lichess usernames back to player profiles.

use anchor_lang::prelude::*;

/// LichessUsernameRecord PDA ensures a linked Lichess username can only ever
/// be attached to one player profile — closing a gap `link_external_elo` had
/// no protection against at all: before this record existed, the same
/// Lichess handle (and the external ELO it seeds) could be attached to any
/// number of different wallets' profiles, unlike local in-game usernames,
/// which `UsernameRecord` already protects. Mirrors `UsernameRecord`'s shape
/// and first-claimant-wins claim semantics exactly (see
/// `account_ix::link_external_elo::handler`) — same as
/// `account_ix::profile::handler`'s existing model for local usernames.
/// Case-sensitive by seed, same as `UsernameRecord` — not a new limitation,
/// just consistent with the existing precedent (no case-folding there
/// either).
///
/// Known limitation, matching `UsernameRecord`'s own: there is no "unlink"
/// instruction, so a wallet that later links a *different* Lichess account
/// leaves its previous claim permanently orphaned (still pointing at that
/// wallet, never released). Out of scope for this fix — same shape as the
/// pre-existing local-username limitation, not introduced by this change.
///
/// Seeds: `[LICHESS_USERNAME_SEED, lichess_username.as_bytes()]`
#[account]
pub struct LichessUsernameRecord {
    pub owner: Pubkey,   // Wallet that linked this Lichess username
    pub created_at: i64, // Timestamp when first linked
}

impl LichessUsernameRecord {
    pub const LEN: usize = 8 + 32 + 8; // Discriminator + Pubkey + i64
}
