# Disaster Recovery — XFChess

How we back up, what we can lose, and exactly how to restore. Owner: **@trilltino**
(bus-factor: solo — this doc is the mitigation). Part of the
[Production Reality Plan](PRODUCTION_REALITY_PLAN.md) WS-B.

## What must never be lost vs what's regenerable

| Data | Source of truth | Loss impact | Recovery |
|---|---|---|---|
| Wagers, game results, ELO, tournaments (settled) | **Solana chain** | None — chain is durable | Re-read from RPC |
| User profiles / usernames | Solana `PlayerProfile` PDA (canonical) + SQLite mirror | Mirror regenerable via `sync-profile` | Re-sync from chain |
| Sessions / JWTs | SQLite `sessions.db` | Users re-login | Acceptable; no backup-critical |
| Identity / KYC / vault | SQLite `vault.db` (encrypted fields) | **Must not lose** | Restore from backup |
| Live tournament state | SQLite (tournament store) | In-flight tournaments disrupted | Restore from backup |

## Backup design (WS-B)

- **Scope:** every `*.db` in `/opt/xfchess/data` (currently `sessions.db`, `vault.db`).
- **Method:** `VACUUM INTO` consistent snapshot (safe under WAL + live writes).
- **Encryption:** `age` **asymmetric** — the server holds only the public recipient key
  and *cannot decrypt its own backups*. Private key stays **offline** (password manager).
- **Offsite:** `rclone` to an S3-compatible bucket (**Cloudflare R2** recommended) or Hetzner
  Storage Box. Enable **object-lock / versioning** on the bucket for ransomware-immutable copies.
- **Schedule:** `xfchess-backup.timer` daily 03:17 UTC (persistent catch-up + jitter).
- **Retention:** 14 local days ([backup-db.sh](../deploy/backup/backup-db.sh)); offsite retention via bucket lifecycle.

### RPO / RTO

- **RPO (max data loss):** ≤ 24h (daily backups). Tighten to hourly if wager volume grows.
  On-chain data has RPO = 0 (durable).
- **RTO (time to restore):** target ≤ 30 min for a single DB (download + decrypt + integrity
  check + swap). Measure and record on first drill.

## Setup (one-time, on the VPS)

```bash
# 1. Tools
apt-get install -y sqlite3 age rclone

# 2. Generate the age keypair on your OFFLINE machine (NOT the server):
age-keygen -o xfchess-backup-age.txt          # keep this file offline!
#   -> public key line: "Public key: age1..."  ← put on the server

# 3. Configure rclone remote (once), e.g. Cloudflare R2:
rclone config    # name it e.g. "r2", type "s3", provider "Cloudflare"

# 4. Add to /opt/xfchess/.env (untracked):
#   BACKUP_AGE_RECIPIENT=age1...              (PUBLIC key only)
#   BACKUP_REMOTE=r2:xfchess-backups/db

# 5. Install units:
cp deploy/backup/xfchess-backup.{service,timer} /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now xfchess-backup.timer
systemctl start xfchess-backup.service        # run one now
journalctl -u xfchess-backup.service -n 50    # verify
```

## Restore

```bash
# On a trusted operator machine that has the OFFLINE age private key:
export BACKUP_REMOTE=r2:xfchess-backups/db
export BACKUP_AGE_IDENTITY=/path/to/xfchess-backup-age.txt
sudo systemctl stop xfchess-backend
deploy/backup/restore-db.sh vault            # latest vault.db (integrity-checked first)
sudo systemctl start xfchess-backend
```
`restore-db.sh` refuses to proceed if `PRAGMA integrity_check` isn't `ok`, and keeps a
`.pre-restore-*` copy of the existing file.

## Restore drill (DO THIS — untested backups aren't backups)

Quarterly, restore into a **scratch path** and boot the backend against it:
```bash
BACKUP_REMOTE=... BACKUP_AGE_IDENTITY=... \
  deploy/backup/restore-db.sh vault "" /tmp/restore-test/vault.db
# point a throwaway backend at /tmp/restore-test and hit /health + a read endpoint
```

| Drill date | DB(s) | Result | Restore time | By |
|---|---|---|---|---|
| _pending_ | | | | |

## Regional / total-VPS loss

1. Provision a new VPS, run `deploy/scripts/deploy.ps1` (rebuilds backend + nginx).
2. Restore `vault.db` (+ tournament state) via `restore-db.sh`.
3. Re-point DNS / floating IP.
4. Sessions are disposable — users re-login. On-chain data needs no restore.

**Business cost of downtime:** _quantify (lost wagers/tournament disruption per hour)_ — TBD.
