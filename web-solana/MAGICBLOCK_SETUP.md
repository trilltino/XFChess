# MagicBlock Integration Setup

## Required Dependencies

Add to `package.json`:

```json
{
  "dependencies": {
    "@magicblock-labs/ephemeral-rollups-sdk": "^0.6.5"
  }
}
```

Then run:
```bash
npm install
```

## Dual Connection Architecture

The `MagicBlockClient` class in `src/lib/magicblock.ts` implements the MagicBlock Dev Skill pattern:

- **Base Layer Connection**: `https://api.devnet.solana.com`
  - Use for: account initialization, delegation
  
- **Ephemeral Rollup Connection**: `https://devnet.magicblock.app/`
  - Use for: operations on delegated accounts, undelegation
  - Always uses `skipPreflight: true` for speed

## Usage Example

```typescript
import { MagicBlockClient } from '../lib/magicblock';

const client = new MagicBlockClient(wallet);

// Check if game is delegated
const isDelegated = await client.isDelegated(gamePda);

// Get appropriate program for operation
const program = await client.getProgramForDelegated(gamePda);

// Execute on delegated account (automatically uses ER)
const txHash = await client.executeOnDelegated(
  'recordMove',
  { gameAccount: gamePda, player: wallet.publicKey },
  ['e2e4']  // move notation
);
```

## Migration from Single Connection

Old pattern (single connection):
```typescript
const program = getAnchorProgram(connection, wallet);
```

New pattern (dual connection):
```typescript
const client = new MagicBlockClient(wallet);
const program = await client.getProgramForDelegated(gamePda);
// or explicitly:
// const program = isDelegated ? client.erProgram : client.baseProgram;
```
