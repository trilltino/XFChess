# Tournament Admin — Connection Re-architecture Plan

**Date:** 2026-08-21
**Status:** PROPOSED — nothing implemented yet
**Scope:** How `tauri/tournament-admin` reaches the production backend
**Trigger:** A ~2-hour debugging session where PRODUCTION login failed through six
distinct, unrelated-looking errors. All six trace back to two architectural
decisions, not to six separate bugs.

---

## 1. What actually happened (the evidence)

Every failure below was hit *in sequence*, each one masking the next:

| # | Symptom | Real cause | Layer |
|---|---------|-----------|-------|
| 1 | Panel showed stale UI after source edits | `just admin` skipped rebuild when `dist/` existed | Build |
| 2 | Fixes didn't apply after rebuild | Tauri doesn't hot-reload; old process still running | Process |
| 3 | `error deserializing scope: missing field 'name'` | `admin-shell.json` entries lacked the `name` field `tauri-plugin-shell` 2.3.5 requires | ACL |
| 4 | `shell.spawn not allowed on window ... URL: http://localhost:7454/` | Capability trusted only `local`, but window is a *remote* `http://` origin | ACL |
| 5 | "Tunnel opened but backend did not answer /health" | 8s timeout lost the race against a real SSH handshake | Tunnel |
| 6 | Same, intermittently | Orphaned `ssh.exe` processes squatting port 8091 from earlier attempts | Process |
| 7 | `net::ERR_FAILED 200 (OK)` + CORS block | Backend `ALLOWED_ORIGINS` lacks `http://localhost:7454` | CORS |

**#7 is the smoking gun.** The response was `200 OK` — the tunnel worked, the
backend answered correctly, and the browser threw the response away because of a
missing CORS header. Every "tunnel down" message you saw for the last hour was
a lie told by the browser's error model.

### The two root causes

**A. The admin SPA is served over HTTP from an embedded axum server**
(`http://localhost:7454/tournament-admin/`) rather than as a normal Tauri app.
That makes the webview a *remote* origin, which is what causes #4 (ACL scoping)
and #7 (CORS). Causes #3 too, indirectly — the ACL path was never exercised
until spawn was first reached.

