# MagicBlock ER Testing Guide - On-Chain Evidence Collection

This guide explains how to test each phase of the Ephemeral Rollups (ER) integration and collect on-chain evidence.

## Quick Start

Run the complete test suite:
```batch
test_er_complete.bat
```

Or test individual phases:
```bash
cd web-solana

# Phase 1: Delegation
npx tsx test_er_delegation.ts

# Phase 2: Gameplay (requires game PDA from Phase 1)
npx tsx test_er_gameplay.ts <GAME_PDA>

# Phase 3: Undelegation (requires game PDA)
npx tsx test_er_undelegation.ts <GAME_PDA>

# Verification: Verify all evidence on-chain
npx tsx verify_on_chain.ts
```

---

## Phase 1: Delegation Test

**Purpose**: Verify game PDA delegation to MagicBlock ER

**What it does**:
1. Creates/loads a test wallet
2. Derives a game PDA
3. Sends delegation transaction to ER
4. Captures transaction signature
5. Verifies account state before/after

**Evidence captured**:
- Transaction signature
- Game PDA address
- Pre/post delegation lamports
- ER and Solana explorer links
- Slot number
- Error messages (if any)

**Output file**: `web-solana/evidence/delegation_<timestamp>.json`

**Example evidence**:
```json
{
  "phase": "delegation",
  "timestamp": "2026-02-27T13:30:00.000Z",
  "gamePda": "3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP",
  "payer": "ABC123...",
  "delegationSignature": "5Kx...",
  "erExplorerUrl": "https://explorer.solana.com/tx/5Kx...?cluster=devnet",
  "solanaExplorerUrl": "https://explorer.solana.com/tx/5Kx...?cluster=devnet",
  "preDelegationLamports": 0,
  "postDelegationLamports": 2039280,
  "slot": 123456789
}
```

---

## Phase 2: Gameplay Test

**Purpose**: Test move execution through ER with latency metrics

**What it does**:
1. Executes 6 standard chess moves
2. Routes each move through ER
3. Measures ER confirmation latency
4. Checks Solana eventual consistency
5. Captures all transaction signatures

**Evidence captured**:
- Each move's UCI notation
- Transaction signature per move
- ER confirmation status
- Solana confirmation status
- Latency per move (ms)
- Slot numbers

**Output file**: `web-solana/evidence/gameplay_<timestamp>.json`

**Example evidence**:
```json
{
  "phase": "gameplay",
  "timestamp": "2026-02-27T13:31:00.000Z",
  "gamePda": "3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP",
  "moves": [
    {
      "moveNumber": 1,
      "uci": "e2e4",
      "player": "white",
      "signature": "5Kx...",
      "erConfirmed": true,
      "solanaConfirmed": true,
      "slot": 123456790,
      "latencyMs": 245
    }
  ],
  "totalMoves": 6,
  "successfulMoves": 6,
  "failedMoves": 0,
  "averageLatencyMs": 312
}
```

---

## Phase 3: Undelegation Test

**Purpose**: Verify final state commitment to Solana

**What it does**:
1. Captures pre-undelegation state on ER
2. Sends undelegation transaction
3. Waits for Solana confirmation
4. Captures post-undelegation state
5. Generates state commitment proof

**Evidence captured**:
- Undelegation signature
- Pre-state (lamports, data size, owner, slot)
- Post-state (lamports, data size, owner, slot)
- State commitment proof (hash)
- Explorer links

**Output file**: `web-solana/evidence/undelegation_<timestamp>.json`

**Example evidence**:
```json
{
  "phase": "undelegation",
  "timestamp": "2026-02-27T13:32:00.000Z",
  "gamePda": "3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP",
  "undelegationSignature": "5Kx...",
  "preState": {
    "lamports": 2039280,
    "dataSize": 1024,
    "owner": "3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP",
    "slot": 123456795
  },
  "postState": {
    "lamports": 2039280,
    "dataSize": 1024,
    "owner": "3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP",
    "slot": 123456800
  },
  "stateCommitmentProof": "a1b2c3d4..."
}
```

---

## Verification Phase

**Purpose**: Verify all evidence against the blockchain

**What it does**:
1. Reads all evidence files
2. Queries Solana for each signature
3. Checks confirmation status
4. Verifies account state consistency
5. Generates verification report

