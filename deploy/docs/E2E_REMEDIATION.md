# VPS Remediation — End-to-End Fix Guide

Fixes for the issues found in the VPS/deployment audit (site + game). Work
top-to-bottom: ordered by severity and dependency. Every step has a **Verify**
so you know it worked. Server is Hetzner `178.104.55.19`; deploy tooling is
[../scripts/deploy.ps1](../scripts/deploy.ps1).

> Legend: 🔴 critical · 🟠 high · 🟡 medium · 🔵 low
> `SERVER` = `178.104.55.19` (or your domain once you have one).

- [ ] **R1** 🔴 Remove & rotate committed secrets
- [ ] **R2** 🟠 Fix nginx routing so the game works in prod
- [ ] **R3** 🟠 Correct + extend rate limiting
- [ ] **R4** 🟠 Lock down CORS to an allowlist
- [ ] **R5** 🟡 Fix Docker/monitoring public exposure
- [ ] **R6** 🟡 Consolidate to one `.env.production`
- [ ] **R7** 🟡 Point the release game at HTTPS, fix `/health` check
- [ ] **R8** 🔵 nginx security headers + hardening
- [ ] **R9** 🔵 deploy.ps1: don't auto-commit

---

## R1 🔴 Remove & rotate committed secrets

**Problem:** [../backend/.env.production](../backend/.env.production) is tracked in
git with real `JWT_SECRET`, `IDENTITY_ENCRYPTION_KEY`, `IDENTITY_SALT`,
`RELAY_SHARED_SECRET`. Anyone with repo access can forge JWTs (impersonate any
user) and decrypt identity/KYC data. The file is an **unused leftover** —
`deploy.ps1` uses `deploy/.env.production` (untracked).

### Steps

1. Untrack the file (keeps it on disk, removes from future commits):
   ```bash
   git rm --cached deploy/backend/.env.production
   ```
2. Confirm `.gitignore` already blocks it (it does via `**/.env.*`) — this line
   must exist and there must be **no** `!` exception re-including it:
   ```bash
   git check-ignore deploy/backend/.env.production   # should print the path
   ```
3. Commit the removal:
   ```bash
   git commit -m "chore: untrack committed production secrets"
   ```
4. **Rotate all four secrets** — they are compromised (still in git history).
   Follow [../SECRETS_ROTATION.md](../SECRETS_ROTATION.md) §4 for
   `JWT_SECRET` / `IDENTITY_ENCRYPTION_KEY` / `IDENTITY_SALT`:
   ```bash
   openssl rand -hex 32   # JWT_SECRET
   openssl rand -hex 32   # IDENTITY_ENCRYPTION_KEY   (run identity re-encrypt migration first)
   openssl rand -hex 32   # IDENTITY_SALT
   ```
   For `RELAY_SHARED_SECRET`, generate a new value and update it in **both** the
   server `/opt/xfchess/.env` and the game client's `RELAY_SHARED_SECRET`.
   ```bash
   ssh $SERVER nano /opt/xfchess/.env          # paste new values
   ssh $SERVER sudo systemctl restart xfchess-backend
   ```
5. (Optional but recommended) Purge the secrets from git history with
   `git filter-repo` or BFG, then force-push and re-clone. If you can't, treat
   the old values as permanently burned — rotation in step 4 is what actually
   protects you.

**Verify:**
```bash
git ls-files | grep -c "backend/.env.production"   # must be 0
ssh $SERVER 'systemctl is-active xfchess-backend'  # active
```
Log out / log in on the site — old JWTs should now be rejected.

---

## R2 🟠 Fix nginx routing so the game works in prod

**Problem:** The native game POSTs to `/move/record` and `/session/sign`
([../../src/multiplayer/network/vps/game.rs](../../src/multiplayer/network/vps/game.rs),
[session.rs](../../src/multiplayer/network/vps/session.rs)), but
[../nginx/nginx.conf](../nginx/nginx.conf) only proxies `/api/`, `/auth/`,
`/signup`, `/ws`. Those root-path endpoints (and `/health`) fall through to the
SPA `index.html`, so move recording and session signing are broken in prod.

