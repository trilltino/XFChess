# Contracts deep-audit plan: execution record (2026-10-25)

Executes `XFCHESS_CONTRACTS_DEEP_AUDIT_PLAN_2026-10-25.md` against
`programs/xfchess-game` in this repository (the plan names it canonical).
Nothing was committed, deployed or signed. No key material was read. Every
number below comes from a command run in this session unless marked
**historical**.

## 0. Baseline

| Item | Result |
|---|---|
| Plan baseline | contracts `main` = `6c533c261497d42385a7c36b58044f27009f294c` (tip at review time) |
| Local `programs/` vs that commit | identical apart from CRLF/LF and one test `#[path]` (`client_layout_tests.rs`) |
| Local `programs/` vs deployed source `fb168600` | 10 test-only files (unwrap/expect safety gate) |
| Deployed devnet program | `8tevgsp…FGquyDU`, upgrade authority `C1vn2MT7…Ygkw` unchanged, last deployed slot 514786564 |
| Deployed code hash | first 1,152,400 bytes of ProgramData = `f60ce59fc4ac7cd02a83e520c54b75ea9d44c6127a986fa8efb01b3e431f74fe`, rest zero padding (re-dumped today) |
| **Reproducible build** | `git archive fb168600` + `cargo build-sbf 4.4.0 --arch v0 --tools-version v1.58 -- --locked` on this Windows machine gives **1,152,400 bytes, SHA-256 `f60ce59f…f74fe`, byte-identical to the deployed program** |
| Extraction repo build | same command on contracts `6c533c26`: 1,152,920 bytes (`3fa28e58…`), **+520 bytes**. The only printable-string differences are three embedded source paths (`src\on_chain*.rs` vs `crates\nimzovich_engine\src\on_chain*.rs`): the engine is a git dependency in XFChess and a path dependency in the extraction, so the binaries are not interchangeable as release evidence |
| IDL | `xfchessdotcom/src/lib/xfchess_game.json` is byte-identical to contracts `idl/xfchess_game.json`; 82 instructions |
| Client builders | `src/solana/program_interface/instructions.rs` identical to contracts `clients/rust/instructions.rs` |
| `AGENTS.md` | none exists in either repository |
| Working-tree program (this session's changes, pinned toolchain) | 1,156,240 bytes, `e6ab6731…a358e`, +3,840 bytes (+0.33 %) over the deployed binary |

The 520-byte difference is attributed to path strings. I did not byte-patch
the binary to prove no other difference exists.

## 1. Findings

| ID | Severity | Disposition |
|---|---|---|
| S1 profile ownership | high (as reported) | **confirmed safe**. One production caller (`profile::handler`); the account is the seed-bound canonical PDA and is either created by the program or owner-checked before bytes are read. |
| **S1b** pre-funded profile PDA | low (availability) | **confirmed, fixed**. A plain transfer to a player's profile PDA made `create_account` fail, so `init_profile` failed for that player. Now transfer + allocate + assign when the address already holds lamports. |
| S2 session reset on renewal | medium (as reported) | **policy-accepted**, covered by SBF dispatch tests (active session cannot be reset, only the wallet can renew, rotated key is rejected, spend is capped exactly at the limit and cannot wrap at `u64::MAX`, withdraw cannot take rent). |
| S3 token wager mint | medium (as reported) | **dormant, no patch**. Every write to `Game.wager_token` is `None`; the token branch of `withdraw_expired_wager` is unreachable from any program-created Game. Retiring the branch is a policy decision. |
| S4 rollback recipients | medium (as reported) | **confirmed safe**. The Delegation Program at v3.1.0 / dlp 2.0.x checks `delegation_metadata.rent_payer` and, when a pending commit exists, `commit_record.identity`. Five SBF tests run the deployed devnet Delegation Program binary against substituted recipients and get its own error codes with zero mutation. Not verified: that the deployed binary is a reproducible build of that source, and the mainnet binary. |
| **S5** escrow dust blocks payouts | medium (griefing) | **confirmed, fixed**. Sending 1 to 890,879 lamports to a game's escrow PDA left it rent-paying after the pot was paid out, and the runtime rejected `finalize_game`. The same pattern existed in cancel, withdraw, dispute resolution, stale-dispute claim and recovery. Sub-rent dust is now swept: to the treasury where an account exists (finalize, cancelled settlement, resolve, recovery), otherwise to the refunded creator (cancel, withdraw) or the white seat (stale dispute). Balances at or above the rent minimum stay in the escrow. This is a policy choice (see section 6). |
| **S6** duplicate registration | high | **confirmed, fixed**. `register_player` checked duplicates only in the shards the caller supplied. In a 256-seat tournament a player seated in shard 1 could omit it, pass the check, and take a second seat. It also made `cancel_tournament` unusable (it rejects duplicates). `register_player` now requires every shard the tournament can use. |
| **S7** registration liveness | low | **confirmed, fixed**. The target shard came from `num_registered / 64`; after departures a full shard could be chosen while another had room (`TournamentFull` with 64 free seats). The first shard with room is now used; `start_tournament` already repacks shards. |
| S8 large-tournament cancel | medium (design) | **identified, not changed**. `cancel_tournament` needs one writable account per registered player (`remaining_accounts.len() == registered`) while a transaction can lock at most 64 accounts, so a paid tournament with roughly 50 or more registered players cannot be cancelled in one transaction. Registration-phase players can still `leave_tournament`; Active-phase refunds cannot be returned. Fixing it needs batched or pull-based refunds, which changes the ABI and is a design decision. |

Smaller items fixed under P4:

- `magicblock/crank.rs`: three lossy `u64 as i64` casts now `i64::try_from` with `InvalidArgument`.
- `initialize_tournament`: summing ten `u16` prize shares aborted on overflow; it now returns `InvalidGameStatus` like any other excess.

## 2. P1 reviews with no change needed

- **Settlement** (`lifecycle/settlement.rs`): payouts are computed from the pot, not the escrow balance, so donations are never paid out. New test: lamports are conserved across finalize for a win and for an odd-lamport draw, only the donation remains, a replay fails.
- **Recovery**: legacy `recover_stuck_delegation` always fails closed; `v2` pays only the identity bound before delegation. New test: `Waiting`, `Ready`, `Closed` and `Recovered` identities are rejected with no account change. The dust fix replaced the old "sub-rent excess fails" behaviour with a sweep; its test was updated.
- **Prizes**: shares are validated to sum to at most 10,000; `prize_amount` uses `u128`; claim and distribute set the place bit atomically with payment; mixed SOL/USDC pools are only claimable through the atomic individual claim. Property tests below cover the arithmetic.
- **Pass boundary**: foreign-owned, wrong-PDA, wrong-mint, revoked and double-use cases were already covered (`tournament_pass_tests`, `tournament_pass_bridge_tests`); the bridge tests now run locally with the hash-pinned pass fixture.
- **Moves**: new `move_authorization_tests` cover an unregistered signer, a disabled session, another game's session, the wrong side's session, a forged `next_board`, finished/settled games, the nonce ceiling, stalemate and the fifty-move rule. Every rejection leaves the Game byte-for-byte unchanged.
- **Profile age / username**: new tests for the exact 18-year boundary and `i64::MIN`, `-1`, `0`, `i64::MAX`, and a username owned by someone else.

Observations, unchanged:

- Elimination tournaments only fill places 1-4, so `prize_shares` for places 5-10 stay in escrow.
- Threefold repetition is not enforced on-chain.
- `GameStatus::Inactive` is never written; `Expired` games make `Game::phase()` return an error.
- `set_username` lets one wallet claim any number of names (rent-bounded squatting).
- All 95 `UncheckedAccount` rows are in `account-trust-matrix.md`; the ones that matter (`remaining_accounts` loops in cancel, distribute, register; optional token accounts in withdraw, cancel, claim) were read above and are consistent with their CHECK comments.

## 3. P2 tests added

| File | What it locks |
|---|---|
| `state_machine_tests.rs` | resign, claim_timeout, record_move, offer_draw and accept_draw are rejected from all 8 non-Active statuses without mutation; positive controls prove the rejections are about status. Table: `state-machine-20261025.md`. |
| `lifecycle_race_tests.rs` (model) | 16 seeded random sequences of join/cancel/resign/timeout/finalize, repeats included: lamports never created, a rejected step changes nothing, status never regresses. Replay with `LIFECYCLE_SEED`. |
| `engine_fuzz_tests.rs` | seeded random games (rejected move never mutates, board stays well formed, `Playing` implies a legal move exists, terminal outcomes have none) and 20,000 arbitrary 68-byte boards (no panic). Replay with `ENGINE_FUZZ_SEED`. |
| `balance_property_tests.rs` | prize shares never exceed the pool and lose under one lamport per place, monotonicity, place bits, `pot`/session total overflow, `has_budget` against both caps over `0 … u64::MAX`. Replay with `BALANCE_PROP_SEED`. |
| `abi_snapshot_tests.rs` + `snapshots/abi.txt` | all 82 instructions (discriminator, account metas in order, argument types) and six account sizes; regenerate with `UPDATE_ABI_SNAPSHOT=1`. |

The IDL-derived snapshot does not by itself prove the compiled program matches the IDL; four representative compiled discriminators are cross-checked.

Not run: the live local Agave + ephemeral-validator round trip (delayed commits, lost callbacks, validator crash and restart). It needs the local L1/ER stack on ports 18899/17799, which is not installed here. The rollback-after-timeout path against the real Delegation Program binary is covered instead (S4).

## 4. P3 measurements

Built with `cargo-build-sbf 4.4.0 --tools-version v1.58 -- --locked`; successful instruction calls from the full suite run with `--test-threads=1` (compute units, one program instruction each). These are **fixture samples, not worst-case baselines**; many instructions have one sample.

| Instruction | samples | min | median | max |
|---|---|---|---|---|
| CreateTournamentGameV2 | 1 | 77,632 | 77,632 | 77,632 |
| RegisterPlayer | 8 | 38,526 | 50,294 | 69,588 |
| ForceUndelegateAfterTimeout | 3 | 54,295 | 54,295 | 60,743 |
| SessionJoinGame | 1 | 56,694 | 56,694 | 56,694 |
| IssuePassEligibility | 7 | 41,663 | 47,587 | 56,587 |
| SessionCreateGameV2 | 1 | 53,685 | 53,685 | 53,685 |
| FinalizeGame | 21 | 32,727 | 43,227 | 52,296 |
| RecordMove | 5 | 19,666 | 19,769 | 52,176 |
| JoinGame | 17 | 29,189 | 35,189 | 41,189 |
| GlobalJoinGame | 1 | 39,239 | 39,239 | 39,239 |
| CancelTournament | 1 | 37,112 | 37,112 | 37,112 |
| InitProfile | 6 | 20,580 | 29,050 | 36,386 |
| ClaimTournamentPrize | 1 | 35,374 | 35,374 | 35,374 |
| GlobalCreateGameV2 | 5 | 32,210 | 34,998 | 35,210 |
| BindTournamentPass | 15 | 28,007 | 29,507 | 34,007 |
| CancelGame | 17 | 18,024 | 22,543 | 27,598 |
| RecoverStuckDelegationV2 | 3 | 15,540 | 19,809 | 21,674 |
| Resign | 11 | 8,719 | 10,221 | 14,721 |
| ClaimTimeout | 4 | 9,513 | 9,595 | 11,095 |
| AuthorizeGlobalSession | 3 | 9,702 | 11,202 | 11,204 |

Binary: 1,156,240 bytes (deployed 1,152,400). Rent-exempt minimum ((account size + 8-byte discriminator + 128 bytes of account overhead) × 6,960 lamports): Game 365 B = 3,431,280; PlayerProfile 289 B = 2,902,320; RecoveryIdentity 90 B = 1,517,280; GlobalSessionDelegation 108 B = 1,642,560; SessionDelegation 92 B = 1,531,200; UsernameRecord 56 B = 1,280,640.

Not measured: heap use, per-instruction stack frames, worst-case CU (largest tournaments, deepest move scans), warm vs cold state.

**Build diagnostics.** `cargo build-sbf` prints 16 "overflows the maximum allowed frame space" errors, all in `crypto_common::hazmat::SerializableState` and `hybrid_array::from_fn` (sha2 0.11 state serialisation). They appear in the canonical `fb168600` build as well, so they predate this work. I did not establish whether any program path can reach those functions.

**Optimisations considered and rejected (decision log).**

- `cancel_tournament` `Vec` + `HashSet` and the `distribute` temporaries: no measured CU or stack problem; S8 shows the real limit is the 64-account lock, not compute. No change.
- Fixed-size prize ledger storage: ten places, trivial cost, no benefit worth an error-order risk.
- Reusing `Clock::get()`/`Rent::get()`: would reorder error timing for a few hundred CU. No change.
- `load_or_new_profile` signature: it is only unsafe if a future caller passes unchecked bytes; changing it only to quiet a scanner was ruled out by the plan.
- Feature selection: the deployable crate defaults to `cranks, move-validation, devnet`; `production` requires `move-validation` at compile time. `--no-default-features` without `production` drops the validator, which is by design for local builds. The engine is `no_std` with default features off; no host-only or debug dependency was found in the SBF closure.

## 5. P4 / P5

**P4.** Program source has no `unwrap`, `expect`, `todo!` or `unreachable!`; the three `panic!` are a compile-time mainnet guard and two test assertions. The `saturating_*` calls (about two dozen) were read: each is preceded by a `.min()`/`> 0` guard, is rent arithmetic, or is clock/score bookkeeping, so none hides a financial invariant. `as` casts reviewed; the only lossy ones were the crank casts (fixed). No structural refactor was made.

**P5.**

- `cargo audit` on the workspace lockfile: 11 vulnerabilities (crypto-bigint 0.5.5, curve25519-dalek 3.2.0, ed25519-dalek 1.0.1, quick-xml 0.37.5/0.39.2, rkyv 0.7.46, rsa 0.9.10, ruzstd 0.8.2, webbrowser 1.1.0) plus 21 unmaintained/unsound/yanked warnings. Of these only `crypto-bigint` (via `solana-secp256k1-recover`) and `rkyv` (via `magicblock-delegation-program-api`) are in the `xfchess-game-sbf` dependency tree; the dalek versions are not. Reachability from program code was not re-analysed beyond the existing `sbpf-*-reachability-20261024.txt` records.
- CI (`.github/workflows/ci.yml`) edits: clippy now also runs over the test targets, the built program's SHA-256 and size are logged, and a hash-pinned Delegation Program fixture is fetched (`docs/security/fetch_dlp_fixture.py`, tested against devnet; pinned `1bc71771…b00f`, 502,240 bytes). The existing `cargo audit` job stays advisory.
- The contracts repository's own workflow (`contracts.yml`) is not edited here; it needs the same fixture step and `--tests` clippy if it is kept as a release gate.
- Deployment: none performed. Deploying this change set needs the usual review (authority, cluster, program ID, ProgramData hash, backend/client sync). No instruction data, account order, PDA seed, error code or account layout changed (the ABI snapshot test passes unchanged).

## 6. Decisions needed

1. **S5 dust destination.** Treasury where an account exists, creator/white seat elsewhere. Alternative: leave dust and let players top the escrow up (the previous behaviour, a griefing vector).
2. **S8 large-tournament cancel.** Needs a batched or pull-based refund design.
3. **S3.** Retire the unreachable token-wager branch of `withdraw_expired_wager`, or keep it and add the mint check.
4. Elimination prize places 5-10: reject such `prize_shares` at initialization, or return the remainder to the host at close.

## 7. Verification

Final run, program built with the pinned toolchain: `cargo test -p xfchess-game --no-fail-fast -- --test-threads=1` gives **301 passed, 0 failed, 4 ignored** (the ignored tests need live network or the local ER stack). `cargo clippy -p xfchess-game --tests -- -D warnings` is clean. The baseline at the start of the session was 263 passed with 11 pass-bridge tests failing because the pass fixture was absent; it is now fetched and hash-verified (`06a784b0…57ff`).

Commands run (all read-only against public devnet): `solana program show/dump` for the game program, the Delegation Program and the pass program; `git archive` + `cargo build-sbf` for the canonical and extraction builds; `cargo test`, `cargo clippy`, `cargo audit`, `cargo tree`, `rustfmt`.
