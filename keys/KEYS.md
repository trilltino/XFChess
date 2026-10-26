# XFChess Keypairs

This directory is gitignored (`keys/` in `.gitignore`), but this specific
file — `KEYS.md` — is tracked in git despite that (predates the
ignore rule; `git add` needs `-f` to touch it again). Every other file here
is untracked and never leaves your machine.
All `.json` files here are Solana keypairs — never commit them.

## Live authority keys (pubkeys match `programs/xfchess-game/src/constants.rs`)

| File | Public Address | Purpose | Network |
|------|---------------|---------|---------|
| `program-authority.json` | `C1vn2MT7tZotZPjUJQDf9oo3dpZZ2tr7NxYLg8jTYgkw` | Program upgrade authority — signs all `anchor deploy` | devnet |
| `fee-payer.json` | `2oPTbQZDtEKrojoeSsHwhz3wv2sgJYYWh2KDpVYhNtxj` | Fee payer / project wallet, admin keypair default for several `src/bin` test tools | devnet |
| `kyc_authority.json` | `2mh7zXgZHaeDnroJQQdHnLNiierWXdn43VnATbGdATZK` | Signs `verify_profile` KYC approvals | devnet |
| `dispute_authority.json` | `HAHgvXf6uYxTqEuUnkkzTS1EQD8sYd342zgxM2wdqpa2` | Signs `resolve_dispute`, `update_kyc_authority`, `update_fee_vault` — highest privilege | devnet |
| `link_authority.json` | `42fiB5KcC1jEVXxmgPoWqpA3zuKEsZGu77YHmCwNEcrh` | Signs `link_external_elo` (Lichess linking) | devnet |
| `vps_authority.json` | `HZTwvN9AUK1n9jmQydrh5vkpdCBZm13W7qD9jtPZJSQc` | Backend operational authority: `update_elo`, `collect_fee`, tournament creation | devnet |
| `treasury_authority.json` | `9jpjASzudVvpbgw5G7zCf7o6EvCw4ejRVcEN1aBLq4Kd` | Signs `withdraw_treasury` (platform fee withdrawal) | devnet |

**Naming note:** these are all underscore-named. Older hyphen-named copies
(`vps-authority.json`, `kyc-authority.json`, `dispute-authority.json`) existed
from a prior naming convention and held *different, unused* keys — they were
deleted 2026-07-23 after confirming none of them matched the pubkeys above or
any current `.env` reference.

## Backup encryption

| File | Purpose |
|------|---------|
| `backup-age-identity.txt` | age private key (public half is the `BACKUP_AGE_RECIPIENT` used to encrypt backups). Without it backups cannot be decrypted. Keep a copy off this machine. |

## Benchmark keys

`tools/cu_benchmark` uses `program-authority.json` as its master funding
wallet and generates `er-cu-children.json` itself on first run. The old
`er-cu-master.json`, `temp-fund.json` and `fee-payers/` pool were unused and
removed 2026-10-26.

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
ssh root@178.104.55.19 "cat /opt/xfchess/keys/vps_authority.json"
# Paste the output into keys/vps_authority.json locally
```

## Backup reminder

Copy this entire `keys/` directory to a **password-protected location** outside
the repo (e.g. an encrypted USB, 1Password secure note, or Bitwarden attachment).
The `.json` files are the raw private keys — anyone with them controls the funds.

Rotate every key here before mainnet — these are devnet-only, and several were
at one point exposed via `backend/.env` in git history (see project notes on
secret exposure).