### Steps

Add these `location` blocks to the **HTTPS server** in
[../nginx/nginx.conf](../nginx/nginx.conf), *above* `location / { try_files ... }`:

```nginx
    # ── Game session signing — rate limited ───────────────────────────────────
    location /session/ {
        limit_req zone=signing burst=5 nodelay;
        proxy_pass         http://127.0.0.1:8090;
        proxy_http_version 1.1;
        proxy_set_header   Host $host;
        proxy_set_header   X-Real-IP $remote_addr;
        proxy_set_header   X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header   X-Forwarded-Proto $scheme;
    }

    # ── Game move recording — rate limited ────────────────────────────────────
    location /move/ {
        limit_req zone=moves burst=20 nodelay;
        proxy_pass         http://127.0.0.1:8090;
        proxy_http_version 1.1;
        proxy_set_header   Host $host;
        proxy_set_header   X-Real-IP $remote_addr;
        proxy_set_header   X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header   X-Forwarded-Proto $scheme;
    }

    # ── Game session/create + finalize + status ───────────────────────────────
    location /game/ {
        proxy_pass         http://127.0.0.1:8090;
        proxy_http_version 1.1;
        proxy_set_header   Host $host;
        proxy_set_header   X-Forwarded-Proto $scheme;
    }

    # ── Health check (used by deploy verify + monitors) ───────────────────────
    location = /health {
        proxy_pass         http://127.0.0.1:8090;
        proxy_set_header   Host $host;
    }
```

Then remove the now-dead root `location /signup { ... }` block (the mailer moved
to `/api/signup`, already covered by `location /api/`).

> Double-check the full set of root-mounted routes with
> `grep -rn 'route("' backend/src/signing/routes/main.rs backend/src/signing/routes/debug.rs`
> and add a `location` for any others the game calls.

Redeploy nginx config (or full deploy):
```powershell
.\deploy\scripts\deploy.ps1 -Server 178.104.55.19 -SkipBuild
```

**Verify:**
```bash
curl -sk https://$SERVER/health                       # -> OK (not HTML)
curl -sko /dev/null -w '%{http_code}\n' https://$SERVER/session/sign  # 400/401/405, NOT 200-HTML
```
A 4xx here is correct (it reached the backend). Getting HTML back means it's
still falling through to the SPA.

---

## R3 🟠 Correct + extend rate limiting

**Problem:** Zones target `/api/sign/` and `/api/move/`, which match no real
route. Auth, signup, and waitlist have no throttle — and signup/waitlist now
send email + write to disk (abuse vector).

### Steps

Replace [../nginx/xfchess_rate_limit.conf](../nginx/xfchess_rate_limit.conf) with:
```nginx
# XFChess rate-limit zones (nginx http context → /etc/nginx/conf.d/)
limit_req_zone $binary_remote_addr zone=signing:10m rate=10r/m;   # /session/*
limit_req_zone $binary_remote_addr zone=moves:10m   rate=60r/m;   # /move/*
limit_req_zone $binary_remote_addr zone=auth:10m    rate=10r/m;   # /api/auth/*
limit_req_zone $binary_remote_addr zone=mail:10m    rate=3r/m;    # signup + waitlist
limit_req_zone $binary_remote_addr zone=api:10m     rate=120r/m;  # general /api/*
```

The `signing`/`moves` zones are consumed by the R2 blocks. Add throttles to the
email + auth endpoints in [../nginx/nginx.conf](../nginx/nginx.conf):
```nginx
    location = /api/signup   { limit_req zone=mail burst=2 nodelay; proxy_pass http://127.0.0.1:8090; proxy_set_header Host $host; proxy_set_header X-Forwarded-Proto $scheme; }
    location = /api/waitlist { limit_req zone=mail burst=2 nodelay; proxy_pass http://127.0.0.1:8090; proxy_set_header Host $host; proxy_set_header X-Forwarded-Proto $scheme; }
    location /api/auth/      { limit_req zone=auth burst=5 nodelay; proxy_pass http://127.0.0.1:8090; proxy_set_header Host $host; proxy_set_header X-Forwarded-Proto $scheme; }
```
Put these **before** the generic `location /api/` block (nginx matches exact/prefix
by specificity, but ordering keeps intent clear). Optionally add
`limit_req zone=api burst=20 nodelay;` inside the generic `/api/` block.

