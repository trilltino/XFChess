# Identity Model: Account (wallet + Lichess-linked) vs Guest/Local Play

## Context

XFChess should have exactly **two kinds of identity**, not a spectrum:

1. **Account** — your on-chain profile. Backed by a Solana wallet and an
   on-chain `PlayerProfile` PDA, with a Lichess account linked via the
   Lichess API. Tracks **two Elo numbers side by side**: your Lichess Elo
   (pulled from Lichess) and your XFChess on-chain Elo (native rating from
   games played on XFChess). You can log into this one account via wallet
   signature, email+password, or Lichess OAuth — three doors, one account.
   This is where KYC, tournaments, wagers, and prizes live.
2. **Guest / Direct Connection** — no account at all. A username you pick
   for yourself, saved only to the local device cache — not the backend, not
   globally unique, just what your friend sees when you connect to them by
   node ID. No Elo. Lets you play the computer, solve puzzles, and connect
   directly to a friend over a copy-pasted connection ID. Nothing here ever
   touches the backend.

Today's code doesn't reflect either half of this correctly:
- `users_v2` is wallet-first but has email/password bolted on as a separate,
  tangled path (`backend/migrations/007_add_password_hash.sql` re-adds
  `password_hash` after older email/password code was never fully removed) —
  it needs to converge into "one account, three login doors," not stay a
  patchwork.
- There is no Lichess integration anywhere in the codebase yet — this is new
  work, not a wiring fix.
- Bot games and local P2P games never touch the backend at all today, for
  anyone, logged in or not (`GameMode::SinglePlayer`/`MultiplayerLocal`
  early-return before any network call). That's *correct* for Guest play,
  but wrong for a logged-in Account — a bot game played while logged into
  your Account should still update your on-chain Elo and be recorded
  server-side, not just cached locally.

This plan also folds in a related but separate cleanup: the backend has two
differently-valued config fields both casually called "treasury"
(`treasury_authority_key` vs `host_treasury_pubkey`), a real source of
confusion even though no code path was found that actually displays a
treasury address as a player's own wallet.

Decisions made with the user so far:
- Email + password stays as one of the three login doors into the Account
  (reuse existing Argon2 `register_email`/`login_email`) — not passwordless.
- XFChess will **sponsor the SOL rent for a player's first on-chain profile**
  creation, removing the "need SOL before you can go on-chain at all"
  problem.
- KYC is required specifically to create the on-chain Solana profile — not
  for email/password login, not for linking Lichess, not for Guest play.
- The treasury/login-wallet naming confusion has no known repro — treat it as
  a config-naming fix, not a bug hunt.

## Current State (verified in code)

**Backend (`backend/src/db`, `backend/src/signing/`)**
- `users_v2`: `wallet TEXT PRIMARY KEY, username, email (optional), password_hash, kyc_status`.
  Wallet-first; email/password is a secondary path onto the *same* row/table
  — this is actually the right shape for "one account, three doors," it just
  needs the third door (Lichess) added and the game-recording gap closed.
- Auth: SIWS (`/auth/siws-challenge` → `/auth/siws-verify`) issues a JWT keyed
  on wallet pubkey. `/auth/register-email` + `/auth/login-email` issue a JWT
  keyed on `"email:<addr>"`. `/auth/link-wallet` merges an email account onto
  a wallet — this linking pattern is the template for linking Lichess too.
- `/auth/init-profile-tx` (`backend/src/signing/routes/auth.rs:650-746`) builds
  an unsigned `init_profile` instruction with the **player's wallet as fee
  payer** — no sponsorship today.
- No backend endpoint exists for recording a bot/local-P2P game result at all,
  for any account state. The only result-reporting paths (`/ratings/update`,
  `/game/undelegate`+`/game/finalize`) require an on-chain
  `CompetitiveMatchState`/`game_id`, which only exists for wallet-backed
  online lobby games.
- No Lichess integration exists anywhere (`grep -ri lichess` across
  `backend/` and `src/` returns nothing).