**Output file**: `web-solana/evidence/verification_report_<timestamp>.json`

**Example report**:
```json
{
  "generatedAt": "2026-02-27T13:33:00.000Z",
  "rpcEndpoint": "https://api.devnet.solana.com",
  "totalFiles": 3,
  "verified": 3,
  "failed": 0,
  "results": [
    {
      "file": "delegation_1234567890.json",
      "phase": "delegation",
      "verified": true,
      "checks": {
        "signatureValid": true,
        "onChainConfirmed": true,
        "stateConsistent": true
      }
    }
  ]
}
```

---

## Manual Verification Commands

Verify a transaction signature:
```bash
# Using Solana CLI
solana confirm <SIGNATURE> --url devnet

# Get transaction details
solana transaction-history <ADDRESS> --url devnet

# Check account info
solana account <GAME_PDA> --url devnet
```

Verify using curl:
```bash
# Get signature status
curl -X POST https://api.devnet.solana.com \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getSignatureStatuses",
    "params": [["<SIGNATURE>"]]
  }'

# Get account info
curl -X POST https://api.devnet.solana.com \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getAccountInfo",
    "params": ["<GAME_PDA>", {"encoding": "base64"}]
  }'
```

---

## Explorer Links

- **Solana Devnet Explorer**: https://explorer.solana.com/?cluster=devnet
- **Solana FM**: https://solana.fm/?cluster=devnet

View specific transactions:
```
https://explorer.solana.com/tx/<SIGNATURE>?cluster=devnet
```

View specific accounts:
```
https://explorer.solana.com/address/<GAME_PDA>?cluster=devnet
```

---

## Configuration

| Setting | Value |
|---------|-------|
| Solana RPC | `https://api.devnet.solana.com` |
| MagicBlock ER RPC | `https://devnet-eu.magicblock.app` |
| Program ID | `3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP` |
| Delegation Program | `DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh` |

---

## Troubleshooting

### "Test wallet not found"
Run Phase 1 first to create a wallet:
```bash
npx tsx test_er_delegation.ts
```

### "Game account not found on ER"
The game may not be delegated yet. Check:
1. Delegation was successful (check evidence file)
2. Wait a few seconds for ER propagation
3. Verify on Solana explorer

### Low balance error
Get devnet SOL from the faucet:
```bash
solana airdrop 2 <WALLET_ADDRESS> --url devnet
```
Or visit: https://faucet.solana.com/

### Transaction not confirmed on Solana
ER transactions are eventually consistent. They may take:
- 5-30 seconds for commit to Solana
- Up to 2 minutes in rare cases

Check again later or run the verification script.

---

## Evidence Directory Structure

```
web-solana/evidence/
├── delegation_<timestamp>.json
├── gameplay_<timestamp>.json
├── undelegation_<timestamp>.json
├── verification_report_<timestamp>.json
├── delegation_error_<timestamp>.json    (if errors occur)
├── gameplay_error_<timestamp>.json       (if errors occur)
└── undelegation_error_<timestamp>.json   (if errors occur)
```

---

## What Each Test Proves

| Phase | On-Chain Evidence | What It Proves |
|-------|------------------|----------------|
| **Delegation** | Delegation signature on ER | Game PDA was successfully delegated to ER |
| **Gameplay** | Move signatures on ER + Solana | Moves were processed through ER with low latency |
| **Undelegation** | Undelegation signature + state proof | Final state was committed to Solana |
| **Verification** | Confirmation statuses | All transactions are confirmed on-chain |

---

## Running in CI/CD

For automated testing, modify the batch file to:
1. Remove `pause` commands
2. Add exit codes
3. Export evidence as artifacts

Example GitHub Actions:
```yaml
- name: Run ER Tests
  run: test_er_complete.bat
  
- name: Upload Evidence
  uses: actions/upload-artifact@v3
  with:
    name: on-chain-evidence
    path: web-solana/evidence/*.json
```

---

## Summary

You now have a complete testing framework that:
- ✅ Tests all 3 ER phases (delegation, gameplay, undelegation)
- ✅ Captures transaction signatures as on-chain evidence
- ✅ Measures ER latency vs Solana
- ✅ Verifies all evidence on-chain
- ✅ Generates JSON reports for audit/proof

Run `test_er_complete.bat` to execute the full test suite and collect comprehensive on-chain evidence.
