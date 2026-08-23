# Social-login (Google) embedded wallets — end-to-end plan

**Status:** Phases 0–4 implemented and deployed to devnet 2026-08-23.
**Drafted:** 2026-08-22

> ## Implementation status (2026-08-23)
>
> | Phase | State |
> |---|---|
> | 0 · De-risk | ✅ done — clean `npm install` + build on both frontends, no `--legacy-peer-deps` |
> | 1 · Website | ✅ done — social login, Wallet Standard registration, lazy SDK load |
> | 2 · Backend identity | ✅ done — migration 030, `POST /auth/privy-login`, JWKS ES256 verifier, `/auth/me` fields |
> | 3 · Desktop popup | ✅ done — `WalletSource` refactor, social block, Privy signing branch, MIME arms |
> | 4 · No-popup gameplay | ✅ done — program upgraded on devnet, flow enabled **for embedded wallets only** |
> | 5 · Recovery / export / funding | ⬜ not started |
> | 6 · Mainnet cutover | ⬜ not started |
>
> **The feature is complete and usable without Phase 4.** A Google user can sign
> up, receive a Solana wallet, authenticate, and sign transactions on both
> surfaces today. Phase 4 only removes the per-transaction popup — exactly the
> degradation §8.4 anticipated.
>
> ### Verification actually run
>
> | Check | Result |
> |---|---|
> | `xfchessdotcom` clean install + build | ✅ no peer conflict |
> | `xfchessdotcom` eslint | ✅ clean |
> | Eager JS payload | 262 KB → **269 KB** gzip (+7 KB); the 1.1 MB SDK loads lazily |
> | Build with `VITE_PRIVY_APP_ID` unset | ✅ kill switch works, nothing renders |
> | `wallet-ui` build + `vitest` | ✅ 33/33 |
> | `cargo build -p backend` | ✅ (2 pre-existing warnings) |
> | `cargo test -p backend --lib` | ✅ 225/225, incl. 7 new Privy tests |
> | `cargo check -p xfchess-tauri` | ✅ |
> | `cargo check --features solana` | ✅ |
> | `cargo build-sbf` | ✅ |
> | `cargo test -p xfchess-game` | 136 passed, 1 failed — `claim_timeout_mutates_only_game_even_when_delegated`, **confirmed pre-existing** by re-running with the change stashed |
>
> > ### Live browser run (2026-08-23)
>
> Driven with a real Chrome session against `npm run dev` on `:5173`.
>
> | Step | Result |
> |---|---|
> | App loads, Privy SDK lazy-loads | ✅ |
> | `[privy] registered embedded wallet with Wallet Standard` | ✅ fires at runtime |
> | Privy row appears in the picker beside Phantom/Solflare | ✅ `Privy, Phantom, Solflare` |
> | Privy iframe loads (`auth.privy.io/apps/<id>/embedded-wallets`) | ✅ origin accepted |
> | "Continue with Google" opens Privy's modal | ✅ "Log in or sign up / Google" |
> | Google button → real OAuth | ✅ `accounts.google.com/v3/signin`, `redirect_uri=https://auth.privy.io/api/v1/oauth/callback` |
> | Entering Google credentials | ⛔ needs the account owner |
>
> **Bug found and fixed by this run.** The Wallet-Standard registration ran
> **three times** and produced two "Continue with Google" buttons. The guard was
> a `useRef`, which is per-component-instance — and this component remounts at
> least twice per load (the lazy-SDK provider swap, plus StrictMode's
> double-invoked effects), handing the effect a fresh empty Set each time.
> Registration is a global operation against a `window` event bus, so the
> bookkeeping had to be module-scoped. Re-verified after the fix: 1 registration,
> 1 button, 1 Privy row.
>
> ### Privy dashboard config — applied via API
>
> The dashboard settings turned out to be writable through the REST API:
> `GET /v1/apps/{id}` reads them and **`POST`** (not `PATCH`/`PUT`, both 405)
> writes them, authenticated with app-id/app-secret basic auth.
>
> Three gaps were found, not just the expected origins:
>
> | Setting | Before | After |
> |---|---|---|
> | `allowed_domains` | `xfchess.com`, `www.xfchess.com` only | + `localhost:5173/5174` and `7454`–`7464` (15 total) |
> | `google_oauth` | **`false`** — Google login was not enabled at all | `true` |
| `email_auth` | `true` | `false` — Google is the only social method by design |
> | `embedded_wallet_config.solana.create_on_login` | **`"off"`** — no wallet would ever have been created | `"users-without-wallets"` |
>
> `user_owned_recovery_options` was already `["user-passcode"]`, satisfying D5's
> recovery requirement. Either of the two bolded values would have made the
> feature silently do nothing even with perfect code and correct origins.
>
### Devnet deploy (Phase 4)
>
> | | |
> |---|---|
> | Extended ProgramData by | 32,768 bytes (new `.so` was 3,344 bytes over the old allocation) |
> | Upgrade signature | `25ZoXVbwUx9Z4HB13GLDpqEKsMXmL7oE2vucgfYTqq6BahLJgXs7cH1xzVcxmFJAuxpBcH9xaGXqo67GV1AcDAzi` |
> | Slot | 484489210 → **486887670** |
> | Data length | 1,291,296 → 1,324,064 bytes |
> | Authority | `C1vn2MT7tZotZPjUJQDf9oo3dpZZ2tr7NxYLg8jTYgkw` (unchanged) |
>
> **The no-popup flow is enabled for embedded wallets only.** Blocker (2) — the
> Solflare cluster-mismatch rejection — is still unresolved for extension
> wallets, and enabling it for them would knowingly reintroduce that regression.
> The gate is `SolanaIntegrationState::wallet_is_embedded`, fed from a new
> `provider` field that wallet-ui reports on `POST /wallet` and the bridge
> surfaces on `GET /status`. It defaults to `false`, so an older bridge that does
> not report the field leaves the flow off.
>
> ### Not done, and why
>
> - **Live Google sign-in completion** — the flow was driven end to end in a real
>   browser and reaches Google's OAuth consent screen (see below). Entering
>   Google credentials needs the account owner, so wallet creation, the
>   `POST /auth/privy-login` round-trip and the on-chain profile step remain
>   unexercised.
> - **Extension wallets on the no-popup path** — needs blocker (2) root-caused,
>   which requires reproducing with Solflare on a non-devnet cluster.
> - **Phases 5–6** — untouched.
>
> ### IDL
>
> Regenerated from the deployed program and written to both the Anchor-canonical
> path and the one this repo's convention names:
>
> | Path | Note |
> |---|---|
> | `target/idl/xfchess_game.json` | Anchor default; under `target/`, so gitignored |
> | `xfchessdotcom/src/lib/xfchess_game.json` | the documented copy — committable |
>
> | | |
> |---|---|
> | `address` | `8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU` — matches the deployed program |
> | instructions / accounts / types / errors | 66 / 14 / 35 / 104 |
> | New error | `GlobalSessionVaultUnderfunded` at **6103** |
>
> That code confirms the append-only discipline worked: the variant landed last,
> so every pre-existing code — including the `6060` the original bug report
> cites — kept its number. Had it been inserted mid-enum, every subsequent error
> would have silently renumbered and any client matching on a numeric code would
> now be reporting the wrong error.
>
> Note the web app does not yet *consume* this file: `anchor_client.ts` and all
> `@coral-xyz/anchor` usage are absent in the current mid-refactor tree. The IDL
> is placed and current so the refactor has it when transaction building returns;
> re-run `anchor idl build --out …` whenever program instructions change.
**Goal:** a player who has never touched crypto signs in with Google, gets a real
Solana wallet, and can play — on `xfchess.com`, in the desktop wallet popup, and
inside the Bevy game — without ever seeing a seed phrase or installing an extension.

---

