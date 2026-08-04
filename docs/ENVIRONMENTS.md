# Environments & Promotion — XFChess

Dev / staging / prod, and how a build is promoted. Part of the
[Production Reality Plan](PRODUCTION_REALITY_PLAN.md) WS-G. Checklist §5, §14, §22.

## Environments

| Env | Where | Chain | Purpose |
|---|---|---|---|
| **dev / local** | your machine (`just dev` / `just web-stack`) | devnet (public) | day-to-day |
| **staging** | 2nd cheap VPS **or** a `xfchess-staging` systemd slice on the same box, different port + DB dir | devnet + Triton | pre-prod verification with prod-like config |
| **prod** | Hetzner `178.104.55.19` | devnet now → mainnet later | live |

Keep dev/staging/prod **meaningfully similar**; track drift. Config differs only by `.env`
(never by code). All three validate config at startup (`SigningConfig::validate`,
`APP_ENV=production` on prod/staging).

## Config & secrets
- Secrets live only in each env's untracked `.env` (see `.env.example` for the contract).
- `APP_ENV=production` on staging + prod → placeholder/short secrets are hard errors.
- Bad config **cannot** silently run: startup validation exits non-zero; `/readyz` returns 503
  if the DB is unreachable.

## Same-artifact promotion (the rule)
**Build once, promote the same binary** staging → prod. Do **not** rebuild between staging
and prod — that reintroduces "works in staging" drift.

Target model for `ops/scripts/deploy.ps1` (WS-G follow-up):
1. `deploy.ps1 -Environment staging` → build `signing-server` **once**, tag the artifact with
   the git SHA, deploy to staging, run smoke tests (`/health` shows the SHA, `/readyz` = 200).
2. `deploy.ps1 -Environment prod -Artifact <sha>` → **push the identical artifact** (no
   rebuild) to prod, smoke test, done.

Until that switch lands, promotion = deploy the same commit to staging, verify, then run the
prod deploy from the **same commit** (the baked `git_sha` on `/health` proves parity).

## Smoke test (run after every deploy)
```bash
curl -fsS https://$HOST/health   | grep -q '"status":"ok"'      # up + correct git_sha
curl -fsS -o /dev/null -w '%{http_code}' https://$HOST/readyz   # 200 (DB reachable)
```
Wire these into `deploy.ps1` as a post-deploy gate; a failed smoke test triggers rollback.

## Feature flags vs deploy-time config
Keep runtime feature toggles (e.g. enabling a new mode) **separate from deploy** so you can
release without redeploying and roll a feature back independently. Deploy-time config
(ports, DB URLs, RPC endpoints) stays in `.env`.

## Standing up staging (quick path: same-box slice)
```bash
# on the VPS
useradd -r xfchess-staging || true
mkdir -p /opt/xfchess-staging/data
# copy binary + web + nginx server block on a staging subdomain/port
# .env with SIGNING_PORT=8091, SESSION_DB_URL=sqlite:///opt/xfchess-staging/data/sessions.db, APP_ENV=production
cp ops/backend/xfchess-backend.service /etc/systemd/system/xfchess-staging.service  # edit paths/port/user
systemctl enable --now xfchess-staging
```
Point `staging.<domain>` at it in nginx (separate `server` block). Backups (WS-B) can target
prod only; staging DB is disposable.
