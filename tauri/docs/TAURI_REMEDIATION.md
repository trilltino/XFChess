# Tauri Desktop Remediation — End-to-End Plan

Fixes from the Tauri audit of the desktop wrapper (Tauri 2.10, Privy wallet).
Ordered by severity. `[auto]` = applied by this pass · `[you]` = needs your
input/keys/testing.

- [ ] **T1** 🔴 Wallet bridge (:7454) leaks JWT to any website `[auto]` (partial) / `[you]` (full)
- [ ] **T2** 🟠 Shell caps granted to the wallet window `[auto]`
- [ ] **T3** 🟠 Unscoped shell perms + unanchored validator `[auto]`
- [ ] **T4** 🟡 `open_url` → arbitrary ShellExecute `[auto]`
- [ ] **T5** 🟡 No signed auto-updater `[you]`
- [ ] **T6** 🟡 CSP `'unsafe-inline'` + `withGlobalTauri` `[you]`
- [ ] **T7** 🔵 `.unwrap()` panics in IPC commands `[auto]`
- [ ] **T8** 🔵 Dead weak `hash_password` `[auto]`
- [ ] **T9** 🔵 Fragile `deploy.bat` sidecar `[you]`

---

## T1 🔴 Local wallet bridge leaks the auth JWT cross-origin

**Problem:** `GET http://localhost:7454/token` returns the wallet JWT
([main.rs:304](../src/main.rs#L304)) and the bridge uses `CorsLayer::allow_origin(Any)`
([main.rs:539](../src/main.rs#L539)) with no Origin check. Any website open in any
browser can read the token cross-origin → account takeover.

**Fix `[auto]` (this pass):** replace `allow_origin(Any)` with a predicate that only
reflects **local/tauri origins**, blocking arbitrary internet sites from reading
responses:
```rust
.allow_origin(AllowOrigin::predicate(|origin, _parts| {
    let o = origin.as_bytes();
    o.starts_with(b"tauri://")
        || o.starts_with(b"http://tauri.localhost")
        || o.starts_with(b"https://tauri.localhost")
        || o.starts_with(b"http://localhost:")
        || o.starts_with(b"http://127.0.0.1:")
}))
```
This keeps the wallet-ui (dev `localhost:5174`, prod `tauri.localhost`) working while
closing the "any site steals the token" hole.

> **Tradeoff:** the website's localhost game-launch (`Play.tsx` → `:7454`) is blocked
> by this — but that call is already mixed-content-blocked over HTTPS; the real launch
> path is the `xfchess://` deep link, which doesn't go through CORS.

**Fix `[you]` (full hardening, follow-up):**
- Generate a **random per-launch bearer token** in Rust, inject it into the wallet-ui
  window (e.g. via an init script / Tauri command), and require it on every bridge
  request; reject requests without it.
- Add a **Host-header check** (reject non-`localhost`/`127.0.0.1` Host) to stop DNS
  rebinding.
- Consider not exposing `/token` over HTTP at all — hand the token to the wallet-ui via
  a Tauri command instead.

**Verify:** from a normal browser tab, `fetch('http://localhost:7454/token')` fails CORS;
the desktop wallet still logs in and signs.

---

## T2 🟠 Shell capability granted to the wallet window

**Problem:** [capabilities/default.json](../capabilities/default.json) grants
`shell:allow-execute/spawn/open` to `["main","tournament-admin","wallet-popup"]`. The
wallet window loads remote Privy scripts, so XSS/CDN-compromise → RCE.

**Fix `[auto]`:** strip all shell perms from `default.json` (leaving `core:default` for
all windows) and move shell into a new **[capabilities/tournament-admin.json](../capabilities/tournament-admin.json)**
scoped to `windows: ["tournament-admin"]` only.

**Verify:** the tournament-admin terminal still runs; the wallet window can no longer
invoke shell (calls rejected by the capability system).

---

## T3 🟠 Unscoped shell perms + unanchored validator

**Problem:** the bare strings `"shell:allow-execute"` / `"shell:allow-spawn"` grant the
commands with **no scope** (any command runs), and the powershell validator
`ssh root@\d{1,3}...` is **unanchored** (`ssh root@1.2.3.4; evil` matches).

**Fix `[auto]`:** in the new admin capability, drop the bare strings and allow only:
- `ssh` with any args (the remote terminal's purpose — affects the VPS, not the local box)
- `powershell` with an **anchored** validator: `^ssh root@\d{1,3}(\.\d{1,3}){3}$`

**Verify:** admin `ssh` quick-actions work; a crafted `powershell -Command "ssh root@1.2.3.4; calc"` is rejected.

---

## T4 🟡 `open_url` opens arbitrary things

**Problem:** [ipc.rs:110](../src/services/ipc.rs#L110) `open::that(url)` on any string —
opens files/exes/protocol handlers.

**Fix `[auto]`:** allowlist schemes (`http`/`https`/`mailto`/`xfchess`) before opening.

**Verify:** `open_url("https://x.com")` works; `open_url("C:/Windows/System32/calc.exe")` is blocked + logged.

---

## T5 🟡 No signed auto-updater `[you]`
No `updater` plugin/pubkey/endpoints in [tauri.conf.json](../tauri.conf.json). Add
`tauri-plugin-updater` with a signing keypair + release endpoint so a wallet app can be
patched. Ties into your code-signing/distribution work.

## T6 🟡 CSP `'unsafe-inline'` + `withGlobalTauri` `[you]`
Move to nonce/hash script-src if Privy permits, and set `withGlobalTauri: false` unless
a UI needs the global (expose only required commands). Reduces blast radius of any injection.

---

## T7 🔵 `.unwrap()` in IPC commands
**Problem:** window/notification/clipboard commands `.unwrap()` ([ipc.rs](../src/services/ipc.rs)) —
frontend-triggerable panics.
**Fix `[auto]`:** handle errors (log + no-op) instead of unwrapping.

## T8 🔵 Dead weak `hash_password`
**Problem:** single-round SHA-256, unused ([crypto.rs:26](../src/utils/crypto.rs#L26)).
**Fix `[auto]`:** remove it (and its tests) so it can't be adopted later.

## T9 🔵 Fragile deploy sidecar `[you]`
`Command.sidecar("../deploy/scripts/deploy.bat")` ([Dashboard.tsx](../tournament-admin/src/components/Dashboard.tsx#L137))
won't resolve in a bundle and ships a one-click prod-deploy. Register it properly via
`externalBin` or remove it from the shipped admin app.

---

## Applied this pass (`[auto]`): T1 (partial), T2, T3, T4, T7, T8.
Deferred (`[you]`): T1 (bearer-token hardening), T5 (updater), T6 (CSP/global), T9 (sidecar).

**Verify build:** `cargo check -p xfchess-tauri`.
