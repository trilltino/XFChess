# Secure Commits

How to commit to XFChess without leaking secrets or runtime state. **This repo is
public** (`github.com/trilltino/XFChess`) — anything committed here is world-readable
forever, even after deletion.

## TL;DR

- **Never commit** secrets, `.env` files, databases (`*.db`), keypairs, PID files, or `node_modules/`.
- Commit **schema and migrations**, never the live database file.
- Provide `*.env.example` with blank/placeholder values so others know the shape.
- If a secret lands in a commit, **rotate the secret** — scrubbing git history does not un-publish it.

---

## What must never be committed

| Category | Examples | Why |
|----------|----------|-----|
| Secrets / env | `backend/.env`, `web-solana/.env` | Contains JWT secret, encryption key, API keys, on-chain authority keys |
| Live databases | `sessions.db`, `backend/sessions.db`, `backend/vault.db` | User data + churns binary diffs; never reproducible across machines |
| Keypairs | `*-keypair.json`, `id.json`, `*.keypair` | Solana private keys = direct control of funds/authority |
| Runtime state | `backend/.backend.pid`, `*.pid`, `*.log` | Machine-specific noise |
| Dependencies | `node_modules/`, `target/` | Reproducible from lockfiles; bloats the repo (we had **55k** node_modules files tracked) |
| Editor scratch | `*.tmp`, `*.tmp.*` | Accidental saves |

All of the above are covered by [`.gitignore`](../.gitignore). **But `.gitignore` only
stops _new_ files — it does not untrack files that were already committed.** If something
slipped in before the ignore rule existed, untrack it explicitly:

```bash
git rm --cached <path>        # single file, keeps it on disk
git rm -r --cached <dir>      # directory (e.g. node_modules)
```

Then commit the removal. The file stays on your machine; git just stops tracking it.

---

## Working with the database

The SQLite databases (`sessions.db`, `vault.db`) are **runtime artifacts**, not source.

- ✅ Commit **migrations** in `backend/migrations/` — they are the source of truth for schema.
- ✅ Commit **schema-only** dumps if you need a reference (`sqlite3 sessions.db .schema > docs/schema.sql`) — schema has no user data.
- ❌ Never commit the `.db` / `.db-wal` / `.db-shm` files themselves.

A fresh checkout builds its database by running migrations, so the binary file is never needed in git.

---

## Before every commit

1. **Review what you're staging — never blind-add:**
   ```bash
   git status
   git diff --cached --stat
   ```
   Prefer staging specific paths (`git add backend/src/...`) over `git add -A`.

2. **Scan the staged diff for secrets:**
   ```bash
   git diff --cached | grep -iE 'secret|api_key|private_key|password|-----BEGIN|[A-Za-z0-9]{40,}'
   ```
   A hit isn't always a leak (base58 pubkeys, hashes), but look at every one.

3. **Confirm no ignored-but-tracked junk is staged:**
   ```bash
   git diff --cached --name-only | grep -iE '\.env$|\.db$|\.pid$|node_modules/'
   ```
   This should print nothing.

---

## Provide `.env.example`, not `.env`

So collaborators know which variables exist without seeing the values:

```bash
# backend/.env.example  (committed — placeholder values only)
JWT_SECRET=
IDENTITY_ENCRYPTION_KEY=
ADMIN_API_KEY=
HELIUS_API_KEY=
SOLANA_RPC_URL=https://api.devnet.solana.com
PROGRAM_ID=8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU
```

The `.gitignore` whitelists `*.env.example` (`!**/.env.example`) so these are allowed through.

---

## Optional: a pre-commit guard

Block the most common mistakes automatically. Save as `.git/hooks/pre-commit` and
`chmod +x` it (Windows: Git Bash respects this hook):

```bash
#!/bin/sh
# Reject commits that stage secrets, databases, PID files, or node_modules.
blocked=$(git diff --cached --name-only | grep -iE '(^|/)\.env$|\.env\.[^/]*$|\.db$|\.db-(wal|shm)$|\.pid$|node_modules/|-keypair\.json$|(^|/)id\.json$' | grep -vE '\.env\.example$')
if [ -n "$blocked" ]; then
  echo "✋ Refusing to commit files that should never be tracked:"
  echo "$blocked" | sed 's/^/   /'
  echo "Use 'git rm --cached <file>' or unstage them. Override (not recommended): git commit --no-verify"
  exit 1
fi
```

This is local-only (hooks aren't shared via git). For team-wide enforcement use a
managed hook runner (e.g. `pre-commit`, `lefthook`) checked into the repo, or a
secret scanner like `gitleaks` in CI.

---

## If a secret was already committed (incident runbook)

Scrubbing history (`git filter-repo`, force-push) **does not** help once a commit has
been pushed to a public remote — it's already cloned, forked, and archived. The only
real fix is to **invalidate the secret**:

1. **Rotate it.** Generate a new value and deploy it.
   - Symmetric secrets (`JWT_SECRET`, `IDENTITY_ENCRYPTION_KEY`, `ADMIN_API_KEY`): mint new random values. Rotating `JWT_SECRET` invalidates existing sessions — acceptable.
   - API keys (`HELIUS_API_KEY`): revoke + reissue in the provider dashboard.
   - **On-chain authority keypairs** (`KYC_AUTHORITY_KEY`, `DISPUTE_AUTHORITY_KEY`, `VPS_AUTHORITY_KEY`, `FEE_PAYER_KEYS`): generate new keypairs and **transfer authority on-chain**, then move any funds out of the exposed fee-payer accounts. Highest priority — these control money.
2. **Untrack** the file going forward (`git rm --cached`, commit) and confirm `.gitignore` covers it.
3. *(Optional)* Rewrite history to remove the file so future clones don't see it — but treat step 1 as the actual remediation, not this.