- Two "treasury" fields in `backend/src/signing/config.rs`: `treasury_authority_key`
  (env `TREASURY_AUTHORITY_KEY`, signs `withdraw_treasury`, pubkey `9jpjASz...`)
  and `host_treasury_pubkey` (env `HOST_TREASURY_PUBKEY`, tournament entry-fee
  recipient, pubkey `uLgR6Nx4...`). Both are gated behind `require_api_key` via
  `/admin/wallet-balances` — not reachable by a normal player session, but
  genuinely confusing in code/docs/conversation.

**Game client (`src/`)**
- `GameMode::SinglePlayer` (vs AI) and `GameMode::MultiplayerLocal` never touch
  the network layer — verified via early-returns in
  `dispatch_remote_moves`/`emit_game_ended_event` (`src/multiplayer/systems.rs`).
  Correct behavior for Guest play; needs a login-gated exception for Account play.
- `GameState` defaults to `Auth` on wasm32 builds (web) but `MainMenu` on native
  — i.e. the web build already assumes login-first; native does not.
- On-chain profile creation call sites: `src/ui/account/profile_creation.rs:235`
  (in-game UI) and `src/solana/program_interface/instructions.rs::init_profile_ix`.
- `src/multiplayer/network/identity.rs`'s persistent local Iroh node key is
  already the de facto identity for Guest/Direct Connection play — no account
  needed, already independent of email/wallet identity.

**Solana program (`programs/xfchess-game/`)**
- `InitProfile` (`account_ix/profile.rs`): player's own wallet pays rent via
  `invoke_signed(system_instruction::create_account(&player.key(), ...))`. No
  sponsor/relayer path exists in this instruction today.
- Tournament `register` (`tournament_ix/registration/register.rs`): requires an
  *existing* `player_profile` PDA, player signs and pays the entry fee into
  escrow. **Both native clients stub this out** — `web-solana/src/pages/TournamentDetail.tsx`
  `handleRegister` just logs, and `src/multiplayer/solana/tournament.rs::register_tournament`
  is a no-op returning `Ok(0)`. Only the Blinks-built flow
  (`backend/src/signing/blinks/core.rs::build_register_transaction`) is
  functional today.

## Target Architecture

**Account (the one identity that persists and matters for ranking/money)**
- One backend account row (`users_v2`, or its successor), reachable via three
  login doors: wallet signature (SIWS), email+password
  (`register_email`/`login_email`), or Lichess OAuth (new). `/auth/link-wallet`
  already shows the pattern for converging doors onto one row — add
  `/auth/link-lichess` alongside it.
- **Lichess integration (new work, no existing scaffolding)**:
  - Register an XFChess OAuth application with Lichess; store client
    id/secret in backend config the same careful way other secrets are
    handled (see [[project_secret_exposure]]).
  - New routes: `/auth/lichess/connect` (redirect to Lichess OAuth) and
    `/auth/lichess/callback` (exchange code, call Lichess's `GET /api/account`
    for username + rating, store on the account row).
  - New account fields: `lichess_username`, `lichess_elo` (Lichess exposes
    per-variant ratings — blitz/rapid/classical/etc; default to one clearly
    labeled variant, e.g. classical or blitz, and revisit if multiple are
    wanted).
  - Client UI: a "Connect Lichess" button alongside "Connect Wallet" in the
    account/profile screens, and a second Elo stat tile next to the existing
    on-chain ELO tile (`ProfileStep`'s stats grid in `SignIn.tsx`, and the
    equivalent native profile UI) showing the Lichess Elo.
- **Every game played while logged into the Account — including bot games —
  gets recorded to the backend**, in addition to local cache, and updates the
  on-chain Elo. This requires a **new backend recording path** that doesn't
  depend on `CompetitiveMatchState`/`game_id` (that stays reserved for
  on-chain wagered games): a new endpoint to persist a completed game result
  + update the account's on-chain Elo, keyed by account id rather than an
  on-chain match id. Client-side, `SinglePlayer`/`MultiplayerLocal` game-end
  handlers need a login-gated call to this endpoint (skip entirely when
  playing as a Guest).
- **Sponsor first on-chain profile creation**: a backend-paid path for
  `init_profile` (backend keypair as fee payer/rent payer for a player's
  *first* profile only, with an abuse guard — one sponsorship per verified
  account). Removes the "need SOL to go on-chain at all" chicken-and-egg
  problem.
