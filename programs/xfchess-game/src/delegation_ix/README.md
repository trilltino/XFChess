# Delegation Instructions

Solana program instructions for delegation to VPS (Virtual Private Server) and ephemeral rollups.

## Overview

The delegation instructions module enables off-chain move execution through session key delegation to a VPS. This allows players to delegate move signing authority to a trusted VPS, enabling faster gameplay while maintaining on-chain game state integrity.

## Why Delegation?

On-chain move execution can be slow due to Solana block times and network congestion. Delegation allows:
- **Faster gameplay** - Moves are executed off-chain and batched
- **Lower costs** - Fewer on-chain transactions
- **Better UX** - Near-instant move confirmation
- **Security** - Session keys have limited scope and expiry

## Delegation Flow

1. **Session Creation** - Player creates a session with a session key and expiry
2. **VPS Delegation** - Player delegates move authority to a VPS
3. **Off-Chain Moves** - VPS signs moves using the session key
4. **Batch Submission** - Moves are periodically submitted on-chain
5. **Undelegation** - Player can revoke delegation at any time

## Components

- Session key delegation with time-based expiry
- VPS relay authorization for move signing
- Move delegation to off-chain execution
- Session revocation and cleanup

## Session Account Structure

The session account stores delegation state for a player:

```rust
use anchor_lang::prelude::*;

/// Session account for delegation
#[account]
pub struct Session {
    /// The wallet that owns this session
    pub wallet: Pubkey,
    
    /// The session key authorized to sign moves
    pub session_key: Pubkey,
    
    /// Unix timestamp when session was created
    pub created_at: i64,
    
    /// Unix timestamp when session expires
    pub expires_at: i64,
    
    /// Whether the session is currently active
    pub is_active: bool,
    
    /// Whether delegation to VPS is enabled
    pub vps_delegated: bool,
    
    /// VPS endpoint URL
    pub vps_endpoint: String,
    
    /// Unix timestamp when delegation was granted
    pub delegated_at: Option<i64>,
    
    /// Bump seed for PDA derivation
    pub bump: u8,
}
```

## Example: Creating a Session Key

This instruction creates a new session for delegation with a session key and expiry time.

```rust
use anchor_lang::prelude::*;

/// Context for creating a session
#[derive(Accounts)]
pub struct CreateSession<'info> {
    /// The wallet creating the session
    #[account(mut)]
    pub wallet: Signer<'info>,
    
    /// The session account being created
    #[account(
        init,
        payer = wallet,
        seeds = [b"session", wallet.key().as_ref()],
        bump,
        space = 8 + std::mem::size_of::<Session>()
    )]
    pub session: Account<'info, Session>,
    
    pub system_program: Program<'info, System>,
}

/// Creates a new session for delegation
/// 
/// This instruction initializes a session with a session key that can
/// be used to sign moves on behalf of the wallet. The session has an
/// expiry time after which it becomes invalid.
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// * `session_key` - The public key to authorize for signing moves
/// * `expiry` - Unix timestamp when the session expires
/// 
/// # Errors
/// - Can fail if the wallet cannot pay the rent exemption
/// - `InvalidExpiry` - Expiry time is in the past
pub fn create_session(
    ctx: Context<CreateSession>,
    session_key: Pubkey,
    expiry: i64,
) -> Result<()> {
    let session = &mut ctx.accounts.session;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Validate expiry is in the future
    require!(
        expiry > current_time,
        ErrorCode::InvalidExpiry
    );
    
    // Set session properties
    session.wallet = ctx.accounts.wallet.key();
    session.session_key = session_key;
    session.created_at = current_time;
    session.expires_at = expiry;
    session.is_active = true;
    session.vps_delegated = false;
    session.vps_endpoint = String::new();
    session.delegated_at = None;
    session.bump = ctx.bumps.session;
    
    msg!("Session created for {} with expiry {}", session.wallet, expiry);
    Ok(())
}
```

## Example: Delegating to VPS

This instruction authorizes a VPS to sign moves using the session key.

