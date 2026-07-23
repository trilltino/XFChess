# Admin Panel Local/Production Mode + Production Hardening Plan

**Date:** 2026-07-15
**Status:** IN PROGRESS — Phases 0, 1, 2, 4 implemented & verified; 3, 5, 6 outstanding
**Scope:** Tournament admin panel (Tauri GUI + CLIs), deploy scripts, admin route security, audit trail, treasury key custody
**Related docs:** [treasury-payout-and-close-tournament-fixes.md](treasury-payout-and-close-tournament-fixes.md), [production-reality-implementation.md](production-reality-implementation.md), [../../ops/SECRETS_ROTATION.md](../../ops/SECRETS_ROTATION.md)

---

## Implementation status (2026-07-15)

| Phase | Status | Notes |
|-------|--------|-------|
| 0 Stop-the-bleeding | ✅ DONE | nginx `/admin/` 444 (live), ngrok fallback removed + CLIs panic-if-unset, constant-time `ADMIN_TOKEN`, API-key-in-URL fixed (blob download), dead scripts retired, `.env.example` scoped. Backend `cargo check` + panel `tsc` clean. |
| 1 LOCAL/PROD panel | ✅ DONE (1 approval pending) | `config/environments.ts`, `services/tunnel.ts`, LOCAL/PROD selector, env-aware `useAuth`, prod banner, Dashboard/HetznerSsh URL fixes (deploy user + key), CLI env pickers. **Tunnel user + sshd Match block deployed live and verified end-to-end** (root retained, tunnel shell-denied, `:8091→:8090 /health` = 200). **Pending:** prod `ALLOWED_ORIGINS` must include panel Tauri origins + backend restart (baked into deploy.ps1/.env.example; live change awaits explicit go). |
| 2 Deploy script refresh | ✅ DONE | `deploy.bat`, `copy-key.bat`, `deploy-frontend.ps1`, `package_backend_hetzner.bat`, `run_offline2.bat` deleted; `deploy.ps1` uses `xfchess_vps` key; nginx reconciled to HTTP-only-with-full-routes on the box. |
| 3 Named keys + audit log | ⬜ TODO | Persistent SQLite audit log + per-caller identity not yet built (audit log still in-memory). |
| 4 Stub handlers | ✅ DONE | `rotate_authority` deleted (endpoint+handler+panel UI); `tasks_status`/`logs_stream`/`anti_cheat_reports` now honest (no fabricated data). `force_resign`/`treasury_refund` → explicit **501 not_implemented** — real tx blocked on Phase 5 (no `resign_ix`/`withdraw_treasury_ix` builder; `withdraw_treasury` undeployed; `treasury_authority` placeholder). |
| 5 Treasury activation | ⬜ TODO | **Single dedicated wallet — no multisig** (`treasury_authority` = `8e7Nz…`, keyfile present). Remaining: deploy `withdraw_treasury`, set `TREASURY_AUTHORITY_KEY`, add backend ix builder. Unblocks the two 501 money handlers. |
| 6 Defense in depth | ⬜ TODO | In-app `/admin/*` rate limit, fail-closed `ALLOWED_ORIGINS`, threat model, fail2ban. |

**Money-path contract (Phase 4 → 5):** `force_resign` and `treasury_refund` return `501 {error:"not_implemented"}` by design until Phase 5 lands the on-chain prerequisites. To force a disputed game's outcome today, use `POST /admin/dispute/resolve` (dispute_authority). Do not restore fake-success responses on these routes.

---

## Executive summary