**Verify:**
```bash
for i in $(seq 1 6); do curl -sk -o /dev/null -w '%{http_code} ' -X POST \
  https://$SERVER/api/waitlist -H 'Content-Type: application/json' -d '{"email":"x@y.com"}'; done
# expect some 200s then 503 (rate limited)
```

---

## R4 🟠 Lock down CORS to an allowlist

**Problem:** [../../backend/src/infrastructure/router.rs](../../backend/src/infrastructure/router.rs)
uses `CorsLayer::permissive()` (any origin). `ALLOWED_ORIGINS` in `.env` is never
read.

### Steps

Replace the permissive layer (around line 108) with an allowlist built from
`ALLOWED_ORIGINS` (comma-separated). Add near the top of the router module:

```rust
use tower_http::cors::{AllowOrigin, CorsLayer};
use axum::http::{header, HeaderValue, Method};

fn cors_layer() -> CorsLayer {
    let raw = std::env::var("ALLOWED_ORIGINS").unwrap_or_default();
    let origins: Vec<HeaderValue> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();

    let base = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    if origins.is_empty() {
        // Dev fallback only — never leave ALLOWED_ORIGINS empty in prod.
        base.allow_origin(AllowOrigin::any())
    } else {
        base.allow_origin(AllowOrigin::list(origins))
    }
}
```
Then swap the layer:
```rust
// before: .layer(tower_http::cors::CorsLayer::permissive())
.layer(cors_layer())
```
Set the value on the server (`/opt/xfchess/.env`):
```
ALLOWED_ORIGINS=https://yourdomain.com
```
Rebuild + redeploy the backend (`deploy.ps1` without `-SkipBuild`).

**Verify:**
```bash
# Allowed origin echoes back:
curl -skI -H 'Origin: https://yourdomain.com' https://$SERVER/api/rates | grep -i access-control-allow-origin
# Disallowed origin: header absent
curl -skI -H 'Origin: https://evil.com' https://$SERVER/api/rates | grep -i access-control-allow-origin
```

---

## R5 🟡 Fix Docker/monitoring public exposure

**Problem:** [../../docker-compose.yml](../../docker-compose.yml) and
[../monitoring/docker-compose.yml](../monitoring/docker-compose.yml) publish
`8090`, `3000` (Grafana **admin/admin**), `9090`, `9093`, `9100` on `0.0.0.0`.
Docker's iptables rules **bypass UFW**, so on the VPS these become public.

### Steps

1. Bind every published port to localhost in both compose files:
   ```yaml
   ports:
     - "127.0.0.1:3000:3000"   # grafana
     - "127.0.0.1:9090:9090"   # prometheus
     - "127.0.0.1:9093:9093"   # alertmanager
     - "127.0.0.1:9100:9100"   # node-exporter
     - "127.0.0.1:8090:8090"   # backend (root compose)
   ```
2. Change the Grafana admin password (never `admin/admin`):
   ```yaml
   environment:
     - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_ADMIN_PASSWORD:?set in .env}
   ```
3. **Decide the monitoring model.** `deploy.ps1` already installs native
   `node_exporter` (localhost) and expects native Prometheus. Don't *also* run the
   monitoring compose on the VPS — pick one. If you keep native, treat the compose
   files as **local-dev only** and note it at the top of each.
4. To reach Grafana on the VPS, tunnel instead of exposing:
   ```bash
   ssh -L 3000:127.0.0.1:3000 $SERVER   # then open http://localhost:3000
   ```

**Verify (from your laptop, not the server):**
```bash
curl -m 5 -o /dev/null -w '%{http_code}\n' http://$SERVER:3000   # expect timeout/refused
nmap -p 3000,9090,9093,9100 $SERVER                              # closed/filtered
```

---

## R6 🟡 Consolidate to one `.env.production`

