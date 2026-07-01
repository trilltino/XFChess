# Frontend Remediation — End-to-End Plan (web-solana)

Fixes from the frontend audit of `web-solana` (React 19 + Vite 8), plus two
deployment bugs found in [../../.github/workflows/deploy.yml](../../.github/workflows/deploy.yml).
Companion to the VPS guide [E2E_REMEDIATION.md](E2E_REMEDIATION.md). Ordered by
severity. Each item: **Problem → Fix → Verify**, tagged with who does it.

> `[auto]` = applied by this pass · `[you]` = needs your input/secret/decision

- [ ] **FD1** 🟠 CI deploys frontend to the wrong directory `[auto]`
- [ ] **FD2** 🟠 CI builds Vite 8 on Node 18 (build breaks) `[auto]`
- [ ] **F1**  🟠 Dependency vulnerabilities (4 critical / 14 high) `[you]`
- [ ] **F2**  🟠 JWT in localStorage (XSS-exfiltratable) `[you]`
- [ ] **F3**  🟡 API keys baked into public bundle (Helius etc.) `[you]`
- [ ] **F4**  🟡 `web-solana/.env.production` tracked + stale (http/raw-IP) `[auto]`
- [ ] **F5**  🟡 No Content-Security-Policy `[auto]` (report-only)
- [ ] **F6**  🟡 Hardcoded WalletConnect projectId `[auto]`
- [ ] **F7**  🔵 Dead MoonPay link (`pk_test_123`) `[auto]`
- [ ] **F8**  🔵 Mixed-content "Launch Game" fetch `[you]`
- [ ] **F9**  🔵 Committed stale prebuilt bundle (`/index.html` + `/assets`) `[auto]`
- [ ] **F10** 🔵 Pervasive `any` + N+1 price fetch `[you]`

---

## FD1 🟠 CI uploads the frontend to a directory nginx doesn't serve

**Problem:** [deploy.yml](../../.github/workflows/deploy.yml) uploads `web-solana/dist/*`
to `/var/www/xfchess`, but nginx serves `root /opt/xfchess/web`
([../nginx/nginx.conf](../nginx/nginx.conf)) and `deploy.ps1` uploads to
`/opt/xfchess/web`. So a push-to-main frontend deploy lands somewhere nginx never
reads — the site silently doesn't update.

**Fix `[auto]`:** point the workflow at the served path:
```yaml
      target: "/opt/xfchess/web"
```

**Verify:** push to main, then `ssh $SERVER ls -la /opt/xfchess/web/index.html`
shows a fresh mtime, and the live site reflects the change.

---

## FD2 🟠 CI builds Vite 8 on Node 18 (EOL, unsupported)

**Problem:** the workflow pins `node-version: '18'`, but this project uses
`vite@^8` which requires Node 20.19+/22.12+. Node 18 is EOL and the build fails
(or silently misbehaves).

**Fix `[auto]`:** bump to Node 20 (and match locally):
```yaml
          node-version: '20'
```

**Verify:** the `Build` step in Actions succeeds; `web-solana/dist` is produced.

---

## F1 🟠 Dependency vulnerabilities — 4 critical / 14 high

**Problem:** `npm audit --omit=dev` reports 108 issues, mostly transitive via the
WalletConnect / Reown / `viem` / `ws` / `engine.io-client` tree.

**Fix `[you]`:**
```bash
cd web-solana
npm audit fix                 # safe, non-breaking first
npm audit --omit=dev          # see what remains
# For the rest, bump the direct culprits:
npm i @solana/wallet-adapter-wallets@latest @solana/wallet-adapter-walletconnect@latest
npm audit fix --force         # ONLY after testing — may bump majors
npm run build && npm run lint # confirm nothing broke
```

**Verify:** `npm audit --omit=dev` shows 0 critical/high (or a documented,
accepted residue with no browser-reachable impact).

---

## F2 🟠 Session JWT stored in localStorage

**Problem:** the auth token lives in `localStorage` ([SignIn.tsx](../../web-solana/src/pages/SignIn.tsx),
[ProfileViewer.tsx](../../web-solana/src/pages/ProfileViewer.tsx),
[LoginModal.tsx](../../web-solana/src/components/LoginModal.tsx)). Any XSS or
hostile dependency can exfiltrate it → account takeover. It is not `httpOnly`.

**Fix `[you]` (choose one):**
- **Preferred:** issue the session token as an `HttpOnly; Secure; SameSite=Strict`
  cookie from the backend; the SPA stops touching the token. (Backend + CSRF work.)
- **Interim:** keep localStorage but (a) ship the strict CSP from F5, and (b) keep
  JWT TTL short (already tightened per project history) with silent refresh.

**Verify:** with the cookie approach, `localStorage.getItem('xfchess_token')` is
`null` and authed calls still work (cookie sent automatically).

---

## F3 🟡 API keys shipped in the public JS bundle

