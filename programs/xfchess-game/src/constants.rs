//! Program-wide constants, discriminators, and magic numbers.

use anchor_lang::prelude::*;

// PDA seeds — each is the prefix byte string used to derive a Program Derived Address
// for the corresponding on-chain account type. Changing any seed is a breaking change.

#[constant]
pub const GAME_SEED: &[u8] = b"game"; // Derives the GameState PDA per game_id

#[constant]
pub const MOVE_LOG_SEED: &[u8] = b"move_log"; // Derives the MoveLog PDA that stores move history

#[constant]
pub const PROFILE_SEED: &[u8] = b"profile"; // Derives a player's on-chain profile (ELO, username)

#[constant]
pub const USERNAME_SEED: &[u8] = b"username"; // Derives the uniqueness-lock account for a chosen username

#[constant]
pub const FRIENDSHIP_SEED: &[u8] = b"friendship"; // Derives the Friendship PDA per undirected wallet pair (canonical sorted order)

#[constant]
pub const WAGER_ESCROW_SEED: &[u8] = b"escrow"; // Derives the SOL escrow vault that holds wager funds during a game

#[constant]
pub const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation"; // Derives a per-game session-key authorisation record

pub const TOURNAMENT_SEED: &[u8] = b"tournament"; // Derives the TournamentState PDA
pub const TOURNAMENT_PLAYERS_SEED: &[u8] = b"tourney_players"; // Derives the TournamentPlayersShard PDAs (4 shards with 64 players each, seeded with shard_id)
pub const TOURNAMENT_ESCROW_SEED: &[u8] = b"t_escrow"; // Derives the prize-pool escrow vault for a tournament
pub const TOURNAMENT_MATCH_SEED: &[u8] = b"t_match"; // Derives an individual match record within a tournament
pub const TOURNAMENT_USDC_PRIZE_SEED: &[u8] = b"t_usdc_prize"; // Derives the SPL token escrow for USDC prize pool

// ---------------------------------------------------------------------------
// Privileged authority keypairs
// ---------------------------------------------------------------------------
//
// TRUST MODEL: these single keys gate result-setting, ELO, fee collection, and
// dispute resolution. Compromise of any one is catastrophic (forge results /
// drain fees). Each role uses its own dedicated single wallet (no multisig).
// Before mainnet: rotate the devnet keys whose secrets were exposed in git
// history (see backend/.env handling); optionally move to hardware-backed signers.

/// The KYC/identity verification authority (VPS backend signer).
/// Called by `verify_profile` to mark a player as KYC-verified on-chain.
/// Public key: 2mh7zXgZHaeDnroJQQdHnLNiierWXdn43VnATbGdATZK
/// Private key stored in backend/.env as KYC_AUTHORITY_KEY and keys/kyc_authority.json
pub mod kyc_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array([
        0x1a, 0x4e, 0x9b, 0x62, 0xc3, 0x6f, 0x3f, 0xda, 0x95, 0x75, 0x85, 0xdd, 0x99, 0xd3, 0x5e,
        0x0d, 0x9f, 0x24, 0x6d, 0x4d, 0x17, 0x54, 0x6c, 0xb5, 0x01, 0x27, 0xaa, 0xbf, 0x15, 0x75,
        0xb3, 0x82,
    ]);
}

/// The platform dispute-resolution authority — the only signer allowed to
/// call `resolve_dispute`.
/// Public key: HAHgvXf6uYxTqEuUnkkzTS1EQD8sYd342zgxM2wdqpa2
/// Private key stored in backend/.env as DISPUTE_AUTHORITY_KEY and keys/dispute_authority.json
pub mod dispute_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array([
        0xf0, 0x1c, 0x16, 0x70, 0x78, 0x28, 0x62, 0x5a, 0xb2, 0x0b, 0xe0, 0x22, 0x42, 0x43, 0xd1,
        0x7c, 0xd7, 0x70, 0x4d, 0xd2, 0xbb, 0xd6, 0x3f, 0x03, 0x4f, 0xbb, 0x98, 0xd4, 0xca, 0x2f,
        0x3f, 0xd7,
    ]);
}

/// The external-elo linking authority — the only signer allowed to call
/// `link_external_elo` to mark a Lichess account as verified on-chain.
/// Public key: 42fiB5KcC1jEVXxmgPoWqpA3zuKEsZGu77YHmCwNEcrh
/// Private key stored in keys/link_authority.json (gitignored) and, for the
/// backend signer, backend/.env as LINK_AUTHORITY_KEY. Rotate before mainnet.
pub mod link_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array([
        0x2d, 0x00, 0x69, 0x37, 0x6a, 0x4c, 0x05, 0x65, 0x6a, 0xe5, 0x6a, 0x27, 0xf2, 0x5d, 0x41,
        0x10, 0x7a, 0x00, 0x14, 0x49, 0x46, 0x22, 0xac, 0xf4, 0x3b, 0x23, 0x08, 0x8c, 0x88, 0x7c,
        0x3e, 0x06,
    ]);
}

