use anchor_lang::prelude::*;

#[account]
pub struct UsernameRecord {
    pub owner: Pubkey,
    pub created_at: i64,
}

impl UsernameRecord {
    pub const LEN: usize = 8 + 32 + 8;
}

pub fn validate_username(username: &str) -> Result<()> {
    let len = username.len();
    require!(
        len >= 3 && len <= 20,
        crate::errors::GameErrorCode::InvalidLength
    );

    for ch in username.chars() {
        let valid = ch.is_ascii_alphanumeric() || ch == '_' || ch == '-';
        require!(valid, crate::errors::GameErrorCode::InvalidCharacters);
    }

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
            return Err(crate::errors::GameErrorCode::ReservedUsername.into());
        }
    }

    Ok(())
}