## 0 · TL;DR

| Decision | Choice | Why |
|---|---|---|
| Provider | **Privy** (`@privy-io/react-auth` v3.37.x) | Only provider giving Solana embedded wallets **+** Google auth **+** a Wallet-Standard surface **+** fiat funding in one SDK; the repo is already half-configured for it |
| Website | Register Privy's embedded wallet as a **Wallet Standard** wallet | `useWallet()`, `WalletSelectionModal`, `anchor_client.ts`, every page — all keep working unchanged |
| Desktop | New "Continue with Google" block in `wallet-ui`'s `WalletStep` | Same popup, same bridge, same `POST /wallet` handoff to the game |
| In-game | **One** `authorize_global_session` tx, then the existing on-chain session key | No Privy call per move, no popup per move, no delegated custody |
| Backend | New `POST /auth/privy-login` verifying Privy's ES256 JWT via JWKS | `jsonwebtoken 10.3` is already a dependency and does ES256 |
| Custody | **Self-custodial only.** No Privy delegated actions, no server-side signing of user wallets | A wagering platform that can move player funds is a custodian; see §11 |

The single most important architectural claim in this document:

> **The embedded wallet should sign exactly two things, ever.**
> (1) a login message, and (2) one `authorize_global_session` transaction.
> Everything else in the game is signed by the bounded session key that already
> exists in `programs/xfchess-game/src/account_ix/global_session_ix.rs`.

Everything below follows from that.

---

## 1 · Why this failed last time, and what actually changed

This is not the first attempt.

- `a3c1c5dfd feat(web): Privy auth integration, waitlist page, homepage/nav cleanup`
  added `web-solana/src/privy/{config.ts,PrivyProviderWrapper.tsx,PrivyAuthButton.tsx}`.
- `c690b4ca3 fix(web-solana): remove Privy entirely` (2026-07-21) ripped it out:

  > `@privy-io/react-auth` was the sole source of the `@solana/kit` v6-vs-v7 peer
  > conflict breaking the release build … Removing it drops `@solana/kit` and
  > `@solana-program/*` … along with ~750 packages.

So Privy was removed for a **dependency-resolution** reason, not a product one.
Residue is still in the tree — treat it as already-paid-for scaffolding:

