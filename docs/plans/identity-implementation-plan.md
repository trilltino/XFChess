# Two-Tier Identity System — Implementation Plan

## Context

`docs/plans/identity-separation-plan.md` already lays out the target
architecture (Account = wallet + on-chain profile + linked Lichess, dual Elo;
Guest = no-login local play) and investigation findings. This plan is the
follow-up: a concrete, file-level build plan to actually implement it.

**Major correction from the investigation phase**: the earlier plan assumed
Lichess integration was entirely new work. It is not. Deep grounding research
this session found that **the backend and on-chain sides of Lichess linking
are already built**:
- `backend/src/signing/routes/lichess_oauth.rs` (`/api/auth/lichess/init`,
  `/api/auth/lichess/exchange`) and `backend/src/signing/routes/external_elo.rs`
  (`/api/external-elo/link/start|confirm|status|sync`) — full OAuth + linking
  flows already exist.
- The on-chain `PlayerProfile` struct (`programs/xfchess-game/src/state/player_profile.rs`)
  **already has** `lichess_username`, `lichess_verified`, `lichess_blitz`,
  `lichess_rapid`, `lichess_bullet`, `lichess_last_sync`, `external_elo_source`,
  `seeded_from_external` fields (265-byte account, confirmed via
  `#[derive(InitSpace)]` layout and cross-checked against
  `backend/src/signing/elo_cache.rs`'s hardcoded byte offsets).
- `link_external_elo_ix` (`backend/src/signing/solana/instructions.rs:182-209`)
  is signed by a backend-held `link_authority` keypair — **already a
  backend-sponsored on-chain write**, the exact pattern needed for sponsored
  `init_profile` too.
- `xfchessdotcom/src/components/LichessLinkCard.tsx` is a working React
  reference UI for the connect flow.
- Migration `009_external_elo.sql` already created `external_elo_links` /
  `external_elo_sync_log` tables.

So dual-Elo display is almost free — `fetchPlayerProfile` already decodes
`lichessBlitz`/`lichessVerified`/etc. alongside `eloRating`. **The real gap is
narrower than previously scoped**: native (Bevy/egui) client UI for
Lichess-connect doesn't exist yet; the Guest tier doesn't exist; KYC doesn't
gate profile creation yet; Direct Connection (raw node-ID) P2P UI doesn't
exist yet (only VPS-mediated lobbies do); and profile creation isn't
sponsored yet.

**Also found, load-bearing**: migrations in this repo are **not** applied via
`sqlx migrate run` despite what `backend/CLAUDE.md` says — they're wired
individually via `include_str!` + `run_script()` calls hardcoded in
`backend/src/infrastructure/database.rs::run_migrations()` (lines 81–185). A
new migration file does nothing until it's added there explicitly. Next
migration number is `020`.

**Also found, needs a decision before building the KYC gate**: there appear
to be *three* KYC-status stores (`users_v2.kyc_status`, `kyc_records` in the
vault DB via `VaultStore::has_kyc`, and `vault_users` in the vault DB). A
likely latent bug: `backend/src/signing/routes/identity.rs:153-163`
(`register_identity`) inserts into a table literally named `users` (not
`vault_users`), whose schema doesn't match the insert's columns — this should
error on every call, meaning `vault_users` (which `tournament.rs:815`'s KYC
gate reads) may never actually get populated in practice. **Recommendation**:
build the new profile-creation KYC gate on `users_v2.kyc_status` /
`VaultStore::has_kyc` (the stores that are actually written to), not
`vault_users`. Verify `submitKyc`'s actual write path first (Phase 0 below)
before wiring anything to it.

## Design decisions made this session (stated, not re-litigated)

1. **Casual/bot games recorded while logged into Account do NOT touch the
   on-chain `elo_rating`.** Writing to a Solana account costs a transaction
   per game; on-chain Elo should stay driven only by real on-chain
   wagered/ranked settlement, as today. Casual games get a new **backend-only**
   history table with no rating effect — this avoids any Solana program
   change for this piece entirely.
2. **Profile-creation sponsorship needs no Anchor account-struct change.**
   `InitProfile`'s `player: Signer<'info>` only requires `player` to *sign* —
   nothing requires them to be the transaction's fee payer. The backend can
   build the transaction with its own keypair as fee payer (position 0 of
   `Message::new_with_blockhash`) while `player` still signs to satisfy the
   constraint — the same shape already used for Blinks tournament
   registration. No new instruction, no new account.
3. **Lichess OAuth for the native client uses the standard PKCE flow**
   (confirmed against `lila` source, not docs): `GET /oauth` (not
   `/oauth/authorize`) with `code_challenge`/`code_challenge_method=S256`,
   then `POST /api/token` (not `/oauth/token`) with `code_verifier` — no
   client secret, no scope needed for basic profile+rating (`GET /api/account`
   requires zero scopes). No app pre-registration with Lichess is required —
   `client_id` is an arbitrary string, `redirect_uri` just needs to parse and
   match; a loopback (`http://127.0.0.1:<port>/callback`) redirect is
   explicitly allowed. This should reuse the backend's existing
   `lichess_oauth.rs` flow rather than reimplementing PKCE in Rust/Bevy —
   confirm `lichess_oauth.rs`'s `/init` already generates the PKCE
   verifier/challenge server-side (likely does, given it's designed for a web
   popup flow); if so, the native client only needs to open the returned
   `authUrl` in the system browser and receive the callback, mirroring the
   existing wallet-connect Tauri bridge pattern (`perform_wallet_connect` in
   `src/ui/account/auth.rs`, and the `localhost:7454` bridge in
   `tauri/wallet-ui`).

## Phase 0 — Verify before building on top of it

- Read `backend/src/signing/routes/kyc.rs` (or wherever `/api/kyc/submit`
  lives) to find its actual write path, and confirm/deny the `vault_users`
  insert-bug theory in `identity.rs:153-163` with a live local run
  (`cargo run --bin signing-server`, hit `/identity/register`, check for a
  SQLite error in logs). Fix the bug (correct table name, or point at the
  right one) if confirmed — this blocks trusting any KYC gate built on it.
- Read `backend/src/signing/routes/lichess_oauth.rs` in full to confirm the
  PKCE-verifier-generation assumption in decision #3 above, and get the exact
  request/response shapes of `/init` and `/exchange` to wire the native
  client against verbatim (don't re-derive from `lila` source — the backend
  already adapted it).
- Confirm `JoinLinkPlugin` (from `src/multiplayer/join_link.rs`) is actually
  registered in app setup (grep `JoinLinkPlugin` in `src/core/plugin.rs` or
  `src/lib.rs`) and that `P2PUIState.peer_input`/`validate_node_id()` aren't
  already wired to a hidden/dead UI path before building a new one.

## Phase 1 — Backend

1. **New migration `020_casual_games_and_sponsorship.sql`**, wired into
   `database.rs::run_migrations()` (follow the exact pattern of the
   `migration_0NN` blocks already there). Contents:
   - `casual_games` table: id, account identifier (wallet or email-subject),
     opponent type (`bot`/`local_p2p`), result, PGN, timestamp — no Elo
     column (per design decision #1).
   - A sponsorship-guard column/table for profile creation, e.g.
     `profile_sponsored_at` on `users_v2` (nullable timestamp) — set on first
     sponsored `init_profile`, checked to block repeat sponsorship.
2. **New route: sponsored profile creation.** Either extend
   `init_profile_tx` (`backend/src/signing/routes/auth.rs:636-746`) with a
   `sponsored: bool` request field, or add a sibling route
   `init_profile_sponsored_tx`. Must: check `kyc_status` (Phase 0's
   reconciled source) is verified, check `profile_sponsored_at IS NULL`,
   build the tx with the backend keypair as fee payer / position-0 signer
   (per decision #2), set `profile_sponsored_at` after successful build (or
   after confirmed submission — decide based on whether double-spend risk
   matters here; recommend setting it optimistically at build time with a
   cleanup job if unconfirmed, matching whatever pattern
   `link_external_elo`'s backend-signs flow already uses for idempotency).
3. **New route: casual game recording.** `POST /api/games/casual` (name
   TBD), JWT-authed via `authed_wallet`-equivalent (extend to accept
   email-subject JWTs too, not just wallet ones — check `authed_wallet`
   actually already handles `"email:<addr>"` subjects since JWTs are issued
   with that subject shape today; likely just works, verify). Inserts into
   `casual_games`. No on-chain call.
4. **KYC gate integration** — wire into route #2, using the Phase-0-verified
   authoritative KYC source.

## Phase 2 — Solana program

- No new instruction (per decision #2). Add the **first-ever
  instruction-level test for `init_profile`**: `programs/xfchess-game/tests/init_profile_sponsored_tests.rs`,
  modeled on the `ProgramTest`/`BanksClient` harness already used by
  `game_settlement_tests.rs` (not `profile_session_tests.rs`, which is a
  pure unit test with no `ProgramTest` — confirmed no existing test exercises
  `InitProfile` end-to-end today). Test: backend keypair funds + is fee payer,
  player keypair signs, assert the resulting `PlayerProfile.authority ==
  player.pubkey()` (not the backend's) and rent came from the backend's
  balance delta, not the player's.

## Phase 3 — Native client (Bevy/egui) — the largest real gap

1. **Connect Lichess button** in `src/ui/account/auth.rs`, next to "LOGIN
   WITH WALLET" (~line 314-331). Wire to backend's existing
   `/api/auth/lichess/init` (get `authUrl`) → open system browser (reuse
   whatever mechanism `perform_wallet_connect` uses to open the Tauri wallet
   popup, or a plain `webbrowser::open` since this doesn't need the Tauri
   wallet bridge specifically) → poll or listen for the callback → call
   `/api/auth/lichess/exchange`. Exact shapes confirmed in Phase 0.
2. **Guest tier**:
   - Add `pub is_guest: bool` (or equivalent) to `PlayerIdentity`
     (`src/states/main_menu.rs:930-976`) — today `display_name()` silently
     falls back to `"Guest"` on `username.is_none()`, which conflates "not
     logged in yet" with "chose Guest mode." Needs a real distinct state.
   - Local username picker: a simple text input + persistence, following the
     exact pattern of `src/multiplayer/network/identity.rs::load_or_create()`
     (read/generate/persist a file under `dirs::config_dir()/xfchess/`) —
     store as `guest_username` instead of a keypair.
   - Relax `src/puzzle/mod.rs`'s `wallet.trim().is_empty()` gate
     (`mod.rs:150-164`) for `PuzzleMode::Solve` only — leave `PuzzleMode::Earn`
     wallet-gated (it pays out). Guest solve requests can send the local
     Guest username or Iroh node ID in place of `wallet`.
3. **Direct Connection P2P UI** — new `NewMenuPanel::DirectConnection`
   variant, added to the panels dispatched from `render_play_online_panel`
   (`new_menu.rs:941-1016`) via `item_expandable_tip` (matching the existing
   Tournaments/Solana-Multiplayer entries at lines 1006-1015):
   - **Host**: reuse `render_host_p2p_config_screen` (`screens.rs:2189-2397`)
     — it already has room-name + time-control UI — but add a mode flag that
     skips the VPS-announce block (lines 2355-2371) and `p2p_vps_state`
     registration (2339-2347). Display the local node ID via
     `identity::node_id_b58()` using the existing copy-button idiom
     (`OutputCommand::CopyText`, pattern at `screens.rs:2442-2461`).
   - **Join**: new panel with a `TextEdit` bound to `P2PUIState.peer_input`
     (`p2p.rs` — exists but appears unwired to any current UI, confirm in
     Phase 0), validated via the already-written but unused
     `P2PUIState::validate_node_id()` (`p2p.rs:83-124`), firing
     `ConnectToPeerEvent { peer_node_id }` directly — this event is already
     fully wired end-to-end via `handle_connect_to_peer` in `p2p.rs`, so the
     UI is the entire remaining gap.
4. **Dual-Elo display** — `render_profile_panel` (`new_menu.rs:1489-1517`
   area) currently shows only `display_elo()` (on-chain). Add a second stat
   tile for `lichessBlitz`/`lichessVerified`, sourced from the same profile
   fetch that already returns these fields (per the on-chain struct) — likely
   needs a `lichess_elo`/`lichess_verified` field added to `PlayerIdentity`
   or wherever the fetched profile currently gets mapped into UI-visible
   state (`handle_auth_task`/`sync_profile` response handling).

## Phase 4 — Web frontend (`xfchessdotcom`)

1. **Guest bailout in `SignIn.tsx`** — add a "Continue as Guest" option to
   `IdentityStep` mirroring native's "No, I want to play local-online",
   skipping `ConnectWalletStep` entirely.
2. **Confirm `LichessLinkCard.tsx` is actually mounted** somewhere reachable
   (grep its usage — it may be defined but not rendered from any route).
   Wire it into the account/profile screens if not.
3. **KYC gate before `ProfileStep`'s "Initialize Profile"** — per the
   already-approved decision in `docs/plans/identity-separation-plan.md`
   (KYC required for on-chain profile creation), block `createPlayerProfile`
   until `Kyc.tsx`'s submission is complete, using Phase 0's reconciled KYC
   source via a new `getKycStatus`-style call in `xfchessdotcom/src/lib/api/kyc.ts`
   (extends the existing `getUserStatus` pattern).
4. Use the sponsored route from Phase 1 instead of player-funded
   `init_profile_tx` for first-time profile creation specifically.

## Phase 5 — Treasury naming cleanup

- Unchanged from `docs/plans/identity-separation-plan.md`: rename
  `host_treasury_pubkey` → `tournament_fee_recipient` (or similar), update
  `.env.example` files and `backend/src/signing/blinks/` call sites.

## Verification

- `cargo test -p backend` — new casual-game endpoint, sponsored-profile route
  (mock KYC states: verified/unverified/already-sponsored).
- `cargo test -p xfchess-game` — new `init_profile_sponsored_tests.rs`.
- Manual, native client: complete KYC, create a sponsored on-chain profile
  with zero SOL in the wallet; connect Lichess and confirm both Elo tiles
  render; play a bot game while logged in and confirm it lands in
  `casual_games` with no on-chain tx; log out, enter Guest mode, pick a local
  username, play a bot game and a Direct Connection game via pasted node ID,
  confirm zero backend calls (check server logs show nothing).
- Manual, web client: same KYC-gate and Guest-bailout checks in
  `xfchessdotcom`.
