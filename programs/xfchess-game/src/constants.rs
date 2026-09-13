use anchor_lang::prelude::*;

#[constant]
pub const GAME_SEED: &[u8] = b"game";

#[constant]
pub const MOVE_LOG_SEED: &[u8] = b"move_log";

#[constant]
pub const PROFILE_SEED: &[u8] = b"profile";

#[constant]
pub const USERNAME_SEED: &[u8] = b"username";

#[constant]
pub const LICHESS_USERNAME_SEED: &[u8] = b"lichess_username";

#[constant]
pub const FRIENDSHIP_SEED: &[u8] = b"friendship";

#[constant]
pub const WAGER_ESCROW_SEED: &[u8] = b"escrow";

#[constant]
pub const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";

pub const TOURNAMENT_SEED: &[u8] = b"tournament";
pub const TOURNAMENT_PLAYERS_SEED: &[u8] = b"tourney_players";
pub const TOURNAMENT_ESCROW_SEED: &[u8] = b"t_escrow";
pub const TOURNAMENT_MATCH_SEED: &[u8] = b"t_match";
pub const TOURNAMENT_USDC_PRIZE_SEED: &[u8] = b"t_usdc_prize";

// ---------------------------------------------------------------------------
// Authority Constants (environment-gated)
// ---------------------------------------------------------------------------
// Mirrors Raydium's pattern: `declare_id!` and all privileged authority
// constants are split by `localnet` / `devnet` / `mainnet` feature gates.
//
// * `localnet`  — fixed pubkeys for local program-test runs.
// * `devnet`    — the current devnet deployment pubkeys (default feature).
// * `mainnet`   — placeholder all-zeros keys; the const-assertion guard
//   below prevents mainnet builds until keys are rotated.

#[cfg(feature = "localnet")]
mod pda_keys {
    pub const VPS_AUTHORITY: [u8; 32] = [
        0xf6, 0x0c, 0x0b, 0xb0, 0xce, 0xb6, 0xfd, 0x2e, 0xc0, 0x67, 0x87, 0x02, 0x2f, 0x47, 0x5a,
        0x17, 0xd5, 0xce, 0x14, 0x8b, 0x17, 0xe5, 0xda, 0xed, 0x35, 0x15, 0x76, 0x16, 0x0a, 0xa4,
        0x40, 0x05,
    ];
    pub const TREASURY_AUTHORITY: [u8; 32] = [
        0x81, 0xd5, 0xda, 0xbb, 0x6e, 0xc6, 0xc1, 0x4e, 0x77, 0x8c, 0xd0, 0x1c, 0x5a, 0x45, 0x2d,
        0xb7, 0xf2, 0xf0, 0x52, 0xc8, 0x66, 0x7b, 0xb8, 0xd3, 0xd8, 0xc2, 0xc6, 0xa4, 0x7d, 0x24,
        0xa4, 0xd4,
    ];
    pub const DISPUTE_AUTHORITY: [u8; 32] = [
        0xf0, 0x1c, 0x16, 0x70, 0x78, 0x28, 0x62, 0x5a, 0xb2, 0x0b, 0xe0, 0x22, 0x42, 0x43, 0xd1,
        0x7c, 0xd7, 0x70, 0x4d, 0xd2, 0xbb, 0xd6, 0x3f, 0x03, 0x4f, 0xbb, 0x98, 0xd4, 0xca, 0x2f,
        0x3f, 0xd7,
    ];
    pub const LINK_AUTHORITY: [u8; 32] = [
        0x2d, 0x00, 0x69, 0x37, 0x6a, 0x4c, 0x05, 0x65, 0x6a, 0xe5, 0x6a, 0x27, 0xf2, 0x5d, 0x41,
        0x10, 0x7a, 0x00, 0x14, 0x49, 0x46, 0x22, 0xac, 0xf4, 0x3b, 0x23, 0x08, 0x8c, 0x88, 0x7c,
        0x3e, 0x06,
    ];
    pub const KYC_AUTHORITY: [u8; 32] = [
        0x1a, 0x4e, 0x9b, 0x62, 0xc3, 0x6f, 0x3f, 0xda, 0x95, 0x75, 0x85, 0xdd, 0x99, 0xd3, 0x5e,
        0x0d, 0x9f, 0x24, 0x6d, 0x4d, 0x17, 0x54, 0x6c, 0xb5, 0x01, 0x27, 0xaa, 0xbf, 0x15, 0x75,
        0xb3, 0x82,
    ];
}

