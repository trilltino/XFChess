# Solana Blinks Testing Guide

## Prerequisites

- Backend server running on localhost:8090
- Solana wallet (Phantom, Solflare, or other)
- Solana Blinks Inspector (https://inspector.solana.com/)

## Testing the Blinks API

### 1. Start the Backend Server

```bash
cd backend
cargo run --bin signing-server-http
```

The server will start on `http://localhost:8090`

### 2. Create a Test Tournament

First, create a test tournament in the backend via the admin API or directly in the TournamentStore:

```bash
curl -X POST http://localhost:8090/api/admin/tournament \
  -H "Content-Type: application/json" \
  -d '{
    "id": 1,
    "name": "Test Tournament",
    "entry_fee_lamports": 500000000,
    "max_players": 8,
    "elo_min": 1000,
    "elo_max": 2000,
    "status": "open"
  }'
```

### 3. Test Action Metadata Endpoint

Use Solana Blinks Inspector to test the GET endpoint:

```
http://localhost:8090/api/actions/tournament/1
```

Expected response:
```json
{
  "version": "1.0.0",
  "title": "Register for Test Tournament",
  "description": "Register for the tournament and compete for prizes",
  "icon": "https://xfchess.com/icon.png",
  "label": "Register",
  "links": {
    "actions": {
      "default": {
        "href": "/api/actions/tournament/1/register",
        "label": "Register"
      }
    }
  }
}
```

### 4. Test Registration Transaction Endpoint

Use Solana Blinks Inspector to test the POST endpoint:

```
http://localhost:8090/api/actions/tournament/1/register
```

Request body:
```json
{
  "account": "YOUR_WALLET_PUBKEY"
}
```

Expected response:
```json
{
  "transaction": "BASE64_ENCODED_TRANSACTION",
  "fee_estimate": 15000
}
```

### 5. Test Balance Check Endpoint

```
http://localhost:8090/api/actions/tournament/1/check-balance?account=YOUR_WALLET_PUBKEY
```

Expected response:
```json
{
  "wallet": "YOUR_WALLET_PUBKEY",
  "balance_lamports": 1000000000,
  "sufficient": true,
  "required_lamports": 500000000
}
```

### 6. Test Validation Endpoint

```
http://localhost:8090/api/actions/tournament/1/validate
```

Request body:
```json
{
  "account": "YOUR_WALLET_PUBKEY"
}
```

Expected response:
```json
{
  "valid": true,
  "error": null,
  "next_action": null
}
```

### 7. Test Action Chains

#### Registration Chain (for users with wallet and SOL)

```
http://localhost:8090/api/actions/tournament/1/chain/registration?wallet=YOUR_WALLET_PUBKEY
```

Expected response:
```json
{
  "chain_id": "registration-1",
  "name": "Tournament Registration",
  "description": "Register for the tournament and view your match",
  "steps": [
    {
      "step": 1,
      "label": "Validate Registration",
      "action": {
        "type": "validation",
        "data": {
          "url": "/api/actions/tournament/1/validate"
        }
      },
      "completed": false
    },
    {
      "step": 2,
      "label": "Register for Tournament",
      "action": {
        "type": "registration",
        "data": {
          "url": "/api/actions/tournament/1/register"
        }
      },
      "completed": false
    },
    {
      "step": 3,
      "label": "View Your Match",
      "action": {
        "type": "view_match",
        "data": {
          "url": "/tournament/1/my-match?player=YOUR_WALLET_PUBKEY"
        }
      },
      "completed": false
    }
  ],
  "current_step": 0
}
```

#### Onboarding Chain (for users without wallet or SOL)

```
http://localhost:8090/api/actions/tournament/1/chain/onboarding
```

Expected response:
```json
{
  "chain_id": "onboarding-1",
  "name": "Tournament Onboarding",
  "description": "Create wallet, fund with SOL, and register for tournament",
  "steps": [
    {
      "step": 1,
      "label": "Create Wallet",
      "action": {
        "type": "wallet_creation",
        "data": {
          "deep_link": "https://phantom.app/ul/browse/https://xfchess.com"
        }
      },
      "completed": false
    },
    {
      "step": 2,
      "label": "Fund Wallet",
      "action": {
        "type": "funding",
        "data": {
          "url": "https://xfchess.com/fund?amount=0.5",
          "amount_sol": 0.5
        }
      },
      "completed": false
    },
    {
      "step": 3,
      "label": "Validate Registration",
      "action": {
        "type": "validation",
        "data": {
          "url": "/api/actions/tournament/1/validate"
        }
      },
      "completed": false
    },
    {
      "step": 4,
      "label": "Register for Tournament",
      "action": {
        "type": "registration",
        "data": {
          "url": "/api/actions/tournament/1/register"
        }
      },
      "completed": false
    },
    {
      "step": 5,
      "label": "View Your Match",
      "action": {
        "type": "view_match",
        "data": {
          "url": "/tournament/1/my-match"
        }
      },
      "completed": false
    }
  ],
  "current_step": 0
}
```

## Testing with Solana Blinks Inspector

1. Open https://inspector.solana.com/
2. Enter your action URL: `http://localhost:8090/api/actions/tournament/1`
3. Click "Test"
4. Verify the metadata loads correctly
5. Click "Execute Action" to test the registration flow
6. Verify the transaction is built correctly
7. Sign and submit the transaction with your wallet

## Testing Anti-Cheat

### IP Rate Limiting

Make multiple registration attempts from the same IP:

```bash
for i in {1..4}; do
  curl -X POST http://localhost:8090/api/actions/tournament/1/validate \
    -H "Content-Type: application/json" \
    -d '{"account": "test_wallet"}'
done
```

Expected: First 2 requests succeed, 3rd+ fail with rate limit error.

### ELO Validation

Test with different ELO values:

```bash
# Below minimum (should fail)
curl -X POST http://localhost:8090/api/actions/tournament/1/validate \
  -H "Content-Type: application/json" \
  -d '{"account": "low_elo_wallet"}'

# Within range (should succeed)
curl -X POST http://localhost:8090/api/actions/tournament/1/validate \
  -H "Content-Type: application/json" \
  -d '{"account": "normal_elo_wallet"}'
```

## Testing Funding Flow

1. Open the funding page: `http://localhost:5173/fund?wallet=YOUR_WALLET&amount=0.5`
2. Select a provider (MoonPay, Transak, or Banxa)
3. Click "Buy SOL"
4. Verify the redirect URL is correct
5. Complete the funding flow (requires API keys to be configured)

## Common Issues

### Transaction Fails on Chain

- Verify PDAs match the smart contract exactly
- Check the program ID is correct
- Ensure the fee-payer has sufficient SOL for transaction fees

### Balance Check Returns Insufficient

- Verify the wallet actually has SOL
- Check the required SOL amount calculation
- Ensure the RPC URL is correct

### Validation Fails

- Check ELO range settings
- Verify tournament capacity
- Ensure the wallet isn't already registered

## Next Steps

After successful local testing:
1. Deploy to Hetzner VPS
2. Configure production API keys for MoonPay/Transak/Banxa
3. Test with real wallet on mainnet/devnet
4. Monitor for issues in production logs
