# Signing Service

Transaction signing and Solana blockchain integration for the XFChess backend.

## Overview

The signing service is responsible for:
- Building Solana transactions for tournament operations
- Signing transactions with user wallets
- Submitting transactions to the Solana network
- Managing tournament state in memory
- Providing Blinks API endpoints for Solana actions

## Architecture

The signing service follows a layered architecture:
- **Routes Layer** - HTTP endpoints that handle incoming requests
- **Service Layer** - Business logic for transaction building and signing
- **Integration Layer** - Direct interaction with Solana RPC and wallet APIs

## Components

### Blinks Module (`blinks/`)

Solana Blinks is a protocol that allows Solana actions to be integrated into web applications, social media, and other platforms. This module provides the API endpoints for Blinks actions.

- **blinks.rs** - Core Blinks API with action metadata, transaction builder, balance checking, validation
- **blinks_pda.rs** - PDA (Program Derived Address) calculation for tournament accounts
- **blinks_anti_cheat.rs** - IP pattern detection and rate limiting to prevent abuse
- **blinks_chains.rs** - Action chaining for multi-step flows (wallet creation → funding → registration)
- **blinks_onboarding.rs** - Smart onboarding state machine for user session tracking
- **blinks_funding.rs** - Funding integration helpers for MoonPay/Transak/Banxa

### Country-Specific Compliance (`cacf/`)

Compliance modules for different jurisdictions:
- UK - UK-specific gambling regulations
- Brazil - Brazilian gaming compliance
- Germany - German gambling regulations
- Canada - Canadian gaming compliance

### P2P Relay (`p2p_relay/`)

P2P relay service for multiplayer game synchronization. This module handles:
- Peer discovery and connection
- Game state synchronization
- Move relay between players

### Solana Integration (`solana/`)

Direct integration with the Solana blockchain:
- Transaction building and serialization
- RPC client for blockchain queries
- Account data fetching
- Transaction submission

### Routes (`routes/`)

HTTP route handlers for all signing service endpoints.

### Tournament Store (`tournament_store.rs`)

In-memory tournament state management. This module:
- Stores active tournament data
- Tracks player registrations
- Manages match assignments
- Provides tournament queries

### Module Exports (`mod.rs`)

Module exports and initialization, registering all routes and services with the Axum app.

## API Endpoints

### Blinks Endpoints

- `GET /api/actions/tournament/:id` - Returns action metadata for a tournament
- `POST /api/actions/tournament/:id/register` - Builds a registration transaction
- `GET /api/actions/tournament/:id/check-balance` - Checks user's SOL balance
- `POST /api/actions/tournament/:id/validate` - Anti-cheat validation
- `GET /api/actions/tournament/:id/chain/registration` - Registration action chain
- `GET /api/actions/tournament/:id/chain/onboarding` - Onboarding action chain

### Tournament Endpoints

- `GET /tournaments` - Lists all active tournaments
- `GET /tournament/:id` - Gets tournament details
- `POST /tournament/:id/join` - Joins a tournament
- `GET /tournament/:id/my-match` - Gets current match for a player
- `GET /tournament/:id/bracket` - Gets tournament bracket
- `POST /tournament/:id/register-node` - Registers a VPS node

### Signing Endpoints

- `POST /signing/build` - Builds a Solana transaction
- `POST /signing/sign` - Signs a transaction
- `POST /signing/submit` - Submits a transaction to Solana

## Example: Building a Solana Transaction

This example shows how to build a Solana transaction for tournament registration using the signing service.

```rust
use backend::signing::solana::instructions;
use solana_sdk::{
    transaction::Transaction,
    pubkey::Pubkey,
    instruction::{Instruction, AccountMeta},
};
use solana_sdk::system_program;

/// Builds a Solana transaction for tournament registration
/// 
/// # Arguments
/// * `program_id` - The XFChess program ID on Solana
/// * `signer` - The public key of the signer (player)
/// * `tournament_id` - The ID of the tournament to join
/// * `entry_fee` - The entry fee in lamports
/// 
/// # Returns
/// A signed Transaction ready for submission
/// 
/// # Errors
/// Returns an error if transaction building fails
async fn build_registration_transaction(
    program_id: Pubkey,
    signer: Pubkey,
    tournament_id: u64,
    entry_fee: u64,
) -> Result<Transaction, Box<dyn std::error::Error>> {
    // Calculate the tournament PDA
    let (tournament_pda, _) = Pubkey::find_program_address(
        &[b"tournament", &tournament_id.to_le_bytes()],
        &program_id,
    );
    
    // Calculate the player entry PDA
    let (player_entry_pda, _) = Pubkey::find_program_address(
        &[b"player_entry", tournament_pda.as_ref(), signer.as_ref()],
        &program_id,
    );
    
    // Build the instruction data
    let instruction_data = serialize_register_instruction(tournament_id)?;
    
    // Create the instruction with required accounts
    let instruction = Instruction::new_with_bytes(
        program_id,
        &instruction_data,
        vec![
            AccountMeta::new(signer, true),           // Signer account
            AccountMeta::new(tournament_pda, false),   // Tournament account
            AccountMeta::new(player_entry_pda, false), // Player entry account
            AccountMeta::new_readonly(system_program::id(), false), // System program
        ],
    );
    
    // Create the transaction
    let transaction = Transaction::new(
        &[instruction],
        Some(&signer),
    );
    
    Ok(transaction)
}

/// Serializes the instruction data for tournament registration
/// 
/// # Arguments
/// * `tournament_id` - The tournament ID
/// 
/// # Returns
/// Serialized instruction data as bytes
fn serialize_register_instruction(tournament_id: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut data = Vec::new();
    
    // Instruction discriminator (0 for register)
    data.extend_from_slice(&0u8.to_le_bytes());
    
    // Tournament ID
    data.extend_from_slice(&tournament_id.to_le_bytes());
    
    Ok(data)
}
```

