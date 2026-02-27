# Magic Block Ephemeral Rollups Integration Plan

## Overview

This plan details integrating the Magic Block Ephemeral Rollups SDK to route chess game transactions through the Magic Block rollup server for sub-second latency, then committing final state to Solana.

## Architecture Changes

### Current Flow (Direct Solana)
```
Game Client → P2P (Braid) → Co-sign → Solana RPC → Solana Devnet
```

### New Flow (Magic Block ER)
```
Game Client → Magic Block Resolver → ER Validator (sub-second) → 
→ Periodic Commit → Solana Mainnet/Devnet
```

## Integration Components

### 1. Solana Program Updates

**File**: `programs/xfchess-game/Cargo.toml`
```toml
[features]
default = ["move-validation"]
magicblock = ["ephemeral-rollups-sdk", "ephemeral-rollups-sdk/anchor"]

[dependencies]
ephemeral-rollups-sdk = { path = "../../references/ephemeral-rollups-sdk/rust/sdk", optional = true }
```

**File**: `programs/xfchess-game/src/instructions/delegate_game.rs`
- Uncomment and complete the delegation instruction
- Add `handler_undelegate_game` for game finalization

**File**: `programs/xfchess-game/src/lib.rs`
- Enable delegation/undelegation entrypoints

### 2. Game Client Updates

**New File**: `src/multiplayer/magicblock_resolver.rs`
```rust
//! Magic Block Resolver integration for routing transactions to ER validators

use magic_resolver::{Resolver, Configuration};
use solana_sdk::{pubkey::Pubkey, transaction::Transaction};

pub struct MagicBlockResolver {
    resolver: Resolver,
    delegated_accounts: HashMap<Pubkey, DelegationStatus>,
}

impl MagicBlockResolver {
    pub async fn new() -> Result<Self> {
        let config = Configuration {
            chain_rpc_url: "https://api.devnet.solana.com".to_string(),
            ..Default::default()
        };
        let resolver = Resolver::new(config).await?;
        Ok(Self { resolver, ... })
    }
    
    /// Route transaction to appropriate validator (ER or base chain)
    pub async fn send_transaction(&self, tx: Transaction) -> Result<Signature> {
        self.resolver.send_transaction(tx).await
    }
}
```

**File**: `src/multiplayer/rollup_network_bridge.rs`
- Replace direct Solana RPC calls with Magic Block resolver
- Add delegation step before game starts
- Add undelegation on game end

### 3. Web UI Updates

**New File**: `web-solana/src/hooks/useMagicBlock.ts`
```typescript
import { Connection, Transaction } from '@solana/web3.js';
import { delegateGame, undelegateGame } from '../utils/magicblock';

export function useMagicBlock() {
  const delegateToER = async (gameId: string, gamePda: PublicKey) => {
    // Call delegation instruction
    const tx = await delegateGame(program, gamePda, validUntil);
    await sendAndConfirmTransaction(connection, tx, [wallet]);
  };
  
  return { delegateToER };
}
```

**File**: `web-solana/src/pages/GameDetail.tsx`
- Add "Delegate to ER" button before game starts
- Show ER status indicator (delegated/undelegated)

## Deployment Plan

### Phase 1: Program Deployment

1. **Build with Magic Block feature**:
```bash
cd programs/xfchess-game
anchor build --features magicblock
```

2. **Deploy to devnet**:
```bash
anchor deploy --provider.cluster devnet
```

3. **Initialize delegation**:
```bash
# After game creation, delegate the Game PDA to ER
./target/release/xfchess_cli delegate-game --game-id 12345 --valid-until 7200
```

### Phase 2: Client Integration

1. Add `magic-resolver` dependency to `Cargo.toml`
2. Implement resolver connection in game client
3. Update transaction flow to use resolver

### Phase 3: Web UI Integration

1. Add delegation flow to game creation
2. Add ER status indicators
3. Handle delegation errors gracefully

## Key SDK Components Used

| Component | Purpose | Location |
|-----------|---------|----------|
| `ephemeral-rollups-sdk` | CPI for delegation | `programs/xfchess-game` |
| `magic-resolver` | Route transactions to ER | `src/multiplayer/` |
| `@magicblock-labs/ephemeral-rollups-sdk` | Web UI delegation | `web-solana/` |

## Magic Block ER Endpoints

```
Devnet RPC: https://api.devnet.solana.com
ER Resolver: wss://devnet.magicblock.app (example - verify with Magic Block)
```

## Testing Strategy

1. **Local Testing**:
   - Deploy program with magicblock feature
   - Create game and delegate
   - Verify sub-second move confirmation
   - Verify periodic commits to Solana

2. **Integration Testing**:
   - Test resolver routing
   - Test delegation/undelegation flow
   - Test game finalization with state commit

## Rollback Plan

If Magic Block ER has issues:
1. Disable `magicblock` feature in Cargo.toml
2. Revert to direct Solana submission
3. Program still works without ER (fallback mode)

## Cost Analysis

| Operation | Without ER | With ER |
|-----------|------------|---------|
| Move | ~0.005 SOL | ~0.0001 SOL (ER) + 0.005 SOL (commit) |
| Delegation | N/A | ~0.01 SOL (one-time) |
| Undelegation | N/A | ~0.005 SOL |

**Benefit**: Sub-second latency during gameplay, batch commits to Solana.