- **KYC gates on-chain profile creation specifically** — not email/password
  login, not Lichess linking, not Guest play. The backend route that sponsors
  `init_profile` must check `kyc_status` and reject/redirect if unverified.
- Fix the stubbed native tournament `register` call sites so wallet-gated
  tournament joining actually works outside of Blinks.

**Guest / Direct Connection (no account, nothing persists server-side)**
- A locally-chosen display username, stored in local device cache/config —
  **no backend call, no `check-username` availability check** (that check is
  for Account sign-up only; a Guest username only needs to make sense to the
  one friend you're connecting to).
- No Elo, ever.
- Capabilities: play vs computer, solve puzzles, and Direct Connection P2P
  play via a copy-pasted node ID — see the dedicated section below.
- Every game played as a Guest is cached locally only; nothing is sent to the
  backend, matching today's existing (and correct, for this tier) behavior.

**Strong separation, concretely**
- The UI must never suggest Guest play "counts" toward any rating or backend
  record — no Elo display, no history, for Guest sessions.
- Any screen involving money (tournament entry, prize claim, withdrawal) must
  gate on wallet-connected + on-chain profile existing within the Account,
  never reachable from Guest mode at all.
- Keep on-chain Elo and Lichess Elo as distinct, clearly-labeled fields — show
  both, never merge or silently prefer one over the other.

## Login / Sign-Up Screen (three doors into one Account, KYC-gated profile creation)

**Ask**: one screen, black background, "XF Chess — Competitive Chess Server"
banner at top (matching the web splash banner). A username/password box logs
into the Account directly. Below it, "Connect Wallet" and "Connect Lichess"
options also log into (or create) the same Account. Sign-up on the same
screen collects email + username (availability-checked) + password. From
there, a "Create Solana Account" action creates the linked on-chain profile —
gated behind completed KYC, globally, not just at wager time.

**This is mostly already built for the wallet/email doors — reuse it:**

- `src/ui/account/auth.rs` (`AuthUiPlugin`, `GameState::Auth`) is a full
  black-background egui login/register screen: username/email/password
  fields, Login/Register toggle, a "LOGIN WITH WALLET" button, wallet-connect
  polling via Tauri (`perform_wallet_connect`), wallet registration
  (`perform_wallet_register`), and a post-login **"Complete Profile" consent
  modal** — "In order to participate in Wager matches KYC and a Solana Wallet
  is required do you wish to proceed?" with **Yes** (opens the Tauri
  wallet/profile popup) vs **"No, I want to play local-online"**. That second
  option needs to be re-scoped to mean "continue as Guest" now, not "stay
  logged in as some separate local identity."
- Backend already has `/api/auth/register-email` + `/api/auth/login-email`
  (Argon2-hashed passwords — already "idiomatic production" storage, no
  change needed), `/api/auth/check-username/:u` (availability check — exists
  but **no client currently calls it**), `/api/auth/link-wallet`,
  `/api/auth/register` + `/api/auth/login` (wallet signature), and
  `/api/auth/sync-profile`.
- `web-solana/src/pages/SignIn.tsx` has the web equivalent: `IdentityStep`
  (wallet vs email+password) → `CredentialsStep` → `ConnectWalletStep` →
  `ProfileStep` (on-chain profile creation form, currently collecting only
  country/tax_id/dob).
- The full formal KYC form already exists at `web-solana/src/pages/Kyc.tsx`
  (full name, DOB, residence, country-specific tax ID) — it's just not wired
  to gate profile creation today.
- **The Lichess door does not exist yet anywhere** — new UI button + OAuth
  flow, see Target Architecture above.

**Concrete gaps to close:**

1. **Banner copy** — `render_floating_title` in `auth.rs` renders only
   "XFCHESS"; add the "COMPETITIVE CHESS SERVER" subtitle (already used as
   the main-menu fallback logo text in `new_menu.rs`) so the auth screen
   matches the web splash's "Competitive Chess Server" banner.
2. **Add the Lichess door** — a "CONNECT LICHESS" button next to "LOGIN WITH
   WALLET" in `auth.rs`, and next to the wallet button in `SignIn.tsx`'s
   `IdentityStep`/`ConnectWalletStep` — wired to the new
   `/auth/lichess/connect` OAuth flow.