#[cfg(feature = "devnet")]
mod pda_keys {
    pub const VPS_AUTHORITY: [u8; 32] = [
        0xf6, 0x0c, 0x0b, 0xb0, 0xce, 0xb6, 0xfd, 0x2e, 0xc0, 0x67, 0x87, 0x02, 0x2f, 0x47, 0x5a,
        0x17, 0xd5, 0xce, 0x14, 0x8b, 0x17, 0xe5, 0xda, 0xed, 0x35, 0x15, 0x76, 0x16, 0x0a, 0xa4,
        0x40, 0x05,
    ];
    pub const TREASURY_AUTHORITY: [u8; 32] = [
        0x81, 0xd5, 0xda, 0xbb, 0x6e, 0xc6, 0xc1, 0x4e, 0x77, 0x8c, 0xd0, 0x1c, 0x5a, 0x45, 0x2d,
        0xb7, 0xf2, 0xf0, 0x52, 0xc8, 0x66, 0x7b, 0xb8, 0xd3, 0xd8, 0xc2, 0xc6, 0xa4, 0x7d, 0x24,
        0xa4, 0xd4,
    ];
    pub const DISPUTE_AUTHORITY: [u8; 32] = [
        0xf0, 0x1c, 0x16, 0x70, 0x78, 0x28, 0x62, 0x5a, 0xb2, 0x0b, 0xe0, 0x22, 0x42, 0x43, 0xd1,
        0x7c, 0xd7, 0x70, 0x4d, 0xd2, 0xbb, 0xd6, 0x3f, 0x03, 0x4f, 0xbb, 0x98, 0xd4, 0xca, 0x2f,
        0x3f, 0xd7,
    ];
    pub const LINK_AUTHORITY: [u8; 32] = [
        0x2d, 0x00, 0x69, 0x37, 0x6a, 0x4c, 0x05, 0x65, 0x6a, 0xe5, 0x6a, 0x27, 0xf2, 0x5d, 0x41,
        0x10, 0x7a, 0x00, 0x14, 0x49, 0x46, 0x22, 0xac, 0xf4, 0x3b, 0x23, 0x08, 0x8c, 0x88, 0x7c,
        0x3e, 0x06,
    ];
    pub const KYC_AUTHORITY: [u8; 32] = [
        0x1a, 0x4e, 0x9b, 0x62, 0xc3, 0x6f, 0x3f, 0xda, 0x95, 0x75, 0x85, 0xdd, 0x99, 0xd3, 0x5e,
        0x0d, 0x9f, 0x24, 0x6d, 0x4d, 0x17, 0x54, 0x6c, 0xb5, 0x01, 0x27, 0xaa, 0xbf, 0x15, 0x75,
        0xb3, 0x82,
    ];
}

// mainnet: placeholder all-zeros keys — replaced before deployment.
#[cfg(all(
    feature = "mainnet",
    not(any(feature = "localnet", feature = "devnet"))
))]
mod pda_keys {
    pub const VPS_AUTHORITY: [u8; 32] = [0u8; 32];
    pub const TREASURY_AUTHORITY: [u8; 32] = [0u8; 32];
    pub const DISPUTE_AUTHORITY: [u8; 32] = [0u8; 32];
    pub const LINK_AUTHORITY: [u8; 32] = [0u8; 32];
    pub const KYC_AUTHORITY: [u8; 32] = [0u8; 32];
}

// Fallback: no env feature active → default to devnet keys.
#[cfg(not(any(feature = "localnet", feature = "devnet", feature = "mainnet")))]
mod pda_keys {
    pub const VPS_AUTHORITY: [u8; 32] = [
        0xf6, 0x0c, 0x0b, 0xb0, 0xce, 0xb6, 0xfd, 0x2e, 0xc0, 0x67, 0x87, 0x02, 0x2f, 0x47, 0x5a,
        0x17, 0xd5, 0xce, 0x14, 0x8b, 0x17, 0xe5, 0xda, 0xed, 0x35, 0x15, 0x76, 0x16, 0x0a, 0xa4,
        0x40, 0x05,
    ];
    pub const TREASURY_AUTHORITY: [u8; 32] = [
        0x81, 0xd5, 0xda, 0xbb, 0x6e, 0xc6, 0xc1, 0x4e, 0x77, 0x8c, 0xd0, 0x1c, 0x5a, 0x45, 0x2d,
        0xb7, 0xf2, 0xf0, 0x52, 0xc8, 0x66, 0x7b, 0xb8, 0xd3, 0xd8, 0xc2, 0xc6, 0xa4, 0x7d, 0x24,
        0xa4, 0xd4,
    ];
    pub const DISPUTE_AUTHORITY: [u8; 32] = [
        0xf0, 0x1c, 0x16, 0x70, 0x78, 0x28, 0x62, 0x5a, 0xb2, 0x0b, 0xe0, 0x22, 0x42, 0x43, 0xd1,
        0x7c, 0xd7, 0x70, 0x4d, 0xd2, 0xbb, 0xd6, 0x3f, 0x03, 0x4f, 0xbb, 0x98, 0xd4, 0xca, 0x2f,
        0x3f, 0xd7,
    ];
    pub const LINK_AUTHORITY: [u8; 32] = [
        0x2d, 0x00, 0x69, 0x37, 0x6a, 0x4c, 0x05, 0x65, 0x6a, 0xe5, 0x6a, 0x27, 0xf2, 0x5d, 0x41,
        0x10, 0x7a, 0x00, 0x14, 0x49, 0x46, 0x22, 0xac, 0xf4, 0x3b, 0x23, 0x08, 0x8c, 0x88, 0x7c,
        0x3e, 0x06,
    ];
    pub const KYC_AUTHORITY: [u8; 32] = [
        0x1a, 0x4e, 0x9b, 0x62, 0xc3, 0x6f, 0x3f, 0xda, 0x95, 0x75, 0x85, 0xdd, 0x99, 0xd3, 0x5e,
        0x0d, 0x9f, 0x24, 0x6d, 0x4d, 0x17, 0x54, 0x6c, 0xb5, 0x01, 0x27, 0xaa, 0xbf, 0x15, 0x75,
        0xb3, 0x82,
    ];
}