```rust
/// Context for delegating to a VPS
#[derive(Accounts)]
pub struct DelegateToVps<'info> {
    #[account(mut)]
    pub session: Account<'info, Session>,
    
    #[account(mut)]
    pub game: Account<'info, Game>,
    
    pub wallet: Signer<'info>,
    
    /// CHECK: VPS program for CPI
    pub vps_program: Program<'info, VpsProgram>,
    
    pub clock: Sysvar<'info, Clock>,
}

/// Delegates move authority to a VPS
/// 
/// This instruction:
/// 1. Validates the session is active and not expired
/// 2. Sets the VPS delegation flag
/// 3. Stores the VPS endpoint
/// 4. Authorizes the VPS to sign moves via CPI
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// * `vps_endpoint` - The URL of the VPS endpoint
/// 
/// # Errors
/// - `SessionExpired` - Session has expired
/// - `SessionInactive` - Session is not active
/// - `Unauthorized` - Signer is not the session owner
pub fn delegate_to_vps(
    ctx: Context<DelegateToVps>,
    vps_endpoint: String,
) -> Result<()> {
    let session = &mut ctx.accounts.session;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Validate ownership
    require!(
        ctx.accounts.wallet.key() == session.wallet,
        ErrorCode::Unauthorized
    );
    
    // Validate session is active
    require!(
        session.is_active,
        ErrorCode::SessionInactive
    );
    
    // Validate session has not expired
    require!(
        session.expires_at > current_time,
        ErrorCode::SessionExpired
    );
    
    // Set VPS delegation
    session.vps_delegated = true;
    session.vps_endpoint = vps_endpoint.clone();
    session.delegated_at = Some(current_time);
    
    // Authorize VPS to sign moves via CPI
    let cpi_accounts = vps::cpi::accounts::AuthorizeVps {
        session: ctx.accounts.session.to_account_info(),
        game: ctx.accounts.game.to_account_info(),
        authority: ctx.accounts.wallet.to_account_info(),
    };
    
    vps::cpi::authorize_vps(
        CpiContext::new(
            ctx.accounts.vps_program.to_account_info(),
            cpi_accounts,
        ),
    )?;
    
    msg!("Delegated to VPS: {}", vps_endpoint);
    Ok(())
}
```

## Example: Revoking Delegation

This instruction revokes VPS delegation, requiring all future moves to be signed on-chain.

```rust
/// Context for revoking delegation
#[derive(Accounts)]
pub struct RevokeDelegation<'info> {
    #[account(mut)]
    pub session: Account<'info, Session>,
    
    #[account(mut)]
    pub game: Account<'info, Game>,
    
    pub wallet: Signer<'info>,
    
    /// CHECK: VPS program for CPI
    pub vps_program: Program<'info, VpsProgram>,
}

/// Revokes VPS delegation
/// 
/// This instruction:
/// 1. Validates the wallet owns the session
/// 2. Revokes VPS delegation
/// 3. Clears the VPS endpoint
/// 4. Updates the game to require on-chain moves
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// 
/// # Errors
/// - `Unauthorized` - Signer is not the session owner
/// - `NotDelegated` - Session is not currently delegated
pub fn revoke_delegation(ctx: Context<RevokeDelegation>) -> Result<()> {
    let session = &mut ctx.accounts.session;
    
    // Validate ownership
    require!(
        ctx.accounts.wallet.key() == session.wallet,
        ErrorCode::Unauthorized
    );
    
    // Validate delegation exists
    require!(
        session.vps_delegated,
        ErrorCode::NotDelegated
    );
    
    // Revoke VPS delegation
    session.vps_delegated = false;
    session.vps_endpoint = String::new();
    session.delegated_at = None;
    
    // Revoke VPS authorization via CPI
    let cpi_accounts = vps::cpi::accounts::RevokeVps {
        session: ctx.accounts.session.to_account_info(),
        game: ctx.accounts.game.to_account_info(),
        authority: ctx.accounts.wallet.to_account_info(),
    };
    
    vps::cpi::revoke_vps(
        CpiContext::new(
            ctx.accounts.vps_program.to_account_info(),
            cpi_accounts,
        ),
    )?;
    
    // Update game to require on-chain moves
    ctx.accounts.game.requires_delegation = false;
    
    msg!("VPS delegation revoked");
    Ok(())
}
```

## Example: Session Expiry Check

This example shows how to check if a session has expired.

```rust
/// Checks if a session has expired
/// 
/// # Arguments
/// * `session` - The session to check
/// * `current_time` - Current Unix timestamp
/// 
/// # Returns
/// true if the session has expired, false otherwise
pub fn is_session_expired(session: &Session, current_time: i64) -> bool {
    !session.is_active || session.expires_at <= current_time
}

/// Validates a session for move signing
/// 
/// # Arguments
/// * `session` - The session to validate
/// * `signer` - The public key attempting to sign
/// * `current_time` - Current Unix timestamp
/// 
/// # Returns
/// Ok(()) if the session is valid, Err otherwise
pub fn validate_session_for_signing(
    session: &Session,
    signer: Pubkey,
    current_time: i64,
) -> Result<()> {
    // Check if session is active
    require!(session.is_active, ErrorCode::SessionInactive);
    
    // Check if session has expired
    require!(
        session.expires_at > current_time,
        ErrorCode::SessionExpired
    );
    
    // Check if signer is the session key
    require!(
        signer == session.session_key,
        ErrorCode::InvalidSessionKey
    );
    
    // Check if VPS delegation is active
    require!(session.vps_delegated, ErrorCode::NotDelegated);
    
    Ok(())
}
```
