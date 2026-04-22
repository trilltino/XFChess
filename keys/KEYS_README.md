# XFChess Keypairs

This directory is **fully gitignored** (except this README).
All `.json` files here are Solana keypairs — never commit them.

## Files

| File | Public Address | Purpose | Network |
|------|---------------|---------|---------|
| `program-authority.json` | `C1vn2MT7tZotZPjUJQDf9oo3dpZZ2tr7NxYLg8jTYgkw` | Program upgrade authority — signs all `anchor deploy` | devnet |
| `fee-payer.json` | `9dT8q8ZaP3XLDx4ecgk2Yptn4F7YTRwMv33H5AuzpKSG` | Fee payer / project wallet | devnet |
| `vps-authority.json` | *(copy from VPS: `/opt/xfchess/keys/vps-authority.json`)* | VPS backend signing key | devnet |
| `kyc-authority.json` | *(copy from VPS: `/opt/xfchess/keys/kyc-authority.json`)* | KYC oracle signer | devnet |

## Deploy command

```powershell
# Point Solana CLI at the upgrade authority, then deploy
solana config set --keypair keys\program-authority.json --url devnet
anchor deploy --provider.cluster devnet
```

## Funding the upgrade authority

The upgrade authority needs ~7 SOL on devnet to deploy the 927 KB program:

```powershell
# Devnet faucet (free, safe)
solana airdrop 5 C1vn2MT7tZotZPjUJQDf9oo3dpZZ2tr7NxYLg8jTYgkw --url devnet

# OR transfer from fee-payer
solana transfer C1vn2MT7tZotZPjUJQDf9oo3dpZZ2tr7NxYLg8jTYgkw 7 \
  --keypair keys\fee-payer.json --url devnet
```

## Recovering missing keys from VPS

```bash
# SSH into VPS and print the key
ssh root@178.104.55.19 "cat /opt/xfchess/keys/vps-authority.json"
# Paste the output into keys/vps-authority.json locally
```

## Backup reminder

Copy this entire `keys/` directory to a **password-protected location** outside
the repo (e.g. an encrypted USB, 1Password secure note, or Bitwarden attachment).
The `.json` files are the raw private keys — anyone with them controls the funds.