// Compile-error guard: mainnet builds must have real authority keys.
#[cfg(all(
    feature = "mainnet",
    not(any(feature = "localnet", feature = "devnet"))
))]
const _MAINNET_KEY_GUARD: () = {
    const fn all_zero(arr: &[u8; 32]) -> bool {
        let mut i = 0;
        while i < 32 {
            if arr[i] != 0 {
                return false;
            }
            i += 1;
        }
        true
    }
    const _C1: () = assert!(
        !all_zero(&pda_keys::VPS_AUTHORITY),
        "mainnet: vps_authority must be rotated"
    );
    const _C2: () = assert!(
        !all_zero(&pda_keys::TREASURY_AUTHORITY),
        "mainnet: treasury_authority must be rotated"
    );
    const _C3: () = assert!(
        !all_zero(&pda_keys::DISPUTE_AUTHORITY),
        "mainnet: dispute_authority must be rotated"
    );
    const _C4: () = assert!(
        !all_zero(&pda_keys::LINK_AUTHORITY),
        "mainnet: link_authority must be rotated"
    );
    const _C5: () = assert!(
        !all_zero(&pda_keys::KYC_AUTHORITY),
        "mainnet: kyc_authority must be rotated"
    );
};

pub mod kyc_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array(pda_keys::KYC_AUTHORITY);
}
pub mod dispute_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array(pda_keys::DISPUTE_AUTHORITY);
}
pub mod link_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array(pda_keys::LINK_AUTHORITY);
}
pub mod vps_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array(pda_keys::VPS_AUTHORITY);
}
pub mod treasury_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array(pda_keys::TREASURY_AUTHORITY);
}

pub const MAX_WAGER_AMOUNT: u64 = 10 * 1_000_000_000; // 10 SOL in lamports

pub const MAX_PLATFORM_FEE_LAMPORTS: u64 = MAX_WAGER_AMOUNT / 10; // 1 SOL

pub const MIN_WAGER_LAMPORTS: u64 = 1_000_000;

pub const CREATE_GAME_COST: u64 = 5_000;
pub const JOIN_GAME_COST: u64 = 5_000;
pub const DELEGATE_COST: u64 = 5_000;
pub const UNDELEGATE_COST: u64 = 5_000;
pub const RECORD_RESULT_COST: u64 = 5_000;

pub const ER_SESSION_FEE_LAMPORTS: u64 = 300_000;

pub const ER_COMMIT_FREQUENCY_MS: u32 = 30_000;
pub const CLAIM_PRIZE_COST: u64 = 5_000;

pub const DISPUTE_RESOLUTION_COST_LAMPORTS: u64 = 10_000;

pub const DISPUTE_BOND_LAMPORTS: u64 = 10_000_000;

pub const ELO_FEE_LAMPORTS: u64 = 5_000;

pub const TREASURY_VAULT_SEED: &[u8] = b"treasury_vault";

pub const GLOBAL_SESSION_SEED: &[u8] = b"global_session";

pub const DISPUTE_TTL_SECS: i64 = 604_800;

pub const CRANK_MAX_SLOT_DELAY: u64 = 300;

pub const CRANK_MAX_SECONDS_EARLY: i64 = 60;

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Bounded durations (checked_add-safe)
// ---------------------------------------------------------------------------

/// Maximum session-key validity: 30 days.
pub const MAX_SESSION_DURATION_SECS: i64 = 30 * 24 * 60 * 60;

/// Maximum dispute duration: 7 days (matches DISPUTE_TTL_SECS).
pub const MAX_DISPUTE_DURATION_SECS: i64 = DISPUTE_TTL_SECS;

/// Withdrawal expiry after game creation/finalization: 24 hours.
pub const WITHDRAW_EXPIRY_SECS: i64 = 86_400;

/// Game session (delegation) expiry: 2 hours.
pub const GAME_SESSION_EXPIRY_SECS: i64 = 2 * 60 * 60;

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

    #[test]
    fn production_authorities_are_pairwise_distinct() {
        let authorities = [
            ("kyc_authority", kyc_authority::ID),
            ("dispute_authority", dispute_authority::ID),
            ("link_authority", link_authority::ID),
            ("vps_authority", vps_authority::ID),
            ("treasury_authority", treasury_authority::ID),
        ];
        for i in 0..authorities.len() {
            for j in (i + 1)..authorities.len() {
                let (name_a, key_a) = authorities[i];
                let (name_b, key_b) = authorities[j];
                assert_ne!(
                    key_a, key_b,
                    "{name_a} and {name_b} must be distinct authorities, found the same key"
                );
            }
        }
    }
}
