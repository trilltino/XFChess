# Plan: per-user auth on move endpoints + program-ID consolidation

Two follow-ups from the 2026-06-13 hardening pass. They are independent — do them in
either order — but the program-ID cleanup (Part B) is lower-effort and higher-urgency
(a wrong ID means the backend talks to the wrong program), so it is listed first as the
recommended starting point despite "Part B".

---

## Part A — Replace the relay shared-secret with per-user JWT auth on the signing endpoints

### Why
The relay shared-secret (`X-Relay-Secret`, added 2026-06-13) is defense-in-depth, not
auth:
- The prod desktop client posts to `http://178.104.55.19/move/record` over the public
  internet, so these endpoints are **not** localhost-only.
- A desktop client ships the secret embedded; anyone running the client can extract it,
  so it cannot distinguish one user from another.

The signing endpoints sign Solana transactions with **server-held session keys** keyed
only by `game_id`. On-chain turn/nonce/`session_delegation` checks stop fund theft, but
without per-caller auth a holder of the relay secret can still drive moves/finalize for
games they are not part of (griefing, forced moves, RPC-cost abuse).

### Goal
Require a valid per-user JWT on the session-key signing endpoints, and authorize the
caller against the specific `game_id` (must be a participant).

### Current state (grounded)
- Protected endpoints live in `backend/src/signing/routes/main.rs::protected_routes()`
  (`/session/create`, `/session/activate`, `/session/sign`, `/session/tee_auth`,
  `/move/record`, `/game/undelegate`, `/game/finalize`), mounted with the relay-secret
  layer in `backend/src/signing/mod.rs`.
- The client already obtains a JWT: `src/states/main_menu.rs` fetches it from the wallet
  bridge `GET /token` then calls `GET /api/auth/me` (see `main_menu.rs:719`). So a token
  exists client-side; it is simply not attached to the VPS calls.
- The shared VPS HTTP client is `src/multiplayer/network/vps/client.rs::client()` — the
  single place to attach an `Authorization: Bearer` header.
- `authed_wallet()` in `backend/src/signing/routes/auth.rs` already does
  verify-JWT + revocation-check and returns the wallet; reuse this pattern.
- The session store (`backend/src/signing/storage/session.rs`) maps `game_id → wallet`
  (`entry.wallet_pubkey`), used today in `record_move`.

### Steps

1. **Backend: a JWT extractor usable in middleware.**
   Promote the verify+revocation logic from `routes::auth::authed_wallet` into a shared
   helper (e.g. `signing::auth::authenticate(state, headers) -> Result<Claims, _>`), so
   both the auth routes and a new middleware can call it. Keep the existing behaviour
   (signature, expiry, `jwt_revocations` cut-off).

2. **Backend: replace the relay-secret layer with a JWT layer on `protected_routes()`.**
   In `signing/mod.rs`, swap `require_relay_secret` for a `require_jwt` middleware that
   401s when the Bearer token is missing/invalid/revoked. This covers authentication.

3. **Backend: per-`game_id` authorization (the important part).**
   Authentication alone is not enough — verify the caller is a participant of the game in
   the request body. Two options:
   - **Cheap/local:** check `claims.sub == entry.wallet_pubkey` from the session store.
     Limitation: the store currently records one wallet per `game_id`; confirm whether
     both players each have a session entry, or extend the store to record `{white, black}`.
   - **Authoritative:** read the on-chain `Game` account (`white`/`black`) and require
     `claims.sub ∈ {white, black}`. Costs one RPC per call; cache per `game_id`.
   Implement this as an explicit check at the top of `record_move`, `sign_tx`,
   `finalize_game`, `undelegate_game`, `create_session`, `activate_session`,
   `tee_auth` — not in the middleware (the middleware lacks the parsed body / `game_id`).

4. **Client: attach the Bearer token.**
   In `src/multiplayer/network/vps/client.rs::client()`, add
   `Authorization: Bearer <jwt>`. The token must be available where `client()` is called;
   thread it through (e.g. store the bridge JWT in a resource/`OnceCell` after the
   `main_menu.rs` fetch, and have `client()` read it). Handle token refresh/expiry — on a
   401, re-fetch via the SIWS/bridge flow and retry once.

5. **Migration / rollout (coordinated — this is the breaking part).**
   - Ship backend that accepts **both** the relay secret **and** a JWT for one release
     (dual-accept), so old clients keep working.
   - Ship the client that sends the JWT.
   - Once telemetry shows old clients drained, drop the relay-secret path and make JWT
     mandatory. Keep `RELAY_SHARED_SECRET` as an optional outer layer if desired.

