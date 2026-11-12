# Resetting devnet to a clean state

Use this before a fresh end-to-end run (tournaments, Elo, profiles). Devnet SOL has no value, so
abandoning old accounts is fine.

## What can and cannot be cleared

- **Local databases and local player data: clearable.** Steps 1 and 2.
- **On-chain accounts: not clearable in bulk.** The program has no "close everything" instruction.
  The only closers are `close_tournament`, `cancel_tournament`, `cancel_game`,
  `withdraw_expired_wager`, `withdraw_global_session` and `withdraw_treasury`. Profiles, usernames and
  delegations have no closer at all.
- **So the on-chain reset is a new program ID.** A new program starts with no accounts. The old
  program and its accounts are abandoned (step 4 recovers the program's own rent).
- A new program ID is also what the Elo rewrite needs: the `PlayerProfile` layout changed, and the
  deployed profiles (170 when last counted) use the old layout.

## 1. Local databases (backend)

Stops the local backend, then deletes `sessions.db` and `vault.db` (they are recreated by the
migrations on the next start):

```powershell
just db-reset
```

## 2. Local player data (game client)

Optional, for a first-run experience. This also deletes the node key, so the player becomes a new
node and a new guest identity.

- `%APPDATA%\xfchess\` (node key, profiles)
- `%LOCALAPPDATA%\xfchess\` (logs)

Close the game and the Tauri app first.

## 3. New program ID

1. Make a new program keypair. Keep it safe; it is the program's address:

   ```powershell
   solana-keygen new --no-bip39-passphrase -o target/deploy/xfchess_game-keypair.json
   solana-keygen pubkey target/deploy/xfchess_game-keypair.json
   ```

2. Replace the old ID `8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU` with the new one in:
   - `programs/xfchess-game/src/lib.rs` (both `declare_id!` lines)
   - `Anchor.toml` (devnet and localnet)
   - `justfile`
   - `scripts/verify/verify_devnet_program.ps1`
   - `scripts/dev/dev8.bat`
   - `tools/cu_benchmark/src/chain/mod.rs`
   - `programs/xfchess-game/tests/snapshots/abi.txt` (regenerate with `UPDATE_ABI_SNAPSHOT=1`)
   - `services/backend/.env` (`PROGRAM_ID`; not tracked by git)
   - any deployed backend's environment

   `git grep 8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU` lists the tracked files that still use
   the old ID.

3. Build and deploy:

   ```powershell
   cargo build-sbf --arch v0 --manifest-path programs/xfchess-game-sbf/Cargo.toml
   solana program deploy target/deploy/xfchess_game.so `
     --program-id target/deploy/xfchess_game-keypair.json --url devnet
   ```

4. Check the deployed code matches the build:

   ```powershell
   scripts/verify/verify_devnet_program.ps1 -ProgramId <new id>
   ```

5. Regenerate the IDL (`anchor build`) and copy it to `apps/website/src/lib/xfchess_game.json`.
6. Call `initialize_config` once with the config authority. I did not find a script for this in the
   repo, so use whatever the last deploy used.
7. Restart the backend and rebuild the client.

## 4. Recover the old program's rent (after the new one is verified)

Closing returns the program's own storage rent to a wallet you choose. It does not touch the data
accounts. The old address cannot be redeployed afterwards.

```powershell
solana program close 8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU `
  --url devnet --recipient <your wallet>
```

Run it with the upgrade authority as the signer.

## 5. Check

Count the accounts of the new program. It should be empty apart from the config accounts:

```powershell
py -3 -I scripts/verify/count_program_accounts.py <new id>
```

It uses the public devnet RPC by default; pass a different RPC URL as the second argument.
Account names come from the IDL, so regenerate the IDL first (step 3.5).
