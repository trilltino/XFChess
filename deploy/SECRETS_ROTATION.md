# Secrets Rotation Procedure

Rotate all secrets immediately if a server is compromised, a key file leaks, or
as part of a scheduled quarterly rotation. Each secret is independent — rotate
only what is affected.

---

## 1. KYC_AUTHORITY_KEY

**What it is:** Ed25519 keypair used to sign KYC approval transactions on-chain.
The public key is registered in the `PlayerProfile` PDA.

**Impact of compromise:** An attacker can forge KYC approvals for any wallet.

### Rotation steps

1. Generate a new keypair locally:
   ```bash
   solana-keygen new --outfile ~/.config/xfchess/kyc-authority-NEW.json --no-bip39-passphrase
   solana-keygen pubkey ~/.config/xfchess/kyc-authority-NEW.json
   ```
2. Call the `update_kyc_authority` admin instruction on-chain with the new pubkey
   (requires the current `DISPUTE_AUTHORITY_KEY` as co-signer).
3. Update `/opt/xfchess/.env` on the server:
   ```
   KYC_AUTHORITY_KEY=<base58-encoded new private key>
   ```
4. Restart the backend: `sudo systemctl restart xfchess-backend`
5. Verify: send a test KYC approval and confirm on-chain acceptance.
6. Securely delete the old keypair file and update the password manager entry.

---

## 2. DISPUTE_AUTHORITY_KEY

**What it is:** Ed25519 keypair that can resolve on-chain disputes and perform
admin-only program instructions (`update_kyc_authority`, `update_fee_vault`).

**Impact of compromise:** An attacker can resolve disputes arbitrarily and
redirect fee vaults. This is the highest-privilege key.

### Rotation steps

1. Generate a new keypair:
   ```bash
   solana-keygen new --outfile ~/.config/xfchess/dispute-authority-NEW.json --no-bip39-passphrase
   solana-keygen pubkey ~/.config/xfchess/dispute-authority-NEW.json
   ```
2. Call `update_dispute_authority` on-chain with the new pubkey. This instruction
   requires both the old `DISPUTE_AUTHORITY_KEY` and the fee payer to sign.
   ```bash
   cargo run --bin pda --features solana -- update-dispute-authority \
     --new-authority <new-pubkey>
   ```
3. Update `/opt/xfchess/.env`:
   ```
   DISPUTE_AUTHORITY_KEY=<base58-encoded new private key>
   ```
4. Restart: `sudo systemctl restart xfchess-backend`
5. Verify: check that `get_dispute_authority` on-chain matches the new pubkey.
6. Shred old key: `shred -u ~/.config/xfchess/dispute-authority-OLD.json`

---

## 3. VPS_SIGNER_KEY

**What it is:** Ed25519 keypair the backend uses to co-sign game transactions
(session delegation, `record_move`). This key holds no SOL and has no admin
privileges — it is whitelisted per-game in the `SessionDelegation` PDA.

**Impact of compromise:** An attacker can submit moves for any active game
session that has delegated to this key.

### Rotation steps

1. Generate a new keypair:
   ```bash
   solana-keygen new --outfile ~/.config/xfchess/vps-signer-NEW.json --no-bip39-passphrase
   solana-keygen pubkey ~/.config/xfchess/vps-signer-NEW.json
   ```
2. Deploy the new key to the server:
   ```bash
   scp ~/.config/xfchess/vps-signer-NEW.json root@SERVER:/opt/xfchess/keys/vps-signer.json
   ssh root@SERVER chmod 600 /opt/xfchess/keys/vps-signer.json
   ```
3. Update `/opt/xfchess/.env`:
   ```
   VPS_SIGNER_KEY=<base58-encoded new private key>
   ```
4. Restart: `sudo systemctl restart xfchess-backend`
5. Active sessions delegated to the old key will fail after restart. Players will
   see a session-expired error and need to re-delegate (one wallet popup). This
   is expected and recoverable — communicate to users if rotation is planned.
6. Revoke old sessions on-chain via `revoke_session_delegation` for all active
   sessions pointing to the old pubkey (optional cleanup; sessions expire anyway).

---

## 4. JWT_SECRET / IDENTITY_ENCRYPTION_KEY / IDENTITY_SALT

These are symmetric secrets used for session JWTs and identity data encryption.
They do not touch Solana.

**Rotation steps:**

1. Generate new values:
   ```bash
   openssl rand -hex 32   # JWT_SECRET
   openssl rand -hex 32   # IDENTITY_ENCRYPTION_KEY
   openssl rand -hex 32   # IDENTITY_SALT
   ```
2. Rotating `IDENTITY_ENCRYPTION_KEY` or `IDENTITY_SALT` will invalidate all
   currently encrypted identity records — run the re-encryption migration script
   first (see `backend/src/db/migrate_identity.rs`) before updating the env.
3. Rotating `JWT_SECRET` invalidates all active login sessions — all users will
   be logged out immediately. Schedule during low-traffic hours.
4. Update `/opt/xfchess/.env` and restart.

**Disaster recovery:** `/opt/xfchess/.env` exists only on the server and is
deliberately excluded from the B2 backup set (it holds secrets). But `vault.db`
backups are ciphertext — without `IDENTITY_ENCRYPTION_KEY` and `IDENTITY_SALT`
they are unrecoverable. Keep a current copy of the production `.env` in a
password manager (or other encrypted offline store), and update it whenever a
secret is rotated. Losing the server *and* these two values means losing all
encrypted identity/KYC data permanently.

---

## Checklist after any rotation

- [ ] New secret is deployed to `/opt/xfchess/.env` (chmod 600)
- [ ] Backend restarted and health check passes (`GET /health`)
- [ ] On-chain authority updated if applicable (KYC / Dispute)
- [ ] Old secret removed from all local machines and password manager
- [ ] Old key file shredded (`shred -u`)
- [ ] Rotation logged (date, who rotated, reason)