**B. An OS process (`ssh.exe`) is spawned and supervised from JavaScript.**
Nothing owns its lifetime, so it leaks on window-close and rebuild (#6), and
its readiness is guessed at with a hardcoded timeout (#5).

### Proof this is drift, not design

`ALLOWED_ORIGINS` on the production box currently reads:

```
http://178.104.55.19,http://tauri.localhost,https://tauri.localhost,http://localhost:7455,http://localhost:1420
```

- `tauri.localhost` — the origin a webview gets from Tauri's **native asset protocol**
- `1420` — Tauri's **default Vite dev port**
- `7455` — off-by-one from the actual `7454`

This list describes a panel that loaded as a normal Tauri app. At some point it
was moved to the embedded HTTP server (almost certainly to reuse the wallet-ui
dist-serving code — see §2), and the origin list was never updated. The config
is a fossil of the previous architecture.

---

## 2. Correction to my earlier verbal advice

I initially suggested "load the window from bundled assets so the origin becomes
`tauri.localhost`". **Having read `tauri.conf.json`, I'm withdrawing that as a
near-term recommendation.**

`build.frontendDist` points at `./wallet-ui/dist` — a *different* SPA. This app
ships **two** frontends (wallet-ui and tournament-admin) but Tauri's
`frontendDist` addresses one directory. Serving the second over the embedded
axum server was a *reasonable* workaround for that constraint, not sloppiness.
Undoing it means merging dist trees or writing a custom URI scheme handler —
real work, real risk, and it does **not** by itself fix CORS (`tauri://localhost`
→ `http://127.0.0.1:8091` is still cross-origin).

There's also a trap: `tauri.conf.json`'s CSP `connect-src` currently allows
`:7454` and `:8090` but **not** `:8091`. That CSP doesn't apply today (it isn't
injected into pages served by the embedded HTTP server), but it *would* apply
the moment the window loads from the asset protocol — trading a CORS error for
a CSP error. Any future move must add `http://127.0.0.1:8091` there.

**Phase 1 below fixes CORS regardless of how the window is loaded, and makes
this whole question optional.** Do that first.

---

## 3. The plan

Three phases, each independently shippable. Phase 1 alone unblocks you
permanently.

### Phase 1 — Stop doing HTTP from the webview *(highest value, smallest diff)*

Replace the browser `fetch` with `@tauri-apps/plugin-http`'s `fetch`, which
routes the request through Rust.

**Why this is the fix:** requests issued from Rust have no browser origin, so
there is no preflight, no CORS, and no CSP `connect-src` check. The entire class
of bug (#4, #7, and the CSP trap in §2) stops being possible. `ALLOWED_ORIGINS`
becomes irrelevant to the panel — no server-side config has to track a
client-side port ever again.

Changes:
1. Add `tauri-plugin-http` to `tauri/Cargo.toml`; register with
   `.plugin(tauri_plugin_http::init())` in `main.rs` (next to the existing
   `tauri_plugin_shell::init()`).
2. Add `@tauri-apps/plugin-http` to `tauri/tournament-admin/package.json`.
3. Add an `http:default` permission scoped to the admin window, allowing
   `http://127.0.0.1:8090` and `http://127.0.0.1:8091` (new capability file, or
   extend `admin-shell.json`).
4. In `services/api.ts`: `import { fetch } from "@tauri-apps/plugin-http"` —
   the call signatures are drop-in compatible, so `request()` is otherwise
   untouched.
5. Same one-line swap in `services/tunnel.ts`'s `waitForHealth`.

**Acceptance:** PRODUCTION login succeeds with `ALLOWED_ORIGINS` on the server
left *unchanged* (still missing `7454`). That proves CORS is genuinely out of
the path rather than incidentally satisfied.

**Risk:** Low. Isolated to two files plus config. Reversible.

---

### Phase 2 — Move tunnel ownership into Rust *(fixes the process-lifecycle class)*

The SSH child process should be owned by the Rust side, which can tie it to app
lifetime.

Changes:
1. `#[tauri::command] async fn ensure_admin_tunnel() -> Result<(), String>` in
   `main.rs`. It:
   - kills any tunnel this app previously spawned (tracked in app state),
   - **pre-flights the port**: if 8091 is already bound by a *foreign* process,
     return a clear error naming the PID instead of silently failing (this is #6),
   - spawns `ssh` via `tauri_plugin_shell`,
   - polls `/health` **from Rust** (`reqwest` is already a dependency) until it
     answers or a generous deadline elapses — no CORS, no fixed guess,
   - stores the `CommandChild` in `tauri::State`.
2. `#[tauri::command] fn kill_admin_tunnel()`, plus a `WindowEvent::Destroyed`
   handler and an app-exit hook that both call it. **This is what makes orphans
   structurally impossible** rather than something you clean up by hand.
3. `services/tunnel.ts` collapses to `invoke("ensure_admin_tunnel")` /
   `invoke("kill_admin_tunnel")`. Delete the `Command`/`Child` handling, the
   `waitForHealth` loop, and the timeout constant.
4. Errors from Rust propagate as real messages — "port 8091 held by PID 1234",
   "ssh exited: Permission denied (publickey)" — instead of one generic string.

**Acceptance:** Close the admin window mid-session, reopen, log in — works first
try. `tasklist | grep ssh.exe` shows zero leftovers after closing the app.

**Risk:** Low–medium. Touches `main.rs`, but the surface is small and additive.

---

### Phase 3 — Delete the tunnel entirely *(optional; the actually-idiomatic answer)*

Install Tailscale on the VPS and on your machine. Bind the admin API to the
tailnet interface. The panel then talks to a stable private address
(`http://xfchess-vps:8090`) with no port forwarding, no child process, no
lifecycle management, no race conditions.

This deletes more code than it adds: all of `tunnel.ts`, the Phase 2 commands,
the `tunnel` SSH user, the `Match User tunnel` sshd block, and the `PermitOpen`
rules. It also works from a second machine with zero extra setup, which the SSH
tunnel does not.

I'd hold this until Phases 1–2 are in and you've had a few calm sessions. It's
an infra change, not a code change, and it deserves its own window.

---

## 4. What you need to do

**Right now, to be unblocked today** (independent of all the above — Phase 1
makes it unnecessary later, but you need to work *tonight*):

```
ssh -i C:/Users/isich/.ssh/xfchess_vps root@178.104.55.19 "sed -i 's/localhost:7455/localhost:7454,http:\/\/localhost:7455/' /opt/xfchess/.env && grep ALLOWED_ORIGINS /opt/xfchess/.env"
```

Confirm the printed line contains `localhost:7454`, then:

```
ssh -i C:/Users/isich/.ssh/xfchess_vps root@178.104.55.19 "systemctl restart xfchess-backend && sleep 2 && systemctl is-active xfchess-backend"
```

Then hit LOG ON. No rebuild needed — this is purely server-side.

**To let me work faster on the phases**, add these to
`.claude/settings.local.json` under `permissions.allow`:

```json
"Bash(cargo build *)",
"Bash(cargo check *)",
"Bash(npm run build *)",
"Bash(taskkill *)"
```

Tonight I lost the ability to compile, kill processes, or even run a read-only
JSON parse partway through. That's why each fix cost you a manual rebuild cycle
instead of being verified before it reached you. With those four rules I can
build and sanity-check locally before handing anything over.

**Decisions I need from you:**

1. **Phase 1 now?** (my recommendation: yes — it's the small one that kills the
   whole CORS class)
2. **Phase 2 in the same pass, or separately?**
3. **Phase 3 — interested, or leave the SSH tunnel alone?**

**Nothing here requires the Hetzner console.** Phases 1–2 are entirely local
code. Phase 3 would need Tailscale installed on the VPS (one command, plus a
login).

---

## 5. Already fixed tonight (no action needed)

- `justfile`: `admin` recipe now calls `build-admin-ui-force`, so it can never
  again serve a stale bundle after a source edit (cause #1).
- `capabilities/admin-shell.json`: added the required `name` field to each
  scoped command entry (cause #3), and widened the trusted origin to
  `http://localhost:*/*` (cause #4).
- `services/tunnel.ts`: health-check timeout 8s → 20s (cause #5). *Phase 2
  deletes this line entirely — a fixed timeout is the wrong shape of fix.*
- `hooks/useAuth.tsx`: the bare `catch { return false }` that collapsed every
  failure into one indistinguishable message now captures the real error and
  surfaces it in the UI and console. **This is what made #7 findable at all** —
  without it, the CORS failure was invisible behind "Tunnel down, or bad token".

---

## 6. Note on the credential

The production `ADMIN_API_KEY` was rotated during this session and is currently:

```
40693cd34830138b2c2de96acdc88c1787c57e82ab40b29a869174eef1b0bcfc
```

It was never the cause of any failure here — the backend accepted it throughout.
Both it and the prior key have appeared in plaintext in an AI chat transcript;
if that transcript is retained anywhere outside your control, rotate again
before mainnet:

```
ssh -i C:/Users/isich/.ssh/xfchess_vps root@178.104.55.19 'NEWKEY=$(openssl rand -hex 32) && sed -i "s/^ADMIN_API_KEY=.*/ADMIN_API_KEY=$NEWKEY/" /opt/xfchess/.env && systemctl restart xfchess-backend && echo $NEWKEY'
```

This is separate from, and additional to, the pre-mainnet key rotation already
tracked for the exposed devnet authority keys.