| Leftover | Location | Still valid? |
|---|---|---|
| `VITE_PRIVY_APP_ID` | [tauri/wallet-ui/.env.example](../../tauri/wallet-ui/.env.example) | yes |
| CSP allowing `auth.privy.io`, `*.privy.io`, `*.privy.systems`, `frame-src` | [tauri/tauri.conf.json:17-24](../../tauri/tauri.conf.json#L17-L24) | yes |
| "connect a wallet (extension or Privy)" | [tauri/wallet-ui/README.md](../../tauri/wallet-ui/README.md) | aspirational, no code |
| "`src/privy/` — Privy embedded-wallet provider + auth button" | [xfchessdotcom/README.md:34](../../xfchessdotcom/README.md#L34) | **stale — directory does not exist** |
| "USDC-based via the desktop wallet's Privy integration" | [backend/src/signing/blinks/funding.rs:3](../../backend/src/signing/blinks/funding.rs#L3) | aspirational |

### The dependency conflict is gone — verified, not assumed

Checked against the live npm registry on 2026-08-22:

```
@privy-io/react-auth@3.37.4
  peerDependencies:
    react            ^18 || ^19
    @solana/kit      >=3.0.3                       (optional)
    @solana-program/{memo,token,system}            (optional)
```

Every Solana peer is now **optional** with an **open-ended `>=` range**. The July
break was a narrow pinned range colliding with Anchor's. Meanwhile:

```
xfchessdotcom/node_modules/@solana/kit → 6.10.0   (pulled in by @coral-xyz/anchor ^0.32.1)
@solana/wallet-adapter-react@0.15.39  peer: @solana/web3.js ^1.98.0
```

`@solana/kit@6.10.0` already satisfies `>=3.0.3`, and it is **already installed**.
`@solana/kit` and `@solana/web3.js` are different packages and coexist fine — the
app keeps using web3.js v1, Privy uses kit internally.

**Phase 0 ran this and it passed** — `npm install` with no `--legacy-peer-deps`,
then a successful build:

```bash
cd xfchessdotcom && npm install @privy-io/react-auth \
  && VITE_BACKEND_URL=https://xfchess.com npm run build
```

### One conflict did surface, and it is worth writing down

Privy's Solana peers are optional, so npm installs happily without them — but
the **bundler** then fails, because Privy's code imports symbols the Vite
optional-peer shim cannot provide:

```
[MISSING_EXPORT] "getAddMemoInstruction" is not exported by
  "__vite-optional-peer-dep:@solana-program/memo:@privy-io/react-auth"
```

The fix is to install those three peers explicitly — but at matching versions,
because **the `@solana-program/*` family is not internally consistent**. At
0.13.0, `memo` and `system` want `@solana/kit ^8` while `token` still wants `^6.5`,
so `npm install @solana-program/memo @solana-program/system @solana-program/token`
fails outright with ERESOLVE. Pin to a set that agrees with whatever kit line
that workspace is already on:

| Workspace | kit | memo | system | token |
|---|---|---|---|---|
| `xfchessdotcom` (kit 6.10.0, from Anchor 0.32) | ^6 | `0.11.2` | `0.12.2` | `0.14.0` |
| `tauri/wallet-ui` (kit 8.0.0, installed fresh) | ^8 | `0.13.0` | `0.14.0` | `0.16.0` |

All are pinned with `--save-exact`. If you ever bump `@solana/kit` in either
workspace, these three must move with it as a set.

---

## 2 · Provider decision

### Requirements, in priority order

1. **Solana** embedded wallets (not EVM-first with Solana bolted on).
2. **Google** login producing a wallet with no seed phrase shown.
3. Works in a **plain Chrome window served from `http://localhost:<port>`** —
   because that is literally what the desktop popup is
   ([tauri/src/main.rs:1163-1176](../../tauri/src/main.rs#L1163-L1176) spawns
   Chrome with `--app=http://localhost:7454/wallet-ui/`).
4. Exposes a **byte-level signing interface**, so it can sign transactions our
   Rust code built without us re-modelling them in TypeScript.
5. **Self-custodial** — the user, not XFChess, controls the key.
6. **Key export** — a player can walk away with their private key.
7. Fiat funding, ideally in-SDK (ties into `docs/STRIPE_PRIVY_USDC_INTEGRATION.md`).

### Comparison

| | Privy | Turnkey | Web3Auth (Consensys) | Dynamic (Fireblocks) | Para |
|---|---|---|---|---|---|
| Solana embedded | ✅ first-class | ✅ raw signer | ✅ | ✅ | ✅ |
| Google login | ✅ | ⚠️ bring your own auth | ✅ | ✅ | ✅ |
| Wallet-Standard registration | ✅ documented recipe | ❌ build it | ⚠️ | ✅ | ⚠️ |
| Key model | TEE, self-custodial | TEE, self-custodial | MPC shares | MPC/TEE | MPC |
| Signing latency | ~175 ms | <150 ms | ~500 ms+ | — | — |
| Key export | ✅ | ✅ | ✅ | ✅ | ✅ |
| Fiat onramp in-SDK | ✅ Stripe / Coinbase / MoonPay | ❌ | ⚠️ | ✅ | ⚠️ |
| Free tier | ~<500 MAU | per-signature | cheapest at scale | — | — |
| Owned by | **Stripe** | independent | Consensys | Fireblocks | independent |

**Choose Privy.** It is the only one satisfying 1–7 without custom work, it
publishes a recipe for the exact Wallet-Standard trick this plan depends on, and
the repo already carries its CSP entries and env scaffolding.

Two risks to accept explicitly rather than discover later:

- **R1 — Stripe ownership.** Privy is now a Stripe company. This project already
  established that **Stripe's ToS bans skill-gaming** — that is why the fiat plan
  uses Coinbase Onramp. Privy's *wallet* product is not Stripe's *payments*
  product, but Privy's card onramp routes through Stripe in US/EU. **Before
  Phase 5, get written confirmation from Privy that a real-money skill-chess
  platform is in-policy for wallets, and treat their card onramp as unavailable
  until separately cleared.** Phases 0–4 do not depend on it either way.
- **R2 — vendor coupling.** Mitigated by design: per §5 the Privy wallet is only
  ever an *owner key*, and everything downstream keys off a plain base58 pubkey.
  Swapping providers means rewriting one React component per surface, not the game.

---

## 3 · Target UX

### 3.1 Website (`xfchess.com`)

`Connect Wallet` → the existing `WalletSelectionModal` gains a top section:

```
┌──────────────────────────────────────┐
│  Select Network Provider             │
├──────────────────────────────────────┤
│  New to crypto?                      │
│  ┌────────────────────────────────┐  │
│  │  G   Continue with Google      │  │  ← new
│  └────────────────────────────────┘  │
│  We create a Solana wallet for you.  │
│  You keep the keys.                  │
├──────────────────────────────────────┤
│  Already have a wallet?               │
│  • Phantom                            │  ← unchanged
│  • Solflare                           │
│  • Mobile Wallet Adapter              │
└──────────────────────────────────────┘
```

After Google auth the wallet lands in the same `wallets` array as Phantom, and
the rest of the site cannot tell the difference.

### 3.2 Desktop popup (`wallet-ui`)

`WalletStep` today shows only Phantom/Solflare, with an "Install" link when the
extension is missing
([tauri/wallet-ui/src/App.tsx:577-780](../../tauri/wallet-ui/src/App.tsx#L577-L780)).
For a non-crypto user *both* rows say "not installed" — today's dead end. New:

```
   Sign In                                    (Card + step dots unchanged)

   ┌────────────────────────────────────┐
   │   G   Continue with Google         │     ← new, primary
   └────────────────────────────────────┘

   ──────────  or use a wallet  ──────────

   [ Phantom  ]                               ← unchanged
   [ Solflare ]
```

### 3.3 In-game (Bevy)

Unchanged from the player's point of view, and that is the point. `Connect
Wallet` still calls `open_wallet_browser()`, the popup still `POST /wallet`s a
pubkey, the game still learns it from `GET /status`. The only visible difference
for a Google user is **the popup stops appearing after onboarding**, because the
session key covers gameplay (§8).

---

## 4 · What exists today (the integration surface)

Read this before touching anything; the plan is written against these seams.

### 4.1 Desktop bridge topology

```
xfchess.exe (Bevy)                   xfchess-tauri.exe                Chrome --app window
──────────────────                   ─────────────────                ───────────────────
open_wallet_browser() ─ TCP "OPEN" ──▶ open_wallet_popup() ── spawn ──▶ localhost:7454/wallet-ui/
                                                                              │
poll_wallet_bridge() ◀─ GET /status ── wallet_pubkey: Mutex<Option<String>> ◀──┘ POST /wallet

sign_via_tauri_only() ─ TCP len+label+tx ──▶ pending: Mutex<PendingTx>
                                                   │  SSE
                                                   ▼
                                            GET /pending/stream ──▶ TransactionSigner
                     ◀── len + signed bytes ── POST /resolved ◀────────┘
```

| Fact | Where |
|---|---|
| Popup URL is `http://localhost:{7454..7464}/wallet-ui/?sid=…` | [tauri/src/main.rs:1163-1176](../../tauri/src/main.rs#L1163-L1176) |
| HTTP port is **dynamic** — nominal 7454, scans +10 | [tauri/src/main.rs:134-152](../../tauri/src/main.rs#L134-L152) |
| Popup is a real Chrome process (`--app=`) on the default profile, **not** a Tauri WebView | [tauri/src/main.rs:1290-1300](../../tauri/src/main.rs#L1290-L1300) |
| Bridge CORS reflects only `tauri://`, `tauri.localhost`, `localhost:*`, `127.0.0.1:*` | [tauri/src/main.rs:918-930](../../tauri/src/main.rs#L918-L930) |
| Bridge proxies `/api/auth/*` to the backend the **game** resolved | `sync_backend_url_to_bridge()` in [src/multiplayer/solana/tauri_signer.rs](../../src/multiplayer/solana/tauri_signer.rs) |
| Signing is byte-in/byte-out: `4-byte LE label len + label + 4-byte LE tx len + tx` | [src/multiplayer/solana/tauri_signer.rs:1-18](../../src/multiplayer/solana/tauri_signer.rs#L1-L18) |
| The signer already has a **keypair auto-sign** path keyed on `sessionStorage["xfchess_session_key"]` | [tauri/wallet-ui/src/App.tsx:874-884](../../tauri/wallet-ui/src/App.tsx#L874-L884) |
| The popup already handles versioned **and** legacy transactions | `deserializeTx`, [tauri/wallet-ui/src/App.tsx:855-861](../../tauri/wallet-ui/src/App.tsx#L855-L861) |

`http://localhost` is a **secure context** in Chrome, so WebCrypto / passkey /
TEE flows inside Privy work in the popup exactly as on `https://`.

### 4.2 Backend auth

[backend/src/signing/routes/auth.rs](../../backend/src/signing/routes/auth.rs) already has:

- `POST /auth/register`, `POST /auth/login` — ed25519 signature over
  `xfchess:<action>:<ts>`, 300 s replay window, 60 s future skew
  (`verify_wallet_sig`, lines 34-80).
- `POST /auth/siws-challenge` / `siws-verify` — nonce-based sign-in (lines 1247-1340).
- `POST /auth/register-email` / `login-email` — argon2; JWT subject `email:<addr>`.
- `POST /auth/link-wallet` — **refuses to re-point an account at a different
  wallet** once linked. Keep that invariant.
- `GET /auth/me` — returns `can_wager` = wallet linked + vault KYC + CACF, with a
  devnet bypass.

Users table is `users_v2 (wallet PK, username, email, kyc_status, created_at, deleted_at)`
([backend/migrations/003_wallet_first_auth.sql](../../backend/migrations/003_wallet_first_auth.sql)).
**The wallet pubkey is the primary key of identity.** Preserve that; do not make
a Privy DID the identity.

### 4.3 On-chain session keys — the thing that makes this cheap

[programs/xfchess-game/src/account_ix/global_session_ix.rs](../../programs/xfchess-game/src/account_ix/global_session_ix.rs):

```rust
pub struct AuthorizeGlobalSessionArgs {
    pub session_key: Pubkey,          // hot key allowed to co-sign
    pub duration_secs: Option<i64>,   // → expires_at
    pub spending_limit: Option<u64>,  // total lamports cap
    pub max_wager: Option<u64>,       // per-game cap
    pub games: Option<u16>,           // games_remaining
    pub deposit_lamports: u64,        // funds the delegation vault
}
```

Its own doc comment:

> After this call the session key may co-sign `global_create_game` and
> `global_join_game` without a wallet popup — for up to `DEFAULT_GAMES` games,
> within `spending_limit` lamports, and until `expires_at`.

`revoke_global_session` kills it immediately. The backend already exposes
`POST /global-session/register` and `DELETE /global-session/:wallet`
([backend/src/signing/routes/global_session.rs](../../backend/src/signing/routes/global_session.rs)),
and validates a submitted secret against the on-chain
`GlobalSessionDelegation.session_key` before accepting it.

Per project history this was **built and wired but its activation trigger was
disabled** — which is why "sign once" never actually worked. Re-enabling it is a
prerequisite of Phase 4, and worth doing regardless of Privy.

---

## 5 · Identity model — the decisions that are hard to reverse

### D1 · The Solana pubkey stays the identity; the Privy DID is a credential

`users_v2.wallet` remains the PK. The Privy `did:privy:…` lives in a *side* table
mapping credentials → wallet. Consequences:

- ELO, KYC, tournaments, game history, on-chain PDAs, `authed_wallet()` — none change.
- Dropping Privy later means those users keep their accounts and their funds;
  they simply re-authenticate differently (or export the key into Phantom).

### D2 · A Privy user is a first-class wallet user, not a second tier

`register`/`login` semantics are identical; `can_wager` is computed identically.
Do **not** add an `is_embedded_wallet` branch anywhere in game logic. The only
place the distinction may exist is UI copy (e.g. "back up your wallet").

### D3 · One human, one account — enforced at link time, not login time

The obvious failure: Alice signs up with Google (wallet A), later installs
Phantom (wallet B), and now has two accounts, two ELOs, and half her money in
each. Policy:

- Privy login for a **new** pubkey → new account, as today.
- Privy login where the Google email **already exists** on another account →
  do **not** auto-merge. Show: "This email is already linked to @handle. Sign in
  with that wallet, then link Google from Settings."
- Linking stays one-directional and irreversible without support, matching the
  existing `link_wallet` guard — reuse its error text verbatim.

Account *merging* (moving ELO/history/funds between wallets) is explicitly **out
of scope**: it touches KYC records, CACF state, on-chain PDAs and settled games,
and needs its own design.

### D4 · No delegated signing, no server-side custody — not without legal sign-off

Privy supports "delegated actions": the server signs for a user's wallet via
`POST https://api.privy.io/v1/wallets/{wallet_id}/rpc`. It is tempting — no
popup, no session key, trivially easy. **Reject it.**

- A platform that holds wagers *and* can unilaterally move player funds is
  plausibly a custodian in every one of UK/DE/BR/CA. That is exactly the boundary
  the 3-part legal model was built to stay on the safe side of.
- A backend compromise becomes a total-loss event for every player rather than a
  session-scoped one. `backend/.env` has already been in public git history once
  (secrets only partly rotated) — "the backend can sign anything" is not a
  theoretical blast radius here.

The bounded on-chain session key delivers most of the UX at a fraction of the
risk, and its limits are enforced by the Solana program rather than by our server.

### D5 · Recovery is a launch blocker, not a nice-to-have

If a player loses their Google account, an embedded wallet holding a wagered
balance is gone. Before real money:

- Configure Privy recovery (second factor / recovery password) for embedded wallets.
- Ship `useExportWallet` behind **Settings → Export private key** with a
  confirmation interstitial.
- Cap balances for social accounts that have neither recovery configured nor an
  acknowledged export (§11 C4).

---

## 6 · Website architecture (`xfchessdotcom/`)

### 6.1 The trick: register the embedded wallet as a Wallet Standard wallet

`@solana/wallet-adapter-react@0.15.39` already pulls
`@solana/wallet-standard-wallet-adapter-react@1.1.4`, which listens for
`wallet-standard:register-wallet` and wraps any registered wallet in a
`StandardWalletAdapter`. `WalletSelectionModal` already renders whatever is in
`useWallet().wallets` and even comments on self-registering Wallet Standard
wallets.

So: take Privy's embedded wallet, dispatch the register event, and it appears in
the existing picker as `adapter.name === 'Privy'`. **No change to
`anchor_client.ts`, `PlayPage`, `session.ts`, or any signing call site.**

The Wallet Standard `solana:signTransaction` feature is defined over
**`Uint8Array` in, `Uint8Array` out**. That is why the `@solana/kit` (Privy) vs
`@solana/web3.js` (us) split is a non-issue at the signing boundary — no typed
transaction object crosses it.

### 6.2 New files

```
xfchessdotcom/src/privy/
├── config.ts                  PRIVY_APP_ID / PRIVY_ENABLED  (revive from c690b4ca3^)
├── PrivyProviderWrapper.tsx   <PrivyProvider>; renders children unchanged when unset
├── registerStandardWallet.ts  RegisterWalletEvent + registerWallet()
├── PrivyStandardBridge.tsx    useSolanaStandardWallets() → registerWallet(), once
└── SocialLoginButtons.tsx     "Continue with Google"
```

`config.ts` — restore verbatim from the deleted commit; the no-op-when-unset
property is what keeps CI green with no secret configured:

```ts
export const PRIVY_APP_ID = (import.meta.env.VITE_PRIVY_APP_ID as string | undefined) || '';
export const PRIVY_ENABLED = PRIVY_APP_ID.length > 0;
```

Provider config in the **v3 shape** — note `embeddedWallets.solana.createOnLogin`;
the top-level `createOnLogin` used by the old code was removed in 3.0:

```tsx
<PrivyProvider
  appId={PRIVY_APP_ID}
  config={{
    appearance: { theme: 'dark', accentColor: '#14f195', walletChainType: 'solana-only' },
    loginMethods: ['google'],                                // wallets stay on wallet-adapter
    embeddedWallets: { solana: { createOnLogin: 'users-without-wallets' } },
    solana: { rpcs: { 'solana:devnet': { rpc: createSolanaRpc(endpoint) } } },
  }}
>
```

Registration bridge:

```tsx
import { useSolanaStandardWallets } from '@privy-io/react-auth/solana';

export function PrivyStandardBridge() {
  const { wallets } = useSolanaStandardWallets();
  const registered = useRef(new Set<string>());
  useEffect(() => {
    for (const w of wallets) {
      if (w.name === 'Privy' && 'privy:' in w.features && !registered.current.has(w.name)) {
        registerWallet(w);
        registered.current.add(w.name);
      }
    }
  }, [wallets]);
  return null;
}
```

`registerWallet` is Privy's documented `CustomEvent` helper: dispatch
`wallet-standard:register-wallet`, plus an `app-ready` listener for the case
where the adapter mounts first.

### 6.3 Edits to existing files

**[xfchessdotcom/src/App.tsx](../../xfchessdotcom/src/App.tsx)** — Privy sits
*outside* the wallet-adapter providers; the bridge sits *inside* so it can
register before enumeration:

```tsx
<PrivyProviderWrapper>
  <ConnectionProvider endpoint={endpoint}>
    <WalletProvider wallets={wallets} autoConnect={autoConnect}>
      <PrivyStandardBridge />
      <Router><AppContent /></Router>
```

**[xfchessdotcom/src/components/WalletSelectionModal.tsx](../../xfchessdotcom/src/components/WalletSelectionModal.tsx)** —
add the social block above the wallet list; add `'Privy'` to the descriptions
map; and crucially **exclude `'Privy'` from the `isTauri` disable branch** —
unlike Phantom/Solflare it needs no extension, so it is the one option that *does*
work inside the Tauri shell.

**Ordering caveat.** `autoConnect` runs on mount, while a Privy wallet registers
only after Privy's state hydrates. Expect the first paint to lack the Privy row.
The modal already re-renders from `useWallet()`; just never gate UI on "Privy is
present" at mount time.

### 6.4 Backend handoff on the website

Unchanged: once a wallet is selected the site runs the existing
`check-wallet → login | register` signature dance. A Privy embedded wallet signs
`xfchess:login:<ts>` through the Wallet Standard `signMessage` feature, and
`verify_wallet_sig` verifies it with **no backend change at all** — it is an
ordinary ed25519 signature from an ordinary Solana keypair.

> Worth internalising: **for the base login path the backend needs zero Privy
> awareness.** §9's route exists for credential linking and abuse control, not to
> make login work.

---

## 7 · Desktop architecture (`tauri/wallet-ui/` + `tauri/src/main.rs`)

### 7.1 The one genuinely hard problem: allowed origins

Privy enforces a per-app-ID allowed-origins list: exact origin, **port included,
no wildcard on port**, HTTPS required except for localhost. Our popup origin is
`http://localhost:N` where `N ∈ [7454, 7464]` — dynamic, because a second local
instance binds the next free port.

| | A · Allowlist the 11 localhost ports | B · Hosted handoff page | C · Fixed port only |
|---|---|---|---|
| Privy config | `http://localhost:7454` … `:7464` as allowed origins | `https://xfchess.com` only | one localhost origin |
| Bridge code change | none | new page + one-time-code exchange + backend route | fail hard if 7454 is taken |
| Multi-instance dev (`just dev2`) | works | works | **breaks** |
| Privy guidance | discouraged for production app IDs | clean | discouraged |
| Extra failure modes | another local page on those ports could *initiate* a login with our app ID (it cannot touch existing wallets) | Chrome Private Network Access blocks `https://` → `http://127.0.0.1`, so results must round-trip through the backend | — |

**Decision: A**, on the single app `cmt4t5mz200920ekywk04lspv` (§10). It is eleven
lines of dashboard config against a page that only ever runs on the user's own
machine, and it keeps desktop and web on one code path and one `aud` value.
Revisit at the mainnet cutover, or sooner if Privy tightens localhost policy —
§10 describes the split and the `aud`-as-a-set design that makes it cheap.

Keep **B fully designed** as the fallback — it is also the answer for any
non-Chromium desktop target:

```
popup ─▶ https://xfchess.com/desktop-link?code=<one-time>&sid=<sid>
             │ Google login + Privy wallet created   (origin: xfchess.com ✓)
             ▼
        POST /auth/desktop-link/complete { code, wallet, privy_token, signature }
             │ backend binds code → { wallet, xfchess_jwt }; 120 s TTL; single use
             ▼
  bridge ─ GET /auth/desktop-link/{code} (polled) ─▶ { wallet, token }
             │ bridge then performs its own internal POST /wallet
```

B needs the **backend** as relay precisely because a page on `https://xfchess.com`
cannot call `http://127.0.0.1:7454` — Chrome's Private Network Access blocks
public→private without an explicit opt-in, and we should not weaken the bridge's
CORS to permit it.

### 7.2 `wallet-ui` changes

`WalletStep` currently owns the whole connect-and-authenticate flow in one
`handleConnect`. Split the **provider-specific part** (obtain a pubkey and a
`signRaw`) from the **XFChess part** (`check-wallet` → `login`/`register` →
`POST /wallet` → `onAuth` → `onContinue`):

```ts
type WalletSource = {
  kind: 'phantom' | 'solflare' | 'privy';
  pubkey: string;
  signRaw: (msg: string) => Promise<string>;          // base58 signature
  signTransaction: (tx: Uint8Array) => Promise<Uint8Array>;
};

async function authenticateWithBackend(src: WalletSource): Promise<void> { /* today's lines 648-700 */ }
```

Phantom/Solflare build a `WalletSource` from `window.phantom.solana` /
`window.solflare`; Privy builds one from `useSignMessage()` / `useSignTransaction()`.

Three behaviours in today's code **must survive** the refactor. Each was a real
fixed bug and is documented as such in the source:

1. **Never report a wallet as connected before a signature verifies.** `POST /wallet`
   happens only after `login`/`register` succeeds — otherwise a rejected prompt
   still unlocked wagered play in the game client.
2. **`username` must come from this call's own auth response**, never
   `localStorage`. The popup's Chrome profile is shared across wallets and a
   stale name leaked into the game client's poller.
3. **Always prompt.** No `onlyIfTrusted` silent reconnect on the login path.

New layout:

```
tauri/wallet-ui/src/
├── App.tsx                    (WalletStep refactored to consume WalletSource)
├── wallet/
│   ├── types.ts               WalletSource
│   ├── extension.ts           phantom / solflare → WalletSource
│   └── privy.ts               Privy hooks → WalletSource
├── privy/
│   ├── config.ts              VITE_PRIVY_APP_ID (already in .env.example)
│   └── PrivyProviderWrapper.tsx
└── main.tsx                   wrap <App/> in the provider
```

`package.json` gains `@privy-io/react-auth` and `@solana/kit`. Note wallet-ui has
**no** `@solana/kit` today (unlike the website), so it must be added explicitly.

### 7.3 `TransactionSigner` with a Privy wallet

The component already branches three ways (session-key auto-sign, extension sign,
manual button). Add a fourth, keeping the byte-level contract exactly as-is:

```ts
// Privy branch — the SSE payload is base64 of the raw tx the Rust side built.
const bytes  = Buffer.from(txB64, 'base64');
const tx     = deserializeTx(bytes);        // unchanged: versioned, else legacy
await refreshBlockhash(tx);                 // unchanged
const signed = await signTransaction({ transaction: tx, wallet: privyWallet });
await resolveAndHide(Buffer.from(signed.serialize()).toString('base64'));
```

`ensureDevnet()` has no analogue for embedded wallets (there is no user-selected
cluster to nudge) — skip it for `kind === 'privy'`, and make sure
`isNetworkMismatchError()` is unreachable on that path.

**In practice this branch should rarely fire** for a Google user, because of §8.
It covers the onboarding transactions (`init_profile`,
`authorize_global_session`) and the fallback when no session is active.

### 7.4 `tauri/src/main.rs` changes

Almost none, which is the point. Two small ones:

- `serve_dist_file`'s MIME map has no `json` / `woff` / `wasm` arms and falls back
  to `application/octet-stream`. Privy's bundle may request a `.json` (or `.wasm`
  for its crypto path); add those arms or the popup fails in a way that is
  miserable to debug.
- Add a note to the CORS comment: **do not** widen the allowlist for Privy. Privy
  runs inside the popup's own origin and needs nothing from the bridge.

---

## 8 · In-game signing — why a Google user stops seeing popups

This is where the plan pays for itself.

### 8.1 The naive design (reject it)

Every in-game transaction → `sign_via_tauri_only` → bridge → popup wakes → Privy
signs. That is a Chrome window flashing to the foreground on every game create,
join and settlement. For extension wallets that is unavoidable (the extension
*is* the approval UI). For an embedded wallet it is pure friction with no
security benefit.

### 8.2 The design

**Onboarding — once, popup visible:**

1. Google login → embedded wallet `W`.
2. `xfchess:register:<ts>` signature → XFChess JWT. *(popup)*
3. `ProfileStep`: handle, country, DOB → off-chain profile. *(no signature)*
4. First wager attempt → `init_profile` transaction. *(popup; exists today)*
5. **`authorize_global_session`**, behind an explicit consent screen. *(popup)*

**Everything after that — no popup:**

6. The game client signs `global_create_game` / `global_join_game` / moves with
   the session keypair — the path `TransactionSigner`'s `handleAutoSign` already
   implements via `sessionStorage["xfchess_session_key"]`.
7. Session expires, `games_remaining` hits 0, or `spending_limit` is reached →
   the popup returns once to re-authorize.

### 8.3 The consent screen is a product requirement, not a formality

The user is authorising a hot key to spend their money. Show the real numbers
from `AuthorizeGlobalSessionArgs`, editable:

```
   Play without interruptions

   XFChess will create a temporary play key on this device.

   Spending cap        0.50 SOL       [edit]
   Max per game        0.10 SOL       [edit]
   Games covered       20             [edit]
   Expires             24 hours       [edit]

   You can revoke this any time in Settings.
   Your wallet key never leaves your device.

              [ Authorize ]   [ Not now ]
```

`Not now` must be fully functional — it falls back to popup-per-transaction.

### 8.4 Prerequisite

The global-session **activation trigger is currently disabled**, which is why
"sign once" has never worked. Phase 4 starts by re-enabling and re-testing
`authorize_global_session_if_needed` end-to-end on devnet. If that cannot be made
to work, this plan degrades to §8.1 (popup per transaction) and is still
shippable — just worse.

---

## 9 · Backend changes

Minimal by design. `POST /auth/login` already accepts an embedded wallet's
signature unmodified (§6.4). What follows serves linking, abuse control and support.

### 9.1 Migration `030_social_identities.sql`

```sql
-- Credentials that resolve to a wallet. One wallet may have several
-- (google + email); one credential resolves to exactly one wallet.
CREATE TABLE IF NOT EXISTS social_identities (
    provider      TEXT NOT NULL,          -- 'privy'
    subject       TEXT NOT NULL,          -- Privy DID, e.g. did:privy:cl…
    wallet        TEXT NOT NULL,          -- FK → users_v2.wallet
    login_method  TEXT NOT NULL,          -- 'google' | 'email' | …
    email         TEXT,                   -- as asserted by the provider
    embedded      INTEGER NOT NULL DEFAULT 1,
    created_at    INTEGER NOT NULL,
    last_login_at INTEGER NOT NULL,
    PRIMARY KEY (provider, subject)
);
CREATE INDEX IF NOT EXISTS idx_social_identities_wallet ON social_identities (wallet);
CREATE UNIQUE INDEX IF NOT EXISTS idx_social_identities_email
    ON social_identities (provider, LOWER(email)) WHERE email IS NOT NULL;
```

The partial unique index on email is what enforces **D3** at the database level:
one Google email cannot silently end up on two accounts.

### 9.2 `POST /auth/privy-login`

Body: `{ privy_token, wallet, signature, timestamp }`.

1. Verify `signature` over `xfchess:login:<timestamp>` — **reuse
   `verify_wallet_sig` unchanged**, replay window included. The wallet signature
   remains the authority; the Privy token is corroborating evidence, never a
   substitute for it.
2. Verify `privy_token` as ES256 against Privy's JWKS:

   ```
   GET https://auth.privy.io/api/v1/apps/{PRIVY_APP_ID}/jwks.json
   ```

   Verified live on 2026-08-23 for app `cmt4t5mz200920ekywk04lspv` — HTTP 200,
   two rotating keys:

   ```json
   { "keys": [
     { "kty": "EC", "crv": "P-256", "alg": "ES256", "use": "sig",
       "kid": "JA2V4sxfqwYeSq5P3PB_hjYZzRlWgkMIeqKkyl84puQ", "x": "…", "y": "…" },
     { "kty": "EC", "crv": "P-256", "alg": "ES256", "use": "sig",
       "kid": "OjBjEkbF1sDKlgHqFPdVFt-gyyfXlzowmeofZq47To0", "x": "…", "y": "…" }
   ] }
   ```

   - **Two keys means `kid`-based selection is mandatory.** Do not grab
     `keys[0]`; match the token header's `kid`, and on a miss re-fetch once
     (Privy rotates) before rejecting.
   - `iss == "privy.io"`, `exp` in the future (Privy access tokens live ~1 h),
     and `aud` ∈ **the configured app-ID set** — parse `PRIVY_APP_ID` as a
     comma-separated list even though it holds one value today, so the §10
     web/desktop split later is a config change rather than a code change.
   - `jsonwebtoken = "10.3.0"` is already in `backend/Cargo.toml` and supports
     `Algorithm::ES256` with `DecodingKey::from_jwk`. No new crate, no Privy SDK.
   - Cache the JWKS with a TTL — a `OnceLock<Mutex<(Jwks, Instant)>>` suffices;
     the codebase already uses that pattern. **Fail closed** on fetch failure;
     never fall back to unverified.
   - The JWKS URL is **public and unauthenticated** — verification needs no app
     secret. That is why Phases 0–4 never touch `PRIVY_APP_SECRET`.
3. Assert the Privy user actually owns `wallet` (check the linked-accounts claim,
   or call Privy's API server-side). Step 1 already blocks the practical attack,
   but this keeps the credential mapping honest.
4. Upsert `social_identities`; return `409` for D3's "email already linked",
   reusing `link_wallet`'s existing message.
5. Issue the ordinary XFChess JWT keyed on the wallet.

### 9.2.1 · Live credentials

```
PRIVY_APP_ID     = cmt4t5mz200920ekywk04lspv
PRIVY_JWKS_URL   = https://auth.privy.io/api/v1/apps/cmt4t5mz200920ekywk04lspv/jwks.json
PRIVY_APP_SECRET = privy_app_secret_58uxqae7c7kSex33RYd85huBBqB1iws3kp13Eh5nRq59mkiq54BCCvc3fexbq4tJhPomyeNh67YDPU69FofUUq2i
```

Placement:

| Value | File | Notes |
|---|---|---|
| `PRIVY_APP_ID` | `backend/.env` | used for the `aud` check and to build the JWKS URL |
| `PRIVY_APP_SECRET` | `backend/.env` | server-only; needed for step 3's Privy API call |
| `VITE_PRIVY_APP_ID` | `xfchessdotcom/.env` | baked into the public bundle — fine, it is an app **ID** |
| `VITE_PRIVY_APP_ID` | `tauri/wallet-ui/.env` | same |

`backend/.env` is untracked and matched by `.gitignore` lines 31-33 and 107-110
(verified 2026-08-23), so the secret does not enter git via the env files.

`PRIVY_APP_SECRET` must never be `VITE_`-prefixed or placed in either frontend
`.env` — those are compiled into public JavaScript. Add both backend values to
the startup env validation and the secrets inventory.

### 9.3 `GET /auth/me` additions

```rust
login_methods: Vec<String>,   // ["google"] | ["wallet"] | ["google","wallet"]
is_embedded_wallet: bool,     // UI-only: drives "back up your wallet" nudges
recovery_configured: bool,    // gates the balance cap in §11 C4
```

`can_wager` logic stays **unchanged**. Resist every temptation to branch it.

### 9.4 Rate limiting and abuse

Social signup is ~free, so Sybil accounts are ~free. Existing controls are
IP-based anti-cheat plus KYC-before-wagering. Add:

- Per-IP cap on new `social_identities` rows per 24 h.
- Flag accounts created <24 h ago that immediately enter wagered play into the
  existing `flagged_games` pipeline (migration 024).
- **Wagered play still requires KYC + CACF** — that is the real Sybil gate and it
  already exists; the above is early signal.

---

## 10 · Privy dashboard configuration

**One app, `cmt4t5mz200920ekywk04lspv`**, serving all three surfaces. The original
draft proposed splitting web and desktop across separate app IDs; a single app is
what actually exists, and it is the simpler starting point — one ID to configure,
one JWKS to cache, one `aud` value to verify. Split later only if the localhost
origins become a problem (see the note below).

| Setting | Value |
|---|---|
| App ID | `cmt4t5mz200920ekywk04lspv` |
| JWKS | `https://auth.privy.io/api/v1/apps/cmt4t5mz200920ekywk04lspv/jwks.json` |
| Login methods | Google only (`email_auth` disabled) |
| Embedded wallets | Solana, `createOnLogin: users-without-wallets` |
| Delegated actions | **OFF** (D4) |
| Recovery | ON (D5) |
| Chain | Solana devnet → mainnet at Phase 6 cutover |

Allowed origins — all of these, since one app covers every surface:

```
https://xfchess.com                 ← website, production
https://www.xfchess.com
http://localhost:7454               ← desktop popup, nominal port
http://localhost:7455                 …and the ten fallback ports the bridge
http://localhost:7456                 may bind when 7454 is taken
http://localhost:7457                 (tauri/src/main.rs:134-152)
http://localhost:7458
http://localhost:7459
http://localhost:7460
http://localhost:7461
http://localhost:7462
http://localhost:7463
http://localhost:7464
http://localhost:5173               ← vite dev, website
http://localhost:5174               ← vite dev, wallet-ui
```

**The one-app tradeoff, stated plainly:** Privy discourages localhost origins on
a production app ID, because any local page served on those ports can *initiate*
a login against this app. It cannot read or use an existing wallet, so the
practical exposure is low. The moment that stops being acceptable — most likely
at the Phase 6 mainnet cutover — split into `XFChess Web`
(`https://xfchess.com` only) and `XFChess Desktop` (localhost only), and give
the backend a small `aud` allowlist instead of a single value. Design for that by
making the `aud` check accept a **set** from day one, so the split is a config
change rather than a code change.

Env placement is in §9.2.1. All three files are written and gitignored.

---

## 11 · Compliance, legal and money

The existing legal model (chess = pure skill; no chance in paid flows; unlicensed
operation OK in UK/DE/BR/CA) is unaffected by *how* a wallet is created. What
changes is the **funnel**, and funnels are where compliance breaks.

**C1 — Age gating becomes more load-bearing.** Google sign-in removes every
friction signal that correlated with adulthood ("install Phantom, fund it, learn
seed phrases"). `ProfileStep` already collects country + DOB and the on-chain gate
requires `DOB > 0`. Make DOB **mandatory before any wagered flow** on the social
path, and non-editable afterwards without support.

**C2 — Sanctions / geo.** A Google account carries no country. Country continues
to come from `ProfileStep` + KYC. Do not let a Privy-asserted locale substitute.

**C3 — Custody.** Covered by D4. Additionally, the delegation vault funded by
`authorize_global_session`'s `deposit_lamports` sits in a **program-owned PDA**
under rules the program enforces, not in a platform account. Write that
distinction into the legal memo explicitly — it is what keeps the session-key UX
on the right side of the line.

**C4 — Loss of access.** See D5. Concrete gate: **cap the total balance an
embedded-wallet account may hold while it has neither recovery configured nor an
acknowledged key export.** Start at roughly a few entry fees. Enforce at wager
time in the backend; surface in the UI as "Back up your wallet to raise your limit".

**C5 — Stripe / Privy ToS.** See R1. Do not build on Privy's card onramp before
written clearance; Phases 0–4 do not depend on it.

**C6 — AGPL.** Privy's SDK is a proprietary npm dependency used by the web
frontend and the popup — ordinary linking of a separate work, no different from
`@solana/web3.js`. Revisit only if we ever vendor or patch it.

---

## 12 · Phased implementation

Each phase is independently shippable and independently revertible.

### Phase 0 — De-risk (½ day)

- [ ] Clean-install proof for **both** frontends with `@privy-io/react-auth`
      added and no `--legacy-peer-deps`. **This is the gate that failed in July —
      if it fails, stop here.**
- [ ] Same for `tauri/wallet-ui` (which has no `@solana/kit` today).
- [ ] Measure the `xfchessdotcom` bundle delta — Privy pulls styled-components,
      viem and WalletConnect; consider lazy-loading the provider.
- [x] Privy app created: `cmt4t5mz200920ekywk04lspv`. App ID + secret written to
      `backend/.env`; `VITE_PRIVY_APP_ID` written to `xfchessdotcom/.env` and
      `tauri/wallet-ui/.env`. All three gitignored (verified).
- [ ] Add the 14 allowed origins from §10 in the Privy dashboard.
- [ ] Add `PRIVY_APP_ID` / `PRIVY_APP_SECRET` to the CI/ops secret store and to
      `ops/scripts/deploy.ps1`'s env template, so prod gets them on deploy.
- [ ] Turn **off** delegated actions and turn **on** recovery in the dashboard.
- [ ] Fix the stale `src/privy/` claim in `xfchessdotcom/README.md` either way.

### Phase 1 — Website, login only (2–3 days)

- [ ] `xfchessdotcom/src/privy/*` (5 files, §6.2).
- [ ] `App.tsx` provider nesting; `WalletSelectionModal` social block.
- [ ] Verify a Google-created wallet completes the existing
      `check-wallet → register → login` flow with **zero backend changes**.
- [ ] Playwright: Google login (mocked) → wallet appears in picker → session
      established.
- [ ] Ship behind `VITE_PRIVY_APP_ID` — unset means literally nothing changes.

### Phase 2 — Backend identity (2 days)

- [ ] Migration `030_social_identities.sql`.
- [ ] `POST /auth/privy-login` + JWKS verifier + cache (§9.2).
- [ ] `GET /auth/me` fields (§9.3).
- [ ] Rate limits (§9.4).
- [ ] Tests: valid token; expired; wrong `aud`; wrong `iss`; JWKS fetch failure ⇒
      **fail closed**; email-already-linked ⇒ 409.

### Phase 3 — Desktop popup (3–4 days)

- [ ] `WalletSource` refactor of `WalletStep` — **pure refactor first, no Privy**,
      in its own commit, so the Phantom/Solflare regression surface is isolated.
- [ ] `tauri/wallet-ui/src/privy/*` + `wallet/privy.ts`.
- [ ] `TransactionSigner` Privy branch (§7.3).
- [ ] `serve_dist_file` MIME arms (§7.4).
- [ ] Register the 11 localhost origins; verify on a machine where 7454 is already
      taken, so the popup really lands on 7455+.
- [ ] Manual: game → Connect Wallet → Google → profile → back in game with a
      pubkey, **in a clean Windows VM with no wallet extension installed**. This
      is the scenario the whole plan exists for.

### Phase 4 — Session keys / no-popup gameplay (3–5 days)

#### 4a · Devnet redeploy runbook (do this first)

The vault-underfunding guard is written, compiles to BPF, and passes tests, but
has no effect until the program is redeployed. Verified prerequisites
(2026-08-23):

| Check | Value |
|---|---|
| Upgrade authority on-chain | `C1vn2MT7tZotZPjUJQDf9oo3dpZZ2tr7NxYLg8jTYgkw` |
| `keys/program-authority.json` resolves to | the same key ✅ |
| Balance | 16.08 devnet SOL |
| ProgramData | `DqgseskxrRYcgBpEks4jqh6eHbpg64SXxtGS1yhW9T7F`, 1,291,296 bytes |
| New `.so` | 1,294,640 bytes — **3,344 bytes larger**, so an extend is required first |

> **Trap: do not deploy with `target/deploy/xfchess_game-keypair.json`.**
> `cargo build-sbf` generates that file when it is absent, and the one it
> generated here resolves to `AhMV8Seoqkm78tA16KByE4YsA6QcDLu8XRmZARiBG9WF` —
> **not** the real program ID. Passing it to `solana program deploy` (or running
> a bare `anchor deploy`) publishes a brand-new program at a new address and
> leaves the live one untouched, which looks like a silent no-op. Always pass
> the program ID explicitly. The file is under `target/`, so it is gitignored
> and safe to delete.

```bash
# 1. Extend ProgramData to fit the larger binary (once).
solana program extend 8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU 32768 \
  --url devnet --keypair keys/program-authority.json

# 2. Upgrade in place — note --program-id takes the ADDRESS, not a keypair file.
solana program deploy target/deploy/xfchess_game.so \
  --program-id 8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU \
  --upgrade-authority keys/program-authority.json \
  --url devnet

# 3. Confirm the slot advanced.
solana program show 8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU --url devnet
```

- [x] Ran 4a. Slot advanced 484489210 → 486887670.
- [x] `global_create_game_rejects_an_underfunded_session_vault` and the
      join-side equivalent both assert `GlobalSessionVaultUnderfunded`, and
      `global_create_game_accepts_a_vault_funded_exactly_enough` proves the
      guard does not overshoot. 3 new tests, all passing.
- [x] Replaced the blanket early `return` in
      `authorize_global_session_if_needed` with a `wallet_is_embedded` gate.
- [x] IDL regenerated to `target/idl_new.json` — **not** copied into the web app,
      see the status block at the top for why.
- [ ] Devnet e2e of the full lifecycle with a real Google account. Needs the
      dashboard origins configured first.
- [ ] Consent screen (§8.3).
- [ ] Settings: view active session, revoke (`DELETE /global-session/:wallet` exists).
- [ ] Verify `handleAutoSign` covers `global_create_game` / `global_join_game`.
- [ ] Verify the limits actually bind: exceed `max_wager` and confirm the
      **program** rejects it, not just the UI.

### Phase 5 — Recovery, export, funding (2–3 days)

- [ ] Privy recovery configured; onboarding nudge.
- [ ] Settings → Export private key, with interstitial.
- [ ] Balance cap for un-backed-up accounts (§11 C4).
- [ ] Funding via `useFundWallet` — **subject to R1/C5 clearance**; otherwise
      route to the existing Coinbase Onramp plan.

### Phase 6 — Mainnet cutover

- [ ] Switch Privy chain config to mainnet-beta.
- [ ] Remove localhost origins from any production app ID.
- [ ] Load-test the JWKS cache.
- [ ] Runbook: "Privy is down" → social login degrades; extension wallets and
      existing sessions keep working. Prove it by blocking the Privy domains at
      the firewall.

---

## 13 · Testing

**Unit**
- `verify_privy_token`: valid / expired / wrong `aud` / wrong `iss` / bad
  signature / unknown `kid` / JWKS unavailable.
- `WalletSource` conformance: each of the three implementations round-trips a
  known message into a signature the backend's `verify_wallet_sig` accepts.

**Integration (backend)**
- New wallet via Privy → `users_v2` row + `social_identities` row.
- Same Google email, different wallet → 409, no rows written.
- `authed_wallet()` treats a Privy-issued JWT exactly like a Phantom-issued one.

**E2E (web, Playwright)**
- Google login (mocked Privy) → wallet in picker → play a casual game.
- Phantom still works with Privy enabled — the Phase 1 regression guard.

**Manual (desktop) — the ones that actually catch things**
1. Clean Windows VM, **no** wallet extension: Google → play.
2. Two local instances (`just dev2`) on 7454 and 7455 at once — both authenticate
   independently, no cross-talk. The bridge has a history of port-discovery bugs;
   this exercises them.
3. Kill the popup mid-signature → the game recovers.
4. Session key expires mid-session → the popup returns exactly once.
5. Block `*.privy.io` at the firewall → extension wallets unaffected, social login
   fails with a clear message instead of hanging.

---

## 14 · Rollout and kill switch

- Everything is gated on `VITE_PRIVY_APP_ID` / `PRIVY_APP_ID`. Unset ⇒ the code
  paths render nothing and the backend route 404s. That is the kill switch:
  **unset the var and redeploy.** Existing Privy users keep their wallets and can
  still sign in after exporting into Phantom.
- Devnet first, for a full release cycle, before mainnet.
- Watch: social-signup → first-game conversion; social-vs-extension wager rate;
  session-key authorize/decline ratio; JWKS cache hit rate; Privy error rate. All
  fit the existing Prometheus `/metrics` + Grafana stack.

---

## 15 · Open questions

1. **R1** — is a real-money skill-chess platform in-policy for Privy wallets under
   Stripe ownership? *Blocks Phase 5 funding only.*
2. ~~Does Privy's Wallet-Standard object expose `signMessage` as well as
   `signTransaction`?~~ **Answered: yes, both, and both are bytes-in/bytes-out**
   (`signMessage({message: Uint8Array, wallet}) -> {signature: Uint8Array}`,
   `signTransaction({transaction: Uint8Array, wallet}) -> {signedTransaction: Uint8Array}`).
   This is what makes the kit-vs-web3.js split a non-issue at the signing
   boundary, as §6.1 predicted.
3. ~~Bundle-size budget for `xfchessdotcom`.~~ **Answered and handled.** The SDK
   is 3.8 MB raw / 1.1 MB gzip — 5x the rest of the app. It is now loaded via
   dynamic import (`src/privy/privyRuntime.ts`), which holds the eager payload
   at 269 KB gzip against a 262 KB pre-Privy baseline.

   Two traps found while doing it, both worth remembering:
   - A `advancedChunks` group naming `@privy-io` pulls the chunk back into the
     entry graph, undoing the dynamic import. Check the script tags in
     `dist/index.html`, not the chunk list, to see what actually loads.
   - Sweeping `@walletconnect`/`@coinbase`/`@metamask`/`viem` into that group is
     worse still: Privy dynamically imports those EVM connectors, so a
     Solana-only social-login config never fetches them unless you force them
     into an eager chunk yourself.
4. Account merging (D3) is deferred — what do we tell the first user who asks?
5. Should the desktop popup use option B (hosted handoff) from the start, given a
   non-Chromium desktop target may arrive sooner than expected? The current
   release branch is `feat/chromeos-release`, so answer this **before** Phase 3.

---

## Appendix A — Verified facts (2026-08-22)

| Fact | Source |
|---|---|
| `@privy-io/react-auth@3.37.4`; all Solana peers optional, `@solana/kit >=3.0.3` | npm registry metadata |
| `@solana/kit@6.10.0` already installed in `xfchessdotcom` via `@coral-xyz/anchor@^0.32.1` | `xfchessdotcom/package-lock.json:3663` |
| `@solana/wallet-standard-wallet-adapter-react@1.1.4` already in the tree | `xfchessdotcom/package-lock.json:6395` |
| Privy v3 removed top-level `createOnLogin`; use `embeddedWallets.solana.createOnLogin` | Privy 3.0 migration guide |
| Privy v3 split `useSolanaWallets` into `useWallets` / `useCreateWallet` / `useExportWallet` | Privy 3.0 migration guide |
| Privy access token: ES256, `iss=privy.io`, `aud=<app id>`, `sub=<did>`, ~1 h TTL | Privy access-tokens doc |
| JWKS for app `cmt4t5mz200920ekywk04lspv` returns HTTP 200 with **two** EC P-256 `ES256` keys (`kid` `JA2V4sxf…` and `OjBjEkbF…`) — so `kid` selection is mandatory | fetched live 2026-08-23 |
| `backend/.env`, `xfchessdotcom/.env`, `tauri/wallet-ui/.env` all matched by `.gitignore` (lines 31-33, 107-110) | `git check-ignore`, 2026-08-23 |
| Allowed origins: exact port required, no port wildcards, localhost discouraged in prod | Privy allowed-domains doc |
| Privy documents registering the embedded wallet via `wallet-standard:register-wallet` | Privy standard-wallets recipe |
| Privy funding routes cards via Stripe (US/EU) and aggregates Coinbase/MoonPay elsewhere | Privy funding doc |
| Privy is a Stripe company; Dynamic → Fireblocks; Web3Auth → Consensys | provider comparison roundups |
| `jsonwebtoken 10.3.0` (ES256-capable) already a backend dependency | `backend/Cargo.toml:47` |

## Appendix B — References

- Privy + Solana getting started — https://docs.privy.io/recipes/solana/getting-started-with-privy-and-solana
- Privy Solana standard wallets — https://docs.privy.io/recipes/solana/standard-wallets
- Privy 3.0 migration — https://docs.privy.io/basics/react/advanced/migrating-to-3.0
- Privy access tokens — https://docs.privy.io/authentication/user-authentication/access-tokens
- Privy allowed domains — https://docs.privy.io/recipes/dashboard/allowed-domains
- Privy funding (fiat onramp) — https://docs.privy.io/guide/frontend/embedded/fiat-onramp
- Privy pricing — https://www.privy.io/pricing
- Embedded-wallet provider comparison — https://www.openfort.io/blog/top-10-embedded-wallets
