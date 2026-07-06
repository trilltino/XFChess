//! Account structure mapping unique usernames back to player profiles.

use anchor_lang::prelude::*;

/// UsernameRecord PDA ensures username uniqueness across all players
/// Seeds: [b"username", username.as_bytes()]
#[account]
pub struct UsernameRecord {
    pub owner: Pubkey,   // Player who owns this username
    pub created_at: i64, // Timestamp when username was claimed
}

impl UsernameRecord {
    pub const LEN: usize = 8 + 32 + 8; // Discriminator + Pubkey + i64
}

/// Validates username format according to rules:
/// - Length: 3-20 characters
/// - Characters: A-Z, a-z, 0-9, underscore, hyphen
/// - Not reserved (admin, system, support, official, etc.)
pub fn validate_username(username: &str) -> Result<()> {
    // Check length
    let len = username.len();
    require!(len >= 3 && len <= 20, UsernameError::InvalidLength);

    // Check valid characters
    for ch in username.chars() {
        let valid = ch.is_ascii_alphanumeric() || ch == '_' || ch == '-';
        require!(valid, UsernameError::InvalidCharacters);
    }

    // Check reserved names (case-insensitive)
    let lower = username.to_lowercase();
    let reserved = [
        "admin",
        "system",
        "support",
        "official",
        "moderator",
        "xf",
        "xfchess",
        "chess",
        "test",
        "dev",
        "null",
    ];
    for r in reserved {
        if lower == r || lower.starts_with(r) {
            return Err(UsernameError::ReservedUsername.into());
        }
    }

    Ok(())
}

#[error_code]
pub enum UsernameError {
    #[msg("Username must be 3-20 characters")]
    InvalidLength,
    #[msg("Username can only contain A-Z, a-z, 0-9, _, -")]
    InvalidCharacters,
    #[msg("This username is reserved")]
    ReservedUsername,
    #[msg("Username already taken")]
    UsernameTaken,
    #[msg("Username not set")]
    UsernameNotSet,
    #[msg("Cannot change username yet - cooldown active")]
    ChangeCooldown,
}
