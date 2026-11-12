# Devnet reset, admin panel and tournaments end to end

Status: plan, nothing below is built unless marked. Devnet only; mainnet is out of scope except
where a step keeps devnet tooling out of a mainnet build.

## Goals

1. Wipe and rebuild devnet state on demand, with a clear record of what was removed.
2. An admin panel that shows every on-chain account with its owner, username and email, and can
   close accounts.
3. Tournaments proven end to end on devnet, in both formats, including the failure cases.

## Decisions

| Decision | Choice | Status |
|---|---|---|
| Account table name | `users` (was `users_v2`); legacy pre-wallet table kept as `legacy_users` | Done; the upgrade test passes |
| Email to wallet link | Verification link, sent through Resend | Agreed |
| Program ID | Fresh ID `6idpQeH5YzXUT4sg8mG8dzxdK7ytmxfEyWkzUcNJJv4N` (keypair in `keys/xfchess_game-program.json`), no migration | Chosen; replaced everywhere in the source, not deployed |
| Reset tool | `admin_close_account`, opt-in `devnet-reset` feature, lamports only to the signing authority | Written, compiles in all feature combinations, ProgramTests not written |
| Authority key | Held on the admin's machine; the panel signs locally, the backend never holds it | Proposed |

Why a fresh program ID: the `PlayerProfile` layout changed, the contract still carries both
legacy and `*_v2` instructions (`create_game` and `create_game_v2`, and so on), and Elo units
changed. One clean break is cheaper than migrating 170 old profiles.

## Phase 1: contract rebuild (devnet)

1. Drop the legacy instructions that have a `_v2` twin, and rename every `*_v2` instruction,
   account struct and client builder to the plain name (the `cu_benchmark` tool, the ABI snapshot
   and the website IDL change with them).
2. Fix Elo units in the tournament path. The contract's ratings are now rating x 100; the backend
   join route defaults to 1200 and stores tournament Elo ranges in plain points. Check how
   `register_player` and the pass policy compare them and make one unit end to end. Unverified.
3. Keep `admin_close_account` behind the `devnet-reset` feature. Add ProgramTests: a non-authority
   signer is refused, `ProgramConfig` is refused, a delegated account is refused, lamports land in
   the authority wallet, a closed account can be recreated.
4. Run `scripts/verify/assert_no_admin_close.py` on every deployed `.so`: a normal build must not
   contain the instruction, the reset build must.
5. Build the SBF binary, deploy under a new program keypair, call `initialize_config`, run
   `verify_devnet_program.ps1`, regenerate the IDL and the ABI snapshot.
6. Replace the old ID everywhere (`git grep` list is in `docs/DEVNET_RESET.md`), then close the
   old program to recover its rent.

Gate: all contract tests pass on the SBF build; the deployed hash matches the local build.

## Phase 2: backend, identity and admin API

1. `signups` table in `sessions.db` (email, wallet nullable, `email_verified_at`, token hash,
   expiry), replacing the flat `subscribers.jsonl`. Idempotent migration like the others.
2. `POST /api/account/email`, authenticated with the wallet JWT: stores a single-use token (hash
   only, 24 hour expiry, rate limited), sends the verification link through Resend, and links the
   email to the wallet when the link is opened. Website sign-up without a wallet stays as it is
   (a welcome email, no link).
3. Admin routes behind the admin key:
   - `GET /admin/chain/summary`: counts by account type and SOL held.
   - `GET /admin/chain/accounts`: paged, decoded by IDL discriminator, joined to `users` (wallet,
     username) and the verified email.
   - `GET /admin/chain/wallet/{wallet}`: accounts plus recent transactions for that wallet.
4. Privacy: emails never on-chain or in logs; masked by default; each reveal writes a row to the
   vault `audit_log`.
5. Confirm the `users` rename with the migration test and a run against a copy of the real
   `sessions.db`.

Gate: backend tests pass, including the rename upgrade test and an email-verification test with a
mock Resend (the pattern in `tests/signup_email.rs`).