6. **Tests.** Extend `backend/tests/e2e_api.rs`:
   - `/move/record` without a Bearer → 401.
   - With a valid JWT for a non-participant wallet → 403.
   - With a valid JWT for a participant → passes the guard.
   Mirror the existing `relay_secret_guards_signing_endpoints` test structure.

### Risks / notes
- Step 4 is the coordinated client change that made this "deferred" originally; do not
  make JWT mandatory before the dual-accept window (step 5) or you brick live clients.
- The session keypair model is unchanged — this only gates *who may ask the server to
  sign*, it does not move signing to the client.

---

## Part B — Consolidate the program ID to a single source of truth

### Why
The deployed devnet program is `8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU`
(`declare_id!` + `Anchor.toml`), but the repo references at least four different IDs.
If the backend runs with the wrong one it builds instructions for a non-existent/old
program and every on-chain call fails.

### The IDs found (grounded)
| ID | Where | Status |
|----|-------|--------|
| `8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU` | `programs/xfchess-game/src/lib.rs` `declare_id!`, `Anchor.toml`, `scripts/run_offline*.bat`, `scripts/dev8.bat`, `web-solana/src/lib/magicblock.ts`, `src/solana/constants.rs` | **CANONICAL (live, deployed)** |
| `AhkTK5LVJHvR51gmDXbsJsqq4wg381AH6vTiaFGGJPWm` | `backend/src/signing/config.rs:78` (env fallback default) | stale — wrong default |
| `C624Z53FYEVDYVkMWSQ1KPQm4o1Jmdhpc5movSSBnezf` | `backend/.env.example:36` | stale — wrong template |
| `A5HtSnmyTPohayj9633D9queFFmL2ep6u45nv1v4Wj3W` | `backend/.env:14` (the **actual running config**), `backend/src/signing/blinks/pda.rs:75` (hardcoded) | ⚠️ **live mismatch** — verify what the prod backend is actually using |

> First action item: confirm which ID the **running** backend uses (`backend/.env` =
> `A5HtSn…`). If prod is on `A5HtSn…`, then either that is a different deployed program
> and `8tevg…` is wrong, or the backend has been mis-pointed. Resolve which program is
> authoritative **before** changing code — the deploy I did targeted `8tevg…`.

### Steps

1. **Decide the canonical ID.** Almost certainly `8tevg…` (it is `declare_id!` and what
   was just upgraded). Confirm `A5HtSn…` is dead (check `solana program show A5HtSn…` on
   devnet) and that no live data lives under it.

2. **Single source of truth in the backend.** Make `config.rs` either:
   - require `PROGRAM_ID` (no silent default — `expect`, like `JWT_SECRET`), or
   - default to the canonical `8tevg…`.
   Remove the `AhkTK…` fallback at `config.rs:78`.

3. **Remove hardcoded IDs.** `backend/src/signing/blinks/pda.rs:75` hardcodes `A5HtSn…` —
   replace with `state.config.program_id`. Grep for any other `Pubkey::from_str("…")`
   literals that bake in an ID.

4. **Fix templates.** Set `backend/.env.example` (`C624Z…`) and
   `deploy/backend/.env.example` (`YOUR_DEPLOYED_PROGRAM_ID`) to the canonical ID, and
   update the live `backend/.env` (`A5HtSn…`) after step 1's decision.

5. **Sweep the rest.** Reconcile the benchmark/check binaries under
   `crates/solana/er-cu-benchmark/` and `src/bin/*` (`pda.rs`, `profile_pda.rs`,
   `read_game.rs`, `tournament_test.rs`) — these are dev tools but will mislead. Prefer
   reading from one shared constant (`src/solana/constants.rs` already holds `8tevg…`;
   have tools import it instead of re-declaring).

6. **Guard against regression.** Add a tiny test/CI check that the backend default (or a
   committed constant) equals `declare_id!` so the IDs can't silently diverge again.

### Verification
- `cargo test -p backend` + `backend/tests/e2e_api.rs` still green.
- Boot the backend with the corrected env and hit a read path that derives a PDA
  (e.g. `/player/:pubkey`) — confirm it resolves against the live program.
- `solana program show <canonical>` matches what the backend logs at startup.

---

## Suggested sequencing
1. **Part B steps 1–4** first (low-risk config/doc cleanup; fixes a possible live break).
2. **Part B steps 5–6** (sweep + regression guard).
3. **Part A** (the coordinated backend+client auth change) as a tracked feature with the
   dual-accept rollout window.