The tournament admin tooling works locally but has **no legitimate production path**: nginx never proxies `/admin/*` (requests fall through to the SPA's `index.html`), so every production admin action to date has gone around the reverse proxy — most likely via the live ngrok URL hardcoded as a fallback in `vps_admin.rs`. Meanwhile the GUI admin panel hardcodes `http://178.104.55.19:8090`, a plain-HTTP address that UFW blocks, so its production dashboards can never have worked.

This plan:

1. Gives the admin panel an explicit **LOCAL / PRODUCTION** environment selector at startup, where PRODUCTION connects through an **SSH tunnel** (never the public HTTPS path).
2. Refreshes every deploy script (one is outright broken, one is a stale duplicate).
3. Fixes the security findings: ngrok fallback, missing nginx admin block, non-constant-time `ADMIN_TOKEN` compare, API-key-in-URL leak, in-memory audit log, stub handlers, single-key treasury custody.

**Design decision — production admin transport: SSH tunnel, not public HTTPS, not WireGuard (yet).**
Rationale: the deploy SSH key infrastructure (`~/.ssh/id_xfchess`, `deploy` user) already exists; the Tauri panel already shells out to `ssh` (`HetznerSsh.tsx` uses `@tauri-apps/plugin-shell`); a tunnel adds zero new public attack surface and zero Hetzner console changes. WireGuard is a fine later upgrade but adds a service to run, keys to manage, and firewall changes for no security gain over `ssh -L` at this scale (1–2 operators).

---

## Verified current state (all file:line checked 2026-07-15)

| # | Finding | Evidence |
|---|---------|----------|
| 1 | `/admin/*` not proxied in prod; falls through to SPA | `ops/nginx/nginx.conf:55-57` (`try_files … /index.html`); no `location /admin/` anywhere in the file |
| 2 | Live ngrok URL as default admin endpoint | `backend/src/bin/vps_admin.rs:22` (`VPS_DEFAULT_URL = "https://unrejuvenated-….ngrok-free.app"`), used on any `SIGNING_SERVICE_URL` error at `:24-35` |
| 3 | `ADMIN_TOKEN` compared with `!=` (timing side channel) | `backend/src/signing/routes/dispute.rs:141-145` |
| 4 | `ADMIN_API_KEY` gate is constant-time (good), but defaults to `"dev"` in debug builds | `backend/src/infrastructure/auth_middleware.rs:26-63` |
| 5 | Audit log: in-memory `Vec`, cap 500, actor hardcoded `"admin"`, lost on restart | `backend/src/signing/routes/admin.rs:49,64,71-73` |
| 6 | API key leaked into URL query string (server logs, history) | `tauri/tournament-admin/src/services/api.ts:258` (`?api_key=${token}`) |
| 7 | GUI panel hardcodes plain-HTTP direct-to-8090 prod URLs (UFW blocks 8090 → always dead) | `tauri/tournament-admin/src/components/DeploymentManager.tsx:95`, `Dashboard.tsx:45,62` |
| 8 | GUI SSH terminal connects as **root** | `tauri/tournament-admin/src/components/HetznerSsh.tsx:17,23,42` |
| 9 | Stub admin handlers that return fake data or do nothing | `admin.rs:213-237` (anti_cheat_reports), `:480` (force_resign), `:574` (treasury_refund), `:651-708` (tasks_status / tls_expiry / logs_stream), `:710-726` (rotate_authority) |
| 10 | Treasury / dispute / VPS authorities: single plaintext keys in `/opt/xfchess/.env` | `backend/src/signing/config.rs:97-106`, `dispute.rs:148-152`; custody hardening in treasury-fixes doc §Fix 4 (decision: single dedicated wallets, no multisig) |
| 11 | `package_backend_hetzner.bat` broken: copies from pre-reorg paths that no longer exist; plain-HTTP `.env.production` fallback | `scripts/package_backend_hetzner.bat:32,48-53` (`ops\xfchess-backend.service`, `ops\nginx.conf`, `ops\deploy.ps1` — all moved to `ops/backend/`, `ops/nginx/`, `ops/scripts/`) |
| 12 | `deploy.bat` is a stale batch duplicate of `deploy.ps1` (which was recently fixed; the .bat was not) | `ops/scripts/deploy.bat` vs `ops/scripts/deploy.ps1` |
| 13 | Dead doc references: `scripts/deploy-coturn.sh` and `deploy-to-hetzner.sh` don't exist | `backend/.env.example:62-64` |
| 14 | Two parallel login implementations in the GUI (TokenAuth verifies via `/admin/players`, useAuth via audit-log) | `TokenAuth.tsx:32`, `hooks/useAuth.tsx:40` |

Things verified as **fine** (no action): `ops/scripts/deploy.ps1` is current (BOM-safe env, JSON health check, clean-clone build, builds real binary `signing-server-http` per `backend/Cargo.toml:22-24`); backend binds `0.0.0.0:8090` behind UFW; `require_api_key` uses constant-time compare; systemd + nginx + certbot flow works.

---

## Phase 0 — Stop the bleeding (small, independent diffs; ship same day)

### 0.1 nginx: explicitly kill `/admin/*` on the public path
`ops/nginx/nginx.conf` — add **above** `location /`:

```nginx
# Admin API is NEVER served publicly. Operators connect via SSH tunnel
# (ssh -L 8091:127.0.0.1:8090). 444 = close connection without response.
location /admin/ {
    return 444;
}
```

Mirror in `ops/staging/nginx-staging.conf`. This converts today's accidental protection (SPA fallback) into an explicit, documented contract.

### 0.2 `vps_admin.rs`: remove the ngrok fallback
Delete `VPS_DEFAULT_URL` (line 22). `vps_base()` must **panic with a clear message** if `SIGNING_SERVICE_URL` is unset — in all build profiles. An admin/treasury tool must fail loudly, never silently reroute over a third-party tunnel. Also delete the `ngrok-skip-browser-warning` header from both `vps_admin.rs:45-48` and `tournament_admin.rs:49-52` once no ngrok path remains.

### 0.3 `dispute.rs`: constant-time `ADMIN_TOKEN` compare
Replace `req.admin_token != expected` (`dispute.rs:142`) with the same `constant_time_eq` used in `auth_middleware.rs:171-182` (move it to a shared module, e.g. `infrastructure/auth_middleware.rs` → `pub fn constant_time_eq`). Keep the empty-token rejection.

**Decision (two secrets):** keep both, with defined scopes — `ADMIN_API_KEY` = transport gate for all `/admin/*`; `ADMIN_TOKEN` = second factor for **financially irreversible** actions only (dispute resolve, future treasury withdraw/refund). Document this in `backend/.env.example`. Rename nothing yet (rename churn belongs in Phase 3 with named keys).

### 0.4 `api.ts`: stop putting the API key in URLs
`tauri/tournament-admin/src/services/api.ts:258` builds `…/admin/archive/download/${type}?api_key=${token}`. Change the download flow to `fetch` with the `X-API-Key` header and save via blob URL. If the backend's archive route reads the query param, add header support there and delete query-param support.

### 0.5 Fix or retire the two broken/stale scripts
- `scripts/package_backend_hetzner.bat`: **retire it** — replace body with an echo pointing at `ops\scripts\deploy.ps1` (it duplicates a now-canonical flow, and all its copy paths are dead). Alternative (not recommended): fix all five paths + change fallback to `https://`.
- `ops/scripts/deploy.bat`: same — reduce to a thin wrapper that calls `powershell -File deploy.ps1 %*`, or delete and update `ops/README.md`. Two divergent implementations of "deploy" is how the recently-fixed ps1 bugs come back.
- `backend/.env.example:62-64`: delete references to nonexistent `deploy-coturn.sh` / `deploy-to-hetzner.sh`; state coturn's actual install story or mark TODO.

**Verify Phase 0:** `curl -k https://<server>/admin/players` → connection closed (no 200, no HTML). `SIGNING_SERVICE_URL= cargo run --bin vps_admin` → immediate panic. `cargo test -p backend`. Grep repo for `ngrok` → only historical docs remain.

---

## Phase 1 — Admin panel LOCAL / PRODUCTION mode

### 1.1 UX: environment selector at startup (GUI)

`TokenAuth.tsx` login screen gets two mode buttons **above** the credential fields (replacing the free-text-URL-first flow):

```
┌──────────────────────────────────────────┐
│            XFCHESS  ORCHESTRATOR         │
│                                          │
│   ┌─────────────┐   ┌────────────────┐   │
│   │  ● LOCAL    │   │  ○ PRODUCTION  │   │
│   └─────────────┘   └────────────────┘   │
│                                          │
│   LOCAL      → http://127.0.0.1:8090     │
│   PRODUCTION → SSH tunnel :8091 → VPS    │
│                                          │
│   ADMIN ACCESS TOKEN  [••••••••••]       │
│   [        INITIATE TERMINAL        ]    │
└──────────────────────────────────────────┘
```

- **LOCAL** — `baseUrl = http://127.0.0.1:8090`, exactly today's behavior. Optionally offer "start local backend" (spawn `cargo run --bin signing-server-http` via the shell plugin, as `start-tournament-admin.bat` does today).
- **PRODUCTION** — the app spawns and owns an SSH tunnel via `@tauri-apps/plugin-shell` (pattern already proven in `HetznerSsh.tsx`):
  `ssh -i ~/.ssh/id_xfchess -o BatchMode=yes -o ExitOnForwardFailure=yes -N -L 8091:127.0.0.1:8090 tunnel@178.104.55.19`
  then `baseUrl = http://127.0.0.1:8091`. Poll `GET /health` through the tunnel before enabling login. Kill the child process on logout/app exit. Surface tunnel state (CONNECTING / UP / DOWN) in the UI.
- The free-text URL field remains under an "Advanced" disclosure for staging/ngrok-era muscle memory, but **plain `http://` to any non-127.0.0.1 host is rejected** in `api.ts` `setCredentials`.
- Persist chosen mode + per-mode token in `localStorage` under separate keys (`admin_token_local`, `admin_token_prod`) so a dev token never gets replayed at prod and vice versa.

### 1.2 Make production mode unmistakable + confirm destructive actions

- Persistent banner strip in `Layout.tsx`: green `LOCAL` / red `PRODUCTION — <server>` on every screen.
- In PRODUCTION mode, mutating actions (create tournament, record result, force resign, treasury ops, bans) require a typed confirmation (`type the tournament ID to confirm`). Implement once as a wrapper around `apiClient` mutating calls, not per-component.

### 1.3 Fix the dead hardcoded prod URLs

- `Dashboard.tsx:45,62` and `DeploymentManager.tsx:95`: derive every URL from `authState.backend_url` (tunnel-aware) instead of `http://<ip>:8090`. These endpoints are currently unreachable in prod (UFW) — this is why prod health tiles never worked.
- `HetznerSsh.tsx`: connect as `deploy@` (restricted sudo user from `deploy.ps1` Step 2a), not `root@`; hoist `serverIp` into a single shared config module (`src/config/environments.ts`) exporting `{ LOCAL, PRODUCTION }` endpoint definitions — the only place the VPS IP appears in the panel.

### 1.4 Consolidate the duplicated login paths

Collapse `TokenAuth.tsx` handshake (`/admin/players`) and `useAuth.tsx` login (`getAuditLog(1)`) into one code path with one cheap, read-only probe: `GET /admin/audit-log?limit=1`. Two implementations already disagree; this is where auth bugs breed.

### 1.5 CLI parity (`tournament_admin`, `vps_admin`)

At startup, if `SIGNING_SERVICE_URL` is unset, print an interactive picker:

```
Select environment:
  1. LOCAL      http://127.0.0.1:8090
  2. PRODUCTION http://127.0.0.1:8091  (requires SSH tunnel — see below)
```

Picking PRODUCTION checks the tunnel with a `GET /health`; if down, print the exact one-liner to open it and exit non-zero. Both CLIs refuse plain-http non-loopback URLs. (`scripts/start-tournament-admin.bat` keeps launching the local stack unchanged.)

### 1.6 Server side: dedicated tunnel user (added to `deploy.ps1`)

New Step 2g in `ops/scripts/deploy.ps1`:

```bash
id tunnel 2>/dev/null || adduser tunnel --disabled-password --shell /usr/sbin/nologin --gecos ''
# authorized_keys: same deploy key (or a dedicated one later)
mkdir -p /home/tunnel/.ssh && cat /root/.ssh/authorized_keys > /home/tunnel/.ssh/authorized_keys
chown -R tunnel:tunnel /home/tunnel/.ssh && chmod 700 /home/tunnel/.ssh && chmod 600 /home/tunnel/.ssh/authorized_keys
```

And an sshd `Match` block (append once, then `sshd -t && systemctl reload sshd`):

```
Match User tunnel
    AllowTcpForwarding yes
    PermitOpen 127.0.0.1:8090
    X11Forwarding no
    AllowAgentForwarding no
    PermitTTY no
    ForceCommand /usr/sbin/nologin
```

Result: the tunnel identity can do exactly one thing — forward to the backend port. Compromise of that key ≠ shell on the box.

**Gotcha to handle:** when `ALLOWED_ORIGINS` is made fail-closed (Phase 6), the panel's origin (`http://localhost:7454` in dev, `tauri://localhost` packaged) must be permitted for `/admin/*` responses, or admin CORS preflights die. Add both to the prod `.env` as part of this phase and test through the tunnel.

**Verify Phase 1:** From a clean machine-state: launch panel → PRODUCTION → tunnel auto-opens → login with prod key → dashboard tiles green → create a **free, 4-player, KYC-off** test tournament → visible via `tournament_admin` CLI pointed at the same tunnel → cancel it. Kill `ssh` mid-session → UI shows DOWN and blocks mutations. LOCAL mode regression: `scripts\start-tournament-admin.bat` flow unchanged.

---

## Phase 2 — Deploy script inventory & refresh

| Script | Verdict | Action |
|--------|---------|--------|
| `ops/scripts/deploy.ps1` | **Current** (recently fixed) | Extend: Step 2g tunnel user (§1.6); add `/admin/` 444 check to Step 10 verify; add `ALLOWED_ORIGINS` reminder incl. panel origins |
| `ops/scripts/deploy.bat` | Stale duplicate | **DELETED 2026-07-15** |
| `ops/scripts/rollback.ps1` | Assumed OK | Smoke-read: confirm binary name `signing-server-http` + `.prev` path match deploy.ps1:267 |
| `ops/scripts/copy-key.bat` | Superseded by deploy.ps1 SSH bootstrap | **DELETED 2026-07-15** |
| `scripts/package_backend_hetzner.bat` | **Broken** (dead paths) | **DELETED 2026-07-15**; CLAUDE.md now points at deploy.ps1 |
| `scripts/run_offline2.bat` | Unreferenced, superseded by run_offline.bat | **DELETED 2026-07-15** |
| `scripts/build.bat`, `run_offline.bat`, `start-tournament-admin.bat` | Local-dev, OK | Keep; update start-tournament-admin.bat only if panel startup flow changes |
| `ops/frontend/deploy-frontend.ps1` | Broken (`$ROOT` path bug, plain-HTTP, BOM) | **DELETED 2026-07-15**; add a `-FrontendOnly` flag to deploy.ps1 if a fast path is wanted |
| `tauri/tournament-admin/ops/` (21 tracked files) | Stale pre-fix snapshot of `ops/`; only ref is the panel's broken sidecar deploy button (`DeploymentManager.tsx:17`) | Pending owner approval to delete (outside 2026-07-15 cleanup scope); fix or remove the deploy button with it |
| `ops/staging/nginx-staging.conf` | Missing admin block | Add `/admin/` 444 (§0.1) |
| `backend/.env.example` | Stale refs | Fix (§0.5); document `ADMIN_TOKEN` scope (§0.3) |

Deliverable: every row moved to "verified current" or deleted, in one commit series, so `ops/` has exactly one canonical path per task.

---

## Phase 3 — Named admin keys + persistent audit log

**Problem:** one shared `ADMIN_API_KEY`, actor recorded as the literal string `"admin"`, log in a 500-entry in-memory Vec (`admin.rs:49-73`).

1. **Named keys.** New env `ADMIN_API_KEYS=alice:3f9a…,ci:77b2…` (fallback: legacy `ADMIN_API_KEY` maps to actor `legacy`). `require_api_key` resolves the matching name via constant-time compare against each candidate and injects `AdminActor(name)` as a request extension.
2. **Persistent log.** Migration `020_admin_audit_log.sql`: `admin_audit_log(id INTEGER PK, ts, actor, action, target, params_json, source_ip, success)` — append-only (no UPDATE/DELETE in code). Write from one axum middleware on the `/admin/*` router (capture method+path+status), not per-handler, so new endpoints can't forget to log. Keep the last-500 in-memory view as a cache for the panel's live tail.
3. **Off-box copy.** The existing 3am B2 `rclone` cron (deploy.ps1:237-239) already syncs `/opt/xfchess/backups`; the audit table lives in `sessions.db`, which is snapshotted there — sufficient for now; note "ship to external log sink" as a later hardening item.
4. Panel: audit view gains an **Actor** column; `rotate_token` endpoint (`admin.rs:736`) is renamed/reworked in this phase to rotate a *named* key.

**Verify:** two keys configured → actions from each appear with correct actor; restart backend → log intact; `sqlite3 … 'select count(*)'` grows.

---

## Phase 4 — Finish or delete the stub handlers

Rule: **an admin button that lies is worse than no button.** For each stub in `admin.rs`:

| Handler | Decision | Notes |
|---------|----------|-------|
| `force_resign` (:480) | **Implement** | Build + submit real resign/timeout tx via the same path as settlement worker; needed mid-incident |
| `treasury_refund` (:574) | **Implement, double-gated** | Needs `withdraw_treasury_ix` builder + `TREASURY_AUTHORITY_KEY` (single wallet, Phase 5) + `ADMIN_TOKEN` second factor (§0.3 scope). Currently honest 501. |
| `rotate_authority` (:710-726) | **Delete endpoint** | True rotation is a ops/SECRETS_ROTATION.md procedure; an endpoint that "just logs" is a footgun. Panel links to the runbook instead |
| `anti_cheat_reports` (:213-237) | **Wire to real data or return 501** | Fake data in a compliance surface is a liability |
| `tasks_status`, `tls_expiry`, `logs_stream` (:651-708) | **Implement cheaply** | tasks: expose real worker tick metadata; TLS: read cert expiry via openssl on the box or from `/metrics`; logs: tail journald via existing SSH pattern in panel, delete backend stub |

Panel components consuming these (`Treasury.tsx`, `MatchManagement.tsx`, `DeploymentManager.tsx`) updated in lockstep; anything left 501 renders an explicit "NOT IMPLEMENTED" state, never fake numbers.

---

## Phase 5 — Treasury activation: single dedicated wallet (no multisig)

**Decision (2026-07-15): drop the Squads multisig — use one dedicated testnet/devnet wallet for `treasury_authority`.** The multisig ceremony (hardware wallets, 2-of-3) is overkill for the current devnet/testnet stage. A single dedicated key, kept separate from `vps_authority`, is the target.

Current state (already in place):

- `treasury_authority` is a **single wallet**: `8e7NzfKVTyeSmsqjuESoXT9WCadkRioyKgJfNeHMG4HM`, keyfile `keys/treasury_authority.json` (gitignored), baked into `programs/xfchess-game/src/constants.rs::treasury_authority::ID`. Verified: keyfile pubkey == constant.
- It is deliberately separate from `vps_authority` (which stays hot for prize-distribution cranks).

To fully activate treasury withdrawals / `treasury_refund` (unblocks the Phase 4 501s):

1. Ensure the deployed devnet program includes `withdraw_treasury` (redeploy if the on-chain program predates it).
2. Set `TREASURY_AUTHORITY_KEY` in the backend `.env` (base58 of `keys/treasury_authority.json`) so the backend can sign withdrawals.
3. Add a backend `withdraw_treasury_ix` builder in `signing/solana/instructions.rs` and wire `treasury_refund` to build+submit it (currently returns 501).
4. Program test: prove `vps_authority` **cannot** call `withdraw_treasury` (authority separation holds).

**Mainnet note:** rotate `treasury_authority` (and `dispute_authority`) to fresh, offline-held keys before mainnet — a single hot key is fine for testnet, not for real funds. Multisig can be revisited then if desired, but is explicitly **not** required by this plan.

---

## Phase 6 — Defense in depth + close production-reality Phase 6

1. **In-app rate limit on `/admin/*`** (tower middleware, e.g. 30 req/min/key): nginx can't rate-limit what it doesn't proxy, and the tunnel bypasses nginx entirely.
2. **Fail-closed `ALLOWED_ORIGINS`** (`router.rs:165-169` currently warns-open) — include panel origins (§1.6 gotcha).
3. **Remove the `"dev"` API-key default from debug builds?** Keep (local DX depends on it) but log loudly at startup and never in release — current behavior already 503s in release; add a startup banner in debug.
4. **Threat model doc** covering: admin tunnel key theft, VPS compromise, hot `vps_authority` abuse, relay secret, prize-distribution crank abuse — one page, lives in `docs/`.
5. **fail2ban on sshd** (tunnel is now the admin plane; brute-force protection on 22 matters more) — add to deploy.ps1 Step 2b.
6. Re-verify nginx rate-limit zones cover the real public routes (production-reality doc Phase 6 item).

---

## What I need from you / the Hetzner console

Nothing in Phases 0–4 requires new Hetzner console changes (the SSH-tunnel design deliberately reuses port 22). But please provide/do the following:

1. **Snapshot before the first deploy of this plan:** Hetzner Cloud Console → Servers → `178.104.55.19` → Snapshots → *Create snapshot*. (Rollback insurance for the sshd `Match` block + nginx changes.)
2. **Tell me whether a Hetzner Cloud Firewall is attached** (Console → Firewalls, or Server → Firewalls tab). If one exists, its rules must mirror UFW (22/80/443 in). If none, UFW is the only layer — fine, but I'll note it in the threat model; optionally attach one as a second layer while you're in the console.
3. **Confirm SSH key access still works** for `root@178.104.55.19` with `~/.ssh/id_xfchess` from your machine (deploy.ps1 will do the rest, including creating the `tunnel` user).
4. **Domain decision (optional but recommended):** prod currently supports IP + self-signed cert. If you point a real domain's DNS A-record at the server and rerun `deploy.ps1 -Domain your.domain`, you get Let's Encrypt. Not a blocker for any phase.
5. **Phase 5:** nothing needed from you — `treasury_authority` is a single dedicated wallet already in `keys/treasury_authority.json` (no multisig, no hardware wallets).

---

## Sequencing & effort

| Phase | Depends on | Size | Risk |
|-------|-----------|------|------|
| 0 Stop-the-bleeding | — | S (5 small diffs) | Low |
| 1 LOCAL/PROD panel mode | 0.1 | M (panel + 2 CLIs + deploy.ps1 step) | Medium (sshd config — snapshot first) |
| 2 Deploy script refresh | 0.5 | S | Low |
| 3 Named keys + audit log | — (parallel w/ 1) | M (migration 020 + middleware + panel col) | Low |
| 4 Stub handlers | 3 (audit logging in place first) | M–L | Medium (touches tx-building paths) |
| 5 Treasury activation | Treasury-fixes redeploy | S–M | Single wallet (no multisig); deploy withdraw_treasury + backend builder |
| 6 Defense in depth | 1 (tunnel is admin plane) | S–M | Low |

Suggested order: **0 → 1 → 2 → 3 → 4**, with **5** scheduled against the mainnet timeline and **6** folded into the same deploys.

## Master verification checklist

- [ ] `curl -k https://<prod>/admin/players` → connection closed (444), not HTML, not 200
- [ ] `vps_admin` / `tournament_admin` with no env → interactive picker; PRODUCTION without tunnel → clear error, exit ≠ 0
- [ ] `grep -ri ngrok backend/ tauri/` → no live URLs
- [ ] Panel PRODUCTION mode: tunnel auto-open, red banner, typed confirmation on mutations, tunnel-kill blocks writes
- [ ] Panel LOCAL mode byte-identical to today's workflow
- [ ] Audit log survives `systemctl restart xfchess-backend`; actor column shows named key
- [ ] `ADMIN_TOKEN` compare is constant-time (code review + shared helper)
- [ ] No API key ever appears in a URL (grep `api_key=` in panel src)
- [ ] Every script in the Phase 2 table verified-current or deleted
- [ ] Stubs: each implemented or returns explicit 501 rendered as NOT IMPLEMENTED
- [ ] Devnet: `withdraw_treasury` callable only by `treasury_authority` (single wallet); `vps_authority` negative test passes
- [ ] `cargo test -p backend`, `cargo clippy`, panel `npm run build` clean