## Phase 3: admin panel (Tauri, `tournament-admin` feature, off in release)

- Banner with cluster and program ID; closing is disabled unless both are devnet.
- Overview: counts by type, SOL held, accounts with no linked user.
- Accounts table: filter by type, username, wallet, email; detail drawer with decoded fields, the
  related set (profile, username record, delegations) and recent transactions.
- Close flow: dry-run list, warnings (escrow lamports, live games, delegated accounts), related
  accounts selected together, typed confirmation, batched execution with progress, results.
- A JSON snapshot of every account before any wipe. Each close writes an audit row with the
  username and email as they were at that moment.
- Signing happens in the panel with the local authority keypair.
- Tournament controls (create, start, cancel, set round deadline, approve prize release) move
  into the same panel.

Gate: a wipe against a throwaway devnet deployment, then the count script shows only the config.

## Phase 4: tournaments end to end on devnet

How it works today is in the chat summary: backend authority creates the tournament accounts,
players register on-chain, the scheduler starts it, `create_tournament_game` seats each pairing,
the orchestrator records results on-chain, the forfeit watcher acts after 3 minutes offline, and
the distributor pays prizes after the anti-cheat gate. Sizes are 2, 4, 8, 16, 32, 64, 128, 256.

Scenarios, each with the on-chain state checked against the backend state:

| # | Scenario | Checks |
|---|---|---|
| 1 | 2-player single elimination | register, start, game seated, result recorded, prize paid |
| 2 | 4-player single elimination | bracket advance, winner moves to next match |
| 3 | 4-player Swiss, 2 rounds | pairings, board results, round advance, completion with top places |
| 4 | 8-player Swiss, 3 rounds | bye handling, tiebreak order, prize ledger |
| 5 | Draw in single elimination | colours swapped replay, higher seed after the replay limit |
| 6 | Player offline for 3 minutes | auto forfeit, bracket continues |
| 7 | Cancel before start | every entry fee refunded, escrow closed |
| 8 | Private tournament, wrong password | refused; right password admitted |
| 9 | Elo range | below and above the range refused, in range admitted (needs the unit fix) |
| 10 | Pass-gated tournament | no pass refused, revoked eligibility refused |
| 11 | Pool over 5 SOL | held until the admin approves |
| 12 | Scheduled start and fill-grace | starts at the time; starts after 5 minutes at the minimum |
| 13 | Restart mid-round | the reconcile pass repairs state |

Needs a harness: something that plays the games without a person at the board. The repo has the
`cu_benchmark` tool and `swiss_tournament_e2e.rs` (in-memory, no chain); a devnet bot harness is
the main missing piece. Open: build it on `cu_benchmark` or as headless clients.

Gate: scenarios 1 to 7 pass on a fresh devnet program; 8 to 13 pass or have a filed defect.

## Order and dependencies

1. Decisions (fresh ID, key custody). Nothing else blocks on these except Phase 1.
2. Phase 2 steps 1 to 4 and Phase 3 read-only views can start without the contract rebuild and are
   useful against the current devnet.
3. Phase 1 before the close flow and before Phase 4.
4. Phase 4 last; it uses the panel as the operator console.

## Risks

- `admin_close_account` can take escrow lamports without a user signature. Mitigations: opt-in
  feature that cannot combine with `mainnet` (compile error), lamports only to the authority, the
  `.so` assertion on every deploy, a short deploy window, panel confirmation and audit trail.
- One authority key signs tournament creation, game seating, results and prizes. Keep it off the
  backend host for the reset tool; for tournaments it must stay online.
- Email and wallet links are personal data: store the least, mask by default, audit reveals.
- The Elo-unit mismatch could invalidate scenario 9 and rated ranges generally.

## Open questions

1. Fresh program ID: yes or no?
2. Does the devnet bot harness build on `cu_benchmark` or run headless clients?
3. Should the Tauri panel keep a local copy of the snapshot, or write it to a chosen folder?