**Problem:** every `VITE_*` var is embedded in the built JS. `VITE_HELIUS_API_KEY`
is put straight into a request URL ([useWalletUsdBalance.ts:46](../../web-solana/src/hooks/useWalletUsdBalance.ts#L46));
`VITE_MOONPAY_API_KEY` / `VITE_TRANSAK_API_KEY` / `VITE_BANXA_API_KEY` are also
baked in. A paid Helius key can be scraped and its quota abused.

**Fix `[you]`:**
- Move Helius price lookups behind the backend (you already have `/api/rates` and
  wallet routes) so the key stays server-side; the frontend calls your API.
- For on-ramp keys, confirm each is a **publishable** key (safe client-side) — if
  any is a secret key, rotate and move it server-side.
- If you must keep a client Helius key, use a **domain-restricted** one.

**Verify:** grep the built bundle — no secret key strings:
`grep -rEi 'helius|api-key=' web-solana/dist/assets/*.js` returns nothing sensitive.

---

## F4 🟡 `web-solana/.env.production` tracked and stale

**Problem:** committed as `VITE_BACKEND_URL=http://178.104.55.19` (plain HTTP, raw
IP). CI/deploy.ps1 overwrite it at build, but a manual build ships mixed-content.

**Fix `[auto]`:**
```bash
git rm --cached web-solana/.env.production
# ensure .gitignore covers it (it does via **/.env.*)
```

**Verify:** `git ls-files | grep -c web-solana/.env.production` → 0.

---

## F5 🟡 No Content-Security-Policy

**Problem:** R8 added `nosniff`/frame/HSTS but no CSP — the main defense-in-depth
against the F2 token-theft risk.

**Fix `[auto]`, report-only first** (wallet SDKs inject scripts + open RPC/WS
connections, so enforce only after watching reports). Added to
[../nginx/nginx.conf](../nginx/nginx.conf):
```nginx
add_header Content-Security-Policy-Report-Only "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data: https:; connect-src 'self' https: wss:; frame-ancestors 'none'; base-uri 'self'" always;
```

**Verify:** load the site, connect a wallet, confirm no functionality breaks and
review console CSP reports. Then swap `-Report-Only` → enforcing.

---

## F6 🟡 Hardcoded WalletConnect projectId

**Problem:** [App.tsx:67](../../web-solana/src/App.tsx#L67) hardcodes a "placeholder"
projectId — can be rate-limited/revoked and break mobile wallet connect.

**Fix `[auto]`:** read from env with the current value as fallback:
```ts
projectId: import.meta.env.VITE_WALLETCONNECT_PROJECT_ID || '66e133d368e7ec815db15024d2627e2b',
```
`[you]`: register your own at cloud.reown.com and set `VITE_WALLETCONNECT_PROJECT_ID`.

**Verify:** WalletConnect QR modal opens; network tab shows your projectId.

---

## F7 🔵 Dead MoonPay link (`pk_test_123`)

**Problem:** [SignIn.tsx:486](../../web-solana/src/pages/SignIn.tsx#L486) opens
`buy.moonpay.com?apiKey=pk_test_123` — a fake key, so the button is broken.

**Fix `[auto]`:** use the env key; if unset, route users to the in-app funding page
instead of a broken external link.

**Verify:** the "buy SOL" button opens a working MoonPay widget (with a real key)
or the `/fund` page.

---

## F8 🔵 Mixed-content "Launch Game" fetch

**Problem:** [Play.tsx:56](../../web-solana/src/pages/Play.tsx#L56) /
[SignIn.tsx:47](../../web-solana/src/pages/SignIn.tsx#L47) `fetch('http://localhost:7454/...')`
from an HTTPS page is blocked by browsers; only works with the desktop bridge.
It's a timeout fallback so it fails quietly.

**Fix `[you]`:** gate the fallback behind a "desktop app detected" check and show a
"Download the desktop app" message on the web instead of a silent failure.

**Verify:** on the website (no desktop app), the launch flow shows guidance rather
than doing nothing.

---

## F9 🔵 Committed stale prebuilt bundle

**Problem:** root `index.html` + `assets/` (150 tracked files) is an old Vite build.
Neither deploy path uses it (`deploy.ps1` uploads `web-solana/dist`; `deploy.yml`
rebuilds), so it only leaks stale baked-in values and bloats the repo.

**Fix `[auto]`:**
```bash
git rm --cached -r index.html assets
# add to .gitignore: /index.html and /assets/
```

**Verify:** `git ls-files | grep -cE '^(index.html|assets/)'` → 0; files remain on
disk locally.

---

## F10 🔵 Type safety + minor perf

**Problem:** `any` used for profiles, RPC responses, wallet objects (e.g.
[ProfileViewer.tsx](../../web-solana/src/pages/ProfileViewer.tsx),
[useWalletUsdBalance.ts](../../web-solana/src/hooks/useWalletUsdBalance.ts)); Helius
token prices fetched sequentially (N+1).

**Fix `[you]`:** introduce typed models for the Anchor profile + RPC shapes;
replace the price loop with `Promise.all`. Incremental, low-risk.

---

## What this pass applies now (`[auto]`)
FD1, FD2, F4, F5, F6, F7, F9. Then `npm run build` to confirm the frontend still
compiles. Everything tagged `[you]` (F1 dep bumps, F2 cookie, F3 key proxy, F8
launch UX, F10 types) is left for a scoped follow-up.

## Final verification
```bash
cd web-solana && npm run build          # succeeds
git ls-files | grep -cE '^(index.html|assets/|web-solana/.env.production)'  # 0
grep -c CONTENT-SECURITY-POLICY ../deploy/nginx/nginx.conf  # >=1 (case-insensitive)
```