## Example: Blinks Action Metadata

Blinks actions require metadata that describes the action to users. This metadata is used by wallet adapters and other clients to display the action appropriately.

```rust
use backend::signing::blinks::ActionMetadata;

/// Action metadata structure for Blinks actions
#[derive(Clone, Debug, Serialize)]
pub struct ActionMetadata {
    /// The title of the action displayed to users
    pub title: String,
    
    /// A detailed description of what the action does
    pub description: String,
    
    /// The label shown on the action button
    pub label: String,
    
    /// URL to an icon for the action
    pub icon: String,
    
    /// Whether the action is disabled
    pub disabled: bool,
}

/// Generates action metadata for tournament registration
/// 
/// # Arguments
/// * `tournament_id` - The tournament ID
/// * `tournament_name` - The name of the tournament
/// * `entry_fee` - The entry fee in SOL
/// 
/// # Returns
/// ActionMetadata struct with tournament information
pub fn get_tournament_action_metadata(
    tournament_id: u64,
    tournament_name: &str,
    entry_fee: f64,
) -> ActionMetadata {
    ActionMetadata {
        title: "Join Tournament".to_string(),
        description: format!(
            "Join {} tournament with {} SOL entry fee",
            tournament_name, entry_fee
        ),
        label: "Join".to_string(),
        icon: "https://example.com/tournament-icon.png".to_string(),
        disabled: false,
    }
}
```

## Example: Tournament Registration Flow

This example shows the complete flow of registering a player for a tournament:

```rust
use backend::signing::tournament_store::TournamentStore;
use backend::signing::solana::rpc_client::RpcClient;

/// Registers a player for a tournament
/// 
/// This function:
/// 1. Validates the tournament exists and is open
/// 2. Checks the player has sufficient balance
/// 3. Builds the registration transaction
/// 4. Signs the transaction
/// 5. Submits to Solana
/// 6. Updates the tournament store
/// 
/// # Arguments
/// * `store` - The tournament store
/// * `rpc_client` - Solana RPC client
/// * `tournament_id` - The tournament ID
/// * `player_pubkey` - The player's public key
/// * `wallet_signer` - Function to sign transactions
/// 
/// # Returns
/// The transaction signature if successful
/// 
/// # Errors
/// Returns an error if any step fails
async fn register_player_for_tournament<F>(
    store: &TournamentStore,
    rpc_client: &RpcClient,
    tournament_id: u64,
    player_pubkey: Pubkey,
    wallet_signer: F,
) -> Result<String, Box<dyn std::error::Error>>
where
    F: Fn(Transaction) -> Result<Transaction, Box<dyn std::error::Error>>,
{
    // Step 1: Validate tournament exists and is open
    let tournament = store.get_tournament(tournament_id)
        .await?
        .ok_or("Tournament not found")?;
    
    if tournament.status != TournamentStatus::Open {
        return Err("Tournament is not open for registration".into());
    }
    
    // Step 2: Check player balance
    let balance = rpc_client.get_balance(&player_pubkey).await?;
    if balance < tournament.entry_fee {
        return Err("Insufficient balance to join tournament".into());
    }
    
    // Step 3: Build registration transaction
    let transaction = build_registration_transaction(
        PROGRAM_ID,
        player_pubkey,
        tournament_id,
        tournament.entry_fee,
    ).await?;
    
    // Step 4: Sign transaction with wallet
    let signed_transaction = wallet_signer(transaction)?;
    
    // Step 5: Submit to Solana
    let signature = rpc_client.send_transaction(&signed_transaction).await?;
    
    // Step 6: Update tournament store
    store.add_player(tournament_id, player_pubkey).await?;
    
    Ok(signature)
}
```

## Example: Anti-Cheat Validation

The anti-cheat system uses IP pattern detection and rate limiting to prevent abuse:

```rust
use backend::signing::blinks_anti_cheat::{IpDetector, RateLimiter};

/// Validates a registration request for anti-cheat
/// 
/// # Arguments
/// * `ip_detector` - Global IP pattern detector
/// * `rate_limiter` - Rate limiter instance
/// * `client_ip` - The client's IP address
/// * `tournament_id` - The tournament ID
/// 
/// # Returns
/// Ok(()) if validation passes, Err otherwise
pub fn validate_registration(
    ip_detector: &IpDetector,
    rate_limiter: &RateLimiter,
    client_ip: &str,
    tournament_id: u64,
) -> Result<(), AntiCheatError> {
    // Check for suspicious IP patterns (VPNs, proxies, etc.)
    if ip_detector.is_suspicious_ip(client_ip) {
        return Err(AntiCheatError::SuspiciousIp);
    }
    
    // Check rate limit (max 3 registrations per IP)
    if rate_limiter.check_rate_limit(client_ip, 3) {
        return Err(AntiCheatError::RateLimitExceeded);
    }
    
    // Check tournament-specific rate limit (max 2 per 5 minutes)
    if rate_limiter.check_tournament_rate_limit(tournament_id, client_ip, 2, 300) {
        return Err(AntiCheatError::TournamentRateLimitExceeded);
    }
    
    Ok(())
}
```