/// The VPS/backend operational authority — the only signer allowed to call
/// privileged instructions such as `update_elo`, `collect_fee`, and
/// tournament creation. Deliberately a dedicated key, separate from the
/// program's upgrade authority, so a compromised backend can't touch the
/// deployed program itself.
/// Matches keys/vps_authority.json (HZTwvN9AUK1n9jmQydrh5vkpdCBZm13W7qD9jtPZJSQc).
/// Must be funded with devnet SOL before it can sign/pay for tournament
/// creation (it's the fee payer for the Tournament/Escrow/Shards PDAs).
pub mod vps_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array([
        0xf6, 0x0c, 0x0b, 0xb0, 0xce, 0xb6, 0xfd, 0x2e, 0xc0, 0x67, 0x87, 0x02, 0x2f, 0x47, 0x5a,
        0x17, 0xd5, 0xce, 0x14, 0x8b, 0x17, 0xe5, 0xda, 0xed, 0x35, 0x15, 0x76, 0x16, 0x0a, 0xa4,
        0x40, 0x05,
    ]);
}

/// The treasury-withdrawal authority — the only signer allowed to call
/// `withdraw_treasury` and drain accumulated platform fees. Deliberately kept
/// separate from `vps_authority` so platform revenue sits behind a dedicated
/// signer without also gating result-signing.
/// Public key: 9jpjASzudVvpbgw5G7zCf7o6EvCw4ejRVcEN1aBLq4Kd
/// A single dedicated devnet/testnet wallet (no multisig). Private key stored in
/// keys/treasury_authority.json (gitignored) / backend .env TREASURY_AUTHORITY_KEY.
/// Rotate before mainnet.
pub mod treasury_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array([
        0x81, 0xd5, 0xda, 0xbb, 0x6e, 0xc6, 0xc1, 0x4e, 0x77, 0x8c, 0xd0, 0x1c, 0x5a, 0x45, 0x2d,
        0xb7, 0xf2, 0xf0, 0x52, 0xc8, 0x66, 0x7b, 0xb8, 0xd3, 0xd8, 0xc2, 0xc6, 0xa4, 0x7d, 0x24,
        0xa4, 0xd4,
    ]);
}

/// Hard cap on a single wager so no one can lock more than 10 SOL in one game.
pub const MAX_WAGER_AMOUNT: u64 = 10 * 1_000_000_000; // 10 SOL in lamports

/// Minimum wager amount (0.001 SOL).
pub const MIN_WAGER_LAMPORTS: u64 = 1_000_000;

/// Lamports advanced by each player to cover on-chain compute costs.
pub const CREATE_GAME_COST: u64 = 5_000; // lamports
pub const JOIN_GAME_COST: u64 = 5_000;
pub const DELEGATE_COST: u64 = 5_000;
pub const UNDELEGATE_COST: u64 = 5_000;
pub const COMMIT_ER_COST: u64 = 5_000; // per ER→L1 commit (0 if MagicBlock sponsors)
pub const RECORD_RESULT_COST: u64 = 5_000;

/// How often (ms) the Ephemeral Rollup commits delegated game state back to the
/// base layer. A fixed cadence — must not be derived from per-game arguments.
pub const ER_COMMIT_FREQUENCY_MS: u32 = 30_000; // 30s
pub const CLAIM_PRIZE_COST: u64 = 5_000;

// ---------------------------------------------------------------------------
// Platform fees
// ---------------------------------------------------------------------------
// All fees are universal (no regional differentiation).
// The backend calculates live lamport amounts from the SOL/GBP rate and passes
// them as instruction parameters — the program stores and enforces them but
// never hardcodes a currency-specific value.

/// Flat infrastructure fee charged when the dispute authority resolves a contested game.
/// This is a fixed cost for the resolution service — not a percentage rake on the pot.
pub const DISPUTE_RESOLUTION_COST_LAMPORTS: u64 = 10_000;

/// Bond the challenger must post when opening a dispute (0.01 SOL). Refunded if
/// the dispute is upheld (challenger ruled the winner) or auto-resolved after the
/// TTL; forfeited to the platform treasury if the dispute is dismissed. Deters
/// a losing player from disputing an active game just to freeze the pot.
pub const DISPUTE_BOND_LAMPORTS: u64 = 10_000_000;

/// ELO update fee per player
pub const ELO_FEE_LAMPORTS: u64 = 5_000; // 0.000005 SOL per ELO update

/// Treasury vault seed
pub const TREASURY_VAULT_SEED: &[u8] = b"treasury_vault";

/// Global persistent session delegation seed
pub const GLOBAL_SESSION_SEED: &[u8] = b"global_session";

/// Time-to-live for an unresolved dispute (7 days in seconds).
/// After this window any party may call claim_stale_dispute for an automatic 50/50 split.
pub const DISPUTE_TTL_SECS: i64 = 604_800;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_authorities_are_not_default_pubkeys() {
        assert_ne!(link_authority::ID, Pubkey::default());
        assert_ne!(dispute_authority::ID, Pubkey::default());
        assert_ne!(kyc_authority::ID, Pubkey::default());
        assert_ne!(vps_authority::ID, Pubkey::default());
        assert_ne!(treasury_authority::ID, Pubkey::default());
    }
}