**Problem:** Two files (`deploy/.env.production` [used] and
`deploy/backend/.env.production` [tracked, different `PROGRAM_ID`/secrets]) — risk
of shipping the wrong config.

### Steps
1. After R1 removes the tracked file, keep **only** `deploy/.env.production`
   (untracked) as the bootstrap source `deploy.ps1` reads.
2. Reconcile `PROGRAM_ID`: confirm the live server value matches
   `declare_id!` in [../../programs/xfchess-game/src/lib.rs](../../programs/xfchess-game/src/lib.rs)
   (`8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU` for devnet).
   ```bash
   ssh $SERVER grep PROGRAM_ID /opt/xfchess/.env
   ```
3. Keep `deploy/backend/.env.example` as the documented template (no real values).

**Verify:** `ls deploy/*.env.production deploy/backend/*.env.production` shows only
the untracked root one (plus `.example`).

---

## R7 🟡 Point the release game at HTTPS + real host

**Problem:** [../../src/multiplayer/network/vps/client.rs](../../src/multiplayer/network/vps/client.rs)
hardcodes `VPS_PROD_URL = "http://178.104.55.19"` (plain HTTP, raw IP →
self-signed cert + 301 redirect loses POST body).

### Steps
1. Once you own a domain and have a Let's Encrypt cert (R2 deploy with `-Domain`),
   change:
   ```rust
   const VPS_PROD_URL: &str = "https://yourdomain.com";
   ```
2. Until then, either keep prod builds pointed at the local backend for testing,
   or set `SIGNING_SERVICE_URL=https://yourdomain.com` in the game's environment
   (it takes precedence over the constant).

**Verify:** run a release game build, make a move, and confirm on the server:
```bash
ssh $SERVER journalctl -u xfchess-backend -f | grep record_move
```

---

## R8 🔵 nginx security headers + hardening

Add to the HTTPS `server` block in [../nginx/nginx.conf](../nginx/nginx.conf):
```nginx
    server_tokens off;
    add_header X-Content-Type-Options nosniff always;
    add_header X-Frame-Options DENY always;
    add_header Referrer-Policy strict-origin-when-cross-origin always;
    add_header Strict-Transport-Security "max-age=63072000; includeSubDomains" always;
    client_max_body_size 1m;
```
(Keep only one `Strict-Transport-Security` line — replace the existing one.)

**Verify:**
```bash
curl -skI https://$SERVER | grep -iE 'x-content-type|x-frame|referrer|strict-transport|server:'
```

---

## R9 🔵 deploy.ps1: don't auto-commit/push

**Problem:** [../scripts/deploy.ps1](../scripts/deploy.ps1) (~line 84) auto-commits
and pushes a dirty tree — can silently ship local secrets/junk.

### Steps
Replace the auto-commit block with an abort:
```powershell
$dirty = git status --porcelain 2>&1
if ($dirty) {
    Write-Host "ABORT: uncommitted changes. Commit or stash before deploying." -ForegroundColor Red
    exit 1
}
```

**Verify:** run `deploy.ps1` with a dirty working tree — it should refuse.

---

## Final verification pass

```bash
# 1. Secrets untracked
git ls-files | grep -c "backend/.env.production"          # 0
# 2. Game endpoints reachable
curl -sk https://$SERVER/health                           # OK
# 3. CORS locked
curl -skI -H 'Origin: https://evil.com' https://$SERVER/api/rates | grep -ci access-control-allow-origin  # 0
# 4. Rate limit active
for i in $(seq 1 6); do curl -sk -o /dev/null -w '%{http_code} ' -X POST https://$SERVER/api/waitlist -H 'Content-Type: application/json' -d '{"email":"x@y.com"}'; done  # trailing 503s
# 5. Monitoring not public
curl -m 5 -o /dev/null -w '%{http_code}\n' http://$SERVER:3000   # refused/timeout
# 6. Security headers
curl -skI https://$SERVER | grep -ci x-content-type-options       # 1
# 7. Service healthy
ssh $SERVER systemctl is-active xfchess-backend                   # active
```

Tick each box at the top as you complete it. R1 first, always.