3. **Wire up username availability** — `/api/auth/check-username/:u` exists
   server-side but neither the native Register form nor web
   `CredentialsStep`/`ProfileStep` calls it live. Add a debounced check to
   both, for Account sign-up only (not Guest usernames).
4. **Web forces wallet connection today** — `SignIn.tsx`'s `handleAuth`
   unconditionally sends every email/password signup into
   `ConnectWalletStep`. Add a "continue as Guest" equivalent so a web user can
   bail out to Guest play entirely, matching native's "No, I want to play
   local-online" button (re-scoped per above) and the Guest section below.
5. **Policy: KYC required at Solana-account creation, not just at wager
   time.** This reverses a deliberate existing choice —
   `tauri/wallet-ui/src/App.tsx`'s `ProfileStatus` type has an explicit
   comment: *"KYC (`is_verified`) is intentionally not gated on here — that's
   checked later, at wager time, same as the existing CACF compliance flow."*
   - Gate the profile-creation action (native "Create Solana Account" /
     web `ProfileStep`'s "Initialize Profile" / the Tauri wallet-ui profile
     flow) behind a completed KYC check against `vault_users.kyc_status`.
   - Replace `ProfileStep`'s current lightweight country/tax_id/dob-only
     capture with an actual gate on the full `Kyc.tsx` submission before
     `createPlayerProfile`/`init_profile` is allowed to fire.
   - The backend route that sponsors `init_profile` must check `kyc_status`
     and reject/redirect if unverified — the KYC gate and the sponsorship
     guard land in the same route.
   - Update `Kyc.tsx`/`Compliance.tsx` copy ("Triggered on first deposit or
     first wagered match entry") to match the new trigger point — flag for
     legal review, not just a code change (see [[project_legal_jurisdictions]]).
6. **Email/password and Lichess-only logins stay KYC-free.** The gate applies
   only to the "create Solana account" action.

## Guest / Direct Connection: launch-screen escape hatch + P2P connection UI

**Ask**: from the same launch/splash screen, a player who wants no account at
all should be able to play the computer, solve puzzles, or connect directly
to a friend via a copy-pasted connection ID — with a plain-language
(non-programmer) explainer of how the ID-based connection works, and a
host/join UI built the same way the existing Blitz/time-control host flow
works. Pick a local display username; no Elo; nothing recorded server-side.

**Already true — reuse, don't rebuild:**

- Native desktop builds already default to `GameState::MainMenu`, not
  `GameState::Auth` (`GameState::Auth` is only forced via the `FORCE_AUTH` env
  var — `src/core/plugin.rs::check_force_auth`). So "Play Against a
  Computer," "Solve Puzzles," and "Play Online → Create Lobby / Join Lobby"
  (`src/states/main_menu/new_menu.rs`) are **already reachable today with
  zero login** on native.
- The hover-explainer mechanic the ask describes already exists:
  `item_tip`/`item_expandable_tip` helpers in `new_menu.rs` render a
  one-line plain-English explanation under each menu item on hover — e.g.
  "Play Online — Host or join a live game against a friend or a matched
  opponent." Extend this pattern for the new Direct Connection entry.
- `src/multiplayer/join_link.rs` already has a
  `xfchess://join/<game_id>/<host_node_id_b58>` deep-link scheme
  (`make_join_link`/`parse_join_link`) and a `JoinViaLinkEvent` — the
  "share an ID with your friend" concept already exists in link form.
- `src/multiplayer/network/identity.rs`'s persistent local Iroh node key is
  already the identity backbone for this tier.

**Gaps to close:**

1. **Web/wasm32 build has no escape hatch.** The wasm32 target defaults
   straight into `GameState::Auth` with no way to reach bot play, puzzles, or
   P2P without going through login first. Add "Play vs Computer," "Puzzles,"
   and "Direct Connection" buttons directly on that screen, mirroring native.
2. **Guest username picker** — a simple local text field (client-side only,
   persisted to local config/cache) shown the first time a player enters
   Guest mode; no backend call.
3. **"Create Lobby"/"Join Lobby" today go through the VPS-backed lobby
   directory** (`p2p_vps_state` in `src/multiplayer/network/p2p_vps.rs`) — a
   backend-mediated discovery list, not the raw manual ID exchange the ask
   describes. Add a distinct **"Direct Connection"** menu entry (using
   `item_expandable_tip`, plain language: *"This works by using an ID. You
   copy it and send it to your friend."*) with two actions:
   - **Host** — reuse the existing Blitz/time-control `HostConfig` setup
     screen as-is, but skip VPS lobby registration entirely: start hosting on
     the local Iroh node and surface a copy button for your own node ID (or
     the existing `xfchess://join/...` link via `make_join_link`).
   - **Join** — a new panel with a text field where the player pastes a
     friend's node ID (or a full `xfchess://join/` link, reusing
     `parse_join_link`) to connect directly — no backend call, no account, no
     VPS lookup.
4. **Keep this path backend-free.** Bot play, puzzles, and Direct Connection
   must never call the new Account game-recording endpoint — confirm the
   gating is "logged into Account?" not "feature enabled?", so it fails safe
   toward not-recorded rather than accidentally recording Guest games.

This is purely a Phase 2, client-side UI item (new egui panels + wiring
existing Iroh node-id/join-link primitives) — no backend or Solana program
changes required.

## Treasury Naming Cleanup

- Rename `host_treasury_pubkey` → something unambiguous (e.g.
  `tournament_fee_recipient` or `entry_fee_vault_pubkey`) to stop it reading as
  the same thing as `treasury_authority_key` (the withdraw-authority signer).
  Update `backend/.env.example`, `deploy/backend/.env.example`, and all call
  sites in `backend/src/signing/blinks/`.
- Leave `/admin/wallet-balances` and `/admin/treasury/refund` as-is (already
  correctly gated behind `require_api_key`) — just confirm no player-facing
  route ever echoes either treasury field back.

## Phased Implementation

1. **Backend**: converge `users_v2` around "one Account, three login doors"
   (wallet/email/Lichess); add Lichess OAuth app registration + `/auth/lichess/connect`
   + `/auth/lichess/callback` + `lichess_username`/`lichess_elo` fields; add
   the Account-scoped game-recording endpoint (updates on-chain Elo, records
   any game played while logged in, including bot games); add sponsored
   `init_profile` route with KYC + abuse guard; rename the treasury config
   fields.
2. **Game client**: wire `SinglePlayer`/`MultiplayerLocal` game-end to the new
   recording endpoint, gated on "logged into Account" (never fires for
   Guests); add the Guest username picker; add the Direct Connection host/join
   panels; add the Lichess connect button + dual-Elo display.
3. **Solana program**: no new instruction needed for sponsorship — reuse
   `InitProfile` with the backend as fee-payer signer for the sponsored case
   (verify the account's authority semantics still resolve to the *player's*
   identity, not the backend's).
4. **Web frontend**: implement the real `handleRegister` in
   `TournamentDetail.tsx` against the on-chain `register` instruction (or the
   existing Blinks flow); finish `src/multiplayer/solana/tournament.rs::register_tournament`;
   add the Guest escape hatch to `SignIn.tsx`; add the Lichess connect button.
5. **Cleanup pass**: once "one Account, three doors" is live, remove any
   remaining code that treats email-only and wallet-only as separate account
   kinds; migrate existing rows.

## Verification

- Unit/integration: `cargo test -p backend` for the new Account game-recording
  endpoint and Lichess OAuth callback (mock the Lichess API); `cargo test -p
  xfchess-game` for sponsored `init_profile` (fee-payer-not-player case).
- Manual: log into the Account via email+password only (no wallet connected
  this session) and confirm the on-chain Elo still displays from the last
  known on-chain state. Connect Lichess and confirm the Lichess Elo populates
  alongside it, clearly labeled as distinct. Play a bot game while logged
  into the Account and confirm it's recorded server-side and updates on-chain
  Elo. Then log out entirely, play as a Guest (pick a local username, no
  login), play a bot game and a Direct Connection game with a friend's node
  ID, and confirm nothing hits the backend — no Elo, no history, purely
  local. Finally, attempt on-chain profile creation without completed KYC and
  confirm it's blocked, then complete KYC and confirm it succeeds with zero
  SOL in the wallet (sponsored rent).
