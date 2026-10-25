# XFChess: contract security audit and minimal-remediation execution prompt

> Copy everything under **Execution prompt** into a coding agent with repository access. This document records verified findings, the owner's confirmed security policies, the upstream versions checked on 23 October 2026, and a full end-to-end audit plan. It is deliberately conservative: never invent a source of truth for an access-control decision.

**Audit basis:** [`trilltino/xfchess-contracts`](https://github.com/trilltino/xfchess-contracts) at `25c1b31b1e8d2692189b795e694caac8a3959658` (22 October 2026), the then-current `main`. Source of truth and deployed implementation are in [`trilltino/XFChess`](https://github.com/trilltino/XFChess), `programs/xfchess-game`. The extraction's `DEPLOYED.md` identifies deployed XFChess commit `70a25c5c4e2d2fef18bb30f54f4260a74e27fadb`, devnet program `8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU`, and a non-byte-identical rebuild. Do not assume that changing the extraction changes the deployed binary, web clients, backend, or full XFChess repository.

**Current review branch:** `security/account-validation-20261023` in `trilltino/xfchess-contracts`. It has four deliberately narrow source edits: profile PDA constraint, checked wager expiry, canonical undelegation buffer/System Program, and runtime Game PDA verification. They have **not** been SBF-compiled, tested, merged, or deployed as part of this audit. Review these changes and add tests before deciding to merge. Do not copy patches blindly if the branch moved.

### Confirmed owner decisions (23 October 2026)

1. **Emergency recovery must pay only the original verified players.** The dispute authority must never be able to redirect the original wager to arbitrary recipient accounts. If the Game account is empty and no authenticated original-player record or verifiable proof is available, the program must **fail closed and preserve escrow**. An administrator's signature alone is not proof of player identity. Implement a durable binding or an equivalent cryptographically verifiable proof; handle legacy games separately and require approval before creating new account schemas.
2. **Global-session spending caps are per wallet-signed authorization.** A fresh, explicit authorization signed by the player's own wallet starts a new allowance and therefore resets `total_spent` to zero. This is intended behavior. An existing active session must not be silently reset, and a session key alone must never renew its allowance. Keep the existing budget semantics and prove them with instruction-level tests.
3. **Every dependency must move to its latest stable release.** This is a mandatory end state of the **separate dependency-upgrade workstream**, not an invitation to update version strings without compiling. Upgrade the application code, feature flags, generated interfaces, tests and build tooling as needed to support current stable releases. If the newest versions cannot coexist safely, demonstrate the incompatibility and mark the upgrade blocked. Do not quietly leave packages on older versions while calling the migration complete.

## Execution prompt

You are acting as an independent senior Solana/Anchor security engineer reviewing a real-money chess program. Work on the actual repository, not a fictional reconstruction. Preserve the deployed program ID, instruction discriminators, account ordering, serialized account layouts, event formats, seed prefixes, externally consumed error codes, and business behavior unless a confirmed vulnerability requires a specific change. Treat every change in payer or recipient authority as high-risk. The owner has decided that recovery payouts must go exclusively to authenticated original players, and session spending allowances reset only with fresh player-wallet authorization. Those policy choices are binding, but they do not establish the technical existence of recovery data. When provenance cannot be proved, fail closed instead of guessing.

### Non-negotiable rules

1. Start by checking repository identity, branch, clean working tree, `AGENTS.md`/local instructions, and the exact reviewed and deployed commits. Compare `xfchess-contracts`, `XFChess/programs/xfchess-game`, deployed metadata, and current on-chain code hash. Report divergences before patching. Do not edit unrelated desktop, backend, or UI code just because it looks untidy.
2. Address verified security defects in severity order. Make the smallest locally understandable patch. Do not rename types, reorder account declarations, redesign modules, mass-format files, or add generic validation frameworks as part of a security-only change.
3. Maintain a per-instruction **account-trust matrix**: declared account type, signer, mutability, expected key/PDA seeds, owner, executable constraint, payer, recipient, CPI target, and any handler-side checks. For each `UncheckedAccount`, state the actual reason it cannot safely be typed; do not equate its mere presence with a vulnerability.
4. Before editing, write a concrete exploit or failure hypothesis with attacker capabilities, prerequisite account ownership, signature requirements, instruction calls, fund/state effect, and why existing checks fail. If any step cannot be established from code or a reproducible test, label the suspicion unconfirmed.
5. For every accepted fix add a negative test that fails before the fix and passes after, plus a positive test proving the legitimate instruction still works. For lamport/token paths, check balances and conservation after failure, success, retry, and concurrent invocation where feasible.
6. Keep security patches and dependency upgrades in separate commits/PRs. Recompute CU usage, SBF binary size, and account rent after dependency updates. The target is the latest published **stable** release of every contract-workspace dependency, including build/test dependencies and indirect lockfile packages. Resolve compatibility through targeted code migrations and updated dependent packages; if impossible, document the precise blocker and mark completion blocked. A successful host build does not prove ER runtime behavior.
7. Do not touch deployment secrets, keypair JSON, live authority accounts, devnet/mainnet upgrades, privileged transfers, or migrations without explicit approval. Never print private keys. Default to a draft PR with a verification log.
8. Treat the SDK and client transaction builders as part of the security boundary. Test exact account-meta order and ABI compatibility using `instruction_layout_tests` and client fixtures.

### Prioritized findings from the reviewed snapshot

#### P0-A. `recover_stuck_delegation.rs`: recipient identity cannot currently be proven

**Reported as HIGH:** `RecoverStuckDelegation.white_authority` and `.black_authority` are mutable `SystemAccount`s without equality checks against `game.white`/`game.black`.

**Verified:** The handler first checks `*game.owner == crate::ID` and `game.data_is_empty()`. It then splits **all lamports in the per-game escrow** between whatever two recipient accounts the caller supplies, and transfers any remaining lamports in the empty game account into the treasury. It requires the hard-coded dispute authority to sign. `ClaimStaleDispute` and `ResolveDispute` do constrain payouts to `game.white`/`game.black`, but those instructions deserialize a nonempty `Account<Game>`. Adding the same `constraint = key == game.white/black` to the recovery accounts is **not possible** while the game is empty. The existing `er_recovery_tests.rs` fixtures explicitly manufacture an empty Game PDA and arbitrary recipients. The trust concern is real, but an attacker without the dispute authority cannot invoke this path.

**Confirmed policy:** Emergency recovery **must pay only the original verified players**. This is no longer a product-intent question. The existing privileged handler does not satisfy that policy simply because the dispute authority must sign. Do not write an invented constraint or treat an administrator-supplied account address as proof.

**Required implementation investigation, in order:**

1. Trace `request_force_undelegate`, `force_undelegate_after_timeout`, canonical buffer restoration, account snapshots, `StuckDelegationRecovered` and the exact path to `game.data_is_empty()`. First look for an existing trustworthy program-owned record that binds the exact game ID to both original player addresses. Read the record's creation, mutation and closing paths, not just its struct definition.
2. If an existing record suffices, add the smallest explicit address/owner/PDA checks connecting `white_authority` and `black_authority` to that record. Test that even the legitimate dispute authority cannot provide substitute recipients.
3. If no such record exists, design an immutable or tightly governed program-owned recovery identity PDA keyed by `game_id`, or an equivalent on-chain cryptographically verified proof. Bind the original white player when the game is created and the original black player when the opponent joins. Determine when that record is finalized before delegation; prevent replacement, post-delegation tampering, cross-game reuse, and premature closure. Assess its rent, upgrades, signer requirements and account/IDL impact. Obtain explicit approval for this necessary protocol extension before coding it.
4. For **already deployed or already wiped games**, do not fabricate historical player identities. Establish whether authenticated chain history and transaction proofs can be verified by the program or a specifically approved recovery process. Off-chain explorer data, operator notes, a database entry, or a dispute-authority signature by itself does not make a recipient cryptographically verified. If the identity cannot be established to the required trust standard, return an error and leave escrow untouched until a verifiable migration/recovery method is approved.
5. Define the reserved black-player value before join, zero-wager games, identical recipient keys, malicious extra lamports in escrow, rent and rounding leftovers, and repeat recovery attempts. Reject duplicate payout destinations for a two-player game unless the stored canonical identities genuinely coincide and that state is valid under game rules. Never silently split a two-player pot into an arbitrary account.
6. Ensure that the verification precedes **all** escrow payment and game-rent movement. Test that rejection is atomic and that both verified recipients receive only the correct allocation. Reconcile signed governance action, immutable identities, emitted event fields and monitoring.

**Required outcome:** A negative test where the real dispute authority signs but substitutes either recipient **must fail without changing balances**. A positive test with authenticated original players must succeed. If durable identity is unavailable, the handler must fail closed. Mark this item **BLOCKED: MISSING VERIFIED HISTORICAL IDENTITY SOURCE** until the necessary account/proof design and migration are approved and implemented; never call the security issue resolved on the strength of the admin signature alone.

#### P0-B. `account_ix/profile.rs`: independently discovered arbitrary account substitution

**New HIGH, confirmed by source review:** `InitProfile.player_profile` was a mutable `UncheckedAccount` without a PDA seed constraint. In its nonempty branch the handler only tested `profile_info.owner == ctx.program_id`. If the discriminator did not match `PlayerProfile`, `profile_init::load_or_new_profile` constructed a default profile, and the handler could `resize` and overwrite account data. An attacker signing as `player` could therefore present a different writable account owned by the XFChess program, such as a Game or tournament account, and try to corrupt its data. This is a separate account-substitution path even though legitimate Profile accounts have an `authority` field.

**Minimal patch already proposed on review branch:**

```rust
#[account(mut, seeds = [PROFILE_SEED, player.key().as_ref()], bump)]
pub player_profile: UncheckedAccount<'info>,
```

**Tests:**

- Reject a program-owned Game PDA supplied as `InitProfile.player_profile`, without changing a single byte or lamport.
- Reject a different player's Profile PDA even when the caller is paying rent.
- Accept initialization of the caller's canonical **empty** Profile PDA.
- Accept legitimate re-initialization/resizing of that exact canonical Profile PDA without resetting verified status, rating, game counters, or other accumulated state.
- Reject a wrong-owner nonempty account. Test the username record remains unchanged when the instruction fails.

**Additional cross-check:** Search every manual `resize`, `assign`, `allocate`, `Discriminator` write, `try_borrow_mut_data`, and `try_deserialize`. For each writable `UncheckedAccount`, prove identity before modifying it. Do not replace the current manual sponsored-initialization flow until ABI/rent semantics are understood.

#### P1. `lib.rs:process_undelegation`: callback must only reconstruct intended Game PDA

**Reported as HIGH:** `InitializeAfterUndelegation.base_account` is unchecked, and `process_undelegation` forwards it plus caller-supplied `account_seeds` to the SDK CPI.

**Verified:** The struct exists in `delegation_ix/undelegation.rs`. `lib.rs:554-574` calls MagicBlock's `undelegate_account`. The exact `ephemeral-rollups-sdk` **v0.16.2** implementation checks `buffer.is_signer`, `buffer.owner == DELEGATION_PROGRAM_ID`, and canonical undelegation buffer derivation before copying. It calls `create_pda` with signer seeds derived from `account_seeds` and the calling program ID; the Solana runtime verifies the PDA's effective signature. However the app previously did **not** restrict the requested PDA class to `GAME_SEED || game_id_le_bytes`.

**Minimal patch already proposed:** Before the CPI, require exactly two seeds (`GAME_SEED` and an 8-byte game ID), derive that Game PDA under `ctx.program_id`, and require the supplied `base_account.key()` to equal it. This preserves the `process_undelegation(account_seeds: Vec<Vec<u8>>)` wire signature, existing account order, and the SDK's internal lifecycle.

**Do not:** Declare `base_account: Account<Game>` or check that it is already owned by XFChess during the callback. The delegation protocol can temporarily change account ownership and recreate state. Do not invent a `game_id` instruction argument; that would alter the ABI.

**Tests:** Reject wrong PDA, empty seed vector, extra seed, wrong seed prefix, wrong game ID length, wrong buffer, buffer with wrong owner, and unsigned buffer. Test actual delegate → commit → undelegate → deserialize Game with both the pinned v0.16.2 SDK and candidate upgrade. Test replays, callback failure atomicity, rent reimbursement, and Game PDA ownership restoration.

#### P2. `delegation_ix/undelegation.rs`: callback account typing

**Reported as MEDIUM:** Four bare `UncheckedAccount`s, no Anchor-level constraints.

**Verified:** The SDK's v0.16.2 `#[ephemeral]` generator emits a similar struct, but it adds a canonical buffer PDA seed constraint and pins the System Program ID. The downstream `undelegate_account` also authenticates the buffer and fails through System Program CPI when payer signature is needed to move rent lamports. It is incorrect to call this completely unvalidated.

**Minimal patch already proposed:**

```rust
#[account(
    seeds = [b"undelegate-buffer", base_account.key().as_ref()],
    bump,
    seeds::program = ephemeral_rollups_sdk::id(),
)]
pub buffer: UncheckedAccount<'info>,

// Preserve SDK callback payer semantics; downstream System CPI enforces
// required authorization if rent lamports must be transferred.
#[account(mut)]
pub payer: UncheckedAccount<'info>,

pub system_program: Program<'info, System>,
```

**Do not blindly change `payer` to `Signer` or `SystemAccount`.** Under SDK `create_pda`, no payer lamport transfer is performed if the target already has adequate balance. Forcing a signature in that path could change valid callback behavior; requiring system ownership could exclude a valid delegated/ephemeral payer. Verify actual MagicBlock invocation semantics and construct a positive test before tightening it. There is no Sysvar in this struct; `system_program` should be a typed `Program<System>`, not a `Sysvar`.

#### P3. `account_ix/global_session_ix.rs`: spending-limit reset on renewal

**Reported as MEDIUM:** `AuthorizeGlobalSessionCtx.session_delegation` uses `init_if_needed`. The handler rejects an unexpired enabled session with remaining games, but an expired, exhausted, or revoked session can be authorized again, and `delegation.total_spent = 0` resets the counter.

**Confirmed policy:** Each *new authorization signed by the player's wallet* gets a fresh spending allowance. A player-wallet signer selects the new session key, duration, spending limit, max wager, game allowance, and any deposit. `GlobalCreateGame` and `GlobalJoinGame` check `session.is_valid`, cap spending via `session.has_budget`, and add to `total_spent` with checked arithmetic. Therefore the `delegation.total_spent = 0` assignment during valid renewal is **intentional**. Do not change it into a lifetime counter.

**Required checks and tests:** Confirm an existing active authorization with remaining games cannot be reset or replaced without the allowed revocation/expiry/exhaustion transition; each new authorization requires the real player wallet signature; a session key alone cannot call authorization or forge the wallet signer; an old session key is invalid after rotation; and a fresh authorization starts at `total_spent == 0` with the exact new spending and game limits. Show that an old key cannot spend leftover funds after rotation and that exhausted sessions cannot bypass authorization by retrying a game instruction. The wallet may explicitly reauthorize after expiry or exhaustion. Preserve deposits and unused lamports according to existing withdrawal rules. Test exact exhaustion/expiry boundaries and maliciously duplicated account metas.

**Extra checks:** Compare `fee_vault_ix.rs`'s separate PlayerSession (seeded by session key) and tournament session authorization (separate PDA, `init`), including client assumptions and whether `withdraw_global_session` is intended to work while a session remains active. A player-authorized withdrawal of unused funds is distinct from authorization of a *session key* to withdraw funds.

**Current status:** **POLICY CONFIRMED: KEEP RESET**. Treat the original finding as a security-test/documentation item unless a reproducible bypass of the wallet signer or session limits is found. Do not add a permanent cumulative cap or change the account layout.

#### P4. `account_ix/withdraw.rs`: unchecked expiry addition

**Reported as MEDIUM; confirmed:** `game.created_at + 86400` was unchecked. The initial Game creation path sets `created_at` from `Clock::get()?.unix_timestamp` via `init_game_fields`, also used by global creation and tournament-game creation. No independent writer to this field was found in the inspected program source, so extreme timestamps are unlikely under legitimate operations, but delegated commits and future migrations warrant defense.

**Minimal patch already proposed:**

```rust
let expiration_time = game
    .created_at
    .checked_add(86400)
    .ok_or(GameErrorCode::ArithmeticOverflow)?;
```

**Tests:** Exact 24-hour boundary, one second after expiry, `i64::MAX - 86_399`, `i64::MAX`, negative or damaged historical timestamps if constructible, wrong signer, wrong Game PDA, and already withdrawn/settled state. An overflow must return the specified error without touching wager escrow or status. Avoid changing the `now > expiration_time` rule to `>=` inadvertently.

### Full instruction and state-machine audit

Analyze all **117 Rust source files under `programs/xfchess-game/src`** in the reviewed snapshot, then the test harness and transitive vendored on-chain engine. Do not claim 100% test coverage merely from reading them. The snapshot contains about 73 deployed instructions according to `DEPLOYED.md`. Generate a registry from `lib.rs` and compare against the binary-discriminator list and IDL.

1. **Accounts and authority**: Check all `#[derive(Accounts)]` contexts. For each account, enumerate `Account`, `SystemAccount`, `Program`, `InterfaceAccount`, `Signer`, `UncheckedAccount`, `AccountLoader`, `Option` and `remaining_accounts`; enforce correct PDA seeds, bumps, owner, executable/program ID, writable flag, signature, and `has_one`/equivalent. Check every `init_if_needed` path for takeover, discriminator confusion, re-initialization or reset, existing-account resizing, aliasing of mutable accounts, and incorrect account-closure destinations. Cover admin/key authority constants, disputes, treasury, KYC, profile sponsor, session delegation, tournament host, eligibility/pass hook, and the MagicBlock callback.
2. **Fund conservation**: For every `system_program::transfer`, `invoke_signed`, direct `try_borrow_mut_lamports`, SPL Token / Token-2022 `transfer_checked`, `burn`, and account `close`, trace authority, destination, exact amount, duplicate invocation, rent reserve, and transaction atomicity. Build tables for wager escrow, dispute bonds, recovery, global-session deposits, tournament SOL guarantees and entry fees, USDC prize ATA, and fees. Explicitly prevent cross-game, cross-tournament, and cross-mint substitution. Check whether any prize claimed via one instruction can be claimed again via another. Check `prizes_claimed` bitmaps, payout types, and remainders.
3. **Game lifecycle**: Enumerate legal transitions of `GameStatus`, `GamePhase`, `is_delegated`, `GameResult`, and dispute status. Test create, join, global-session create/join, delegate, move, offer/accept draw, resign, timeout, cancellation, force recovery, ordinary undelegation, finalize, expired wager, stale dispute, and dispute resolution. Include transitions while state is stale on the base chain, partial rollback, ER validator disappearance, duplicate keeper calls, and simultaneous settle/cancel/timeout races.
4. **Chess correctness and replay**: Check `move-validation` for *all production builds*, authoritative on-chain board transitions, UCI parsing, illegal moves, castling, promotions, en passant, check/mate, draws, repetition/fifty-move handling if represented, `nonce`, `parent_nonce`, `move_count`, and clock rules. Compare on-chain Nimzovich move validation with `xfchess-rules` and a trusted reference implementation over deterministic and randomized perft fixtures. Never trust `next_board` in production without comparison against the move engine. Prove a session key cannot move for another player or another game.
5. **Time and arithmetic**: Inventory every `+`, `-`, `*`, cast, `saturating_*`, `unwrap*`, division and array index driven by instruction parameters or account state. Use `checked_add`, `checked_sub`, `checked_mul`, and fallible conversions where overflow changes funds or permission decisions. Distinguish deliberate saturating counters from silent truncation. Audit `now - date_of_birth` in `profile.rs`, `now - game.updated_at` in `cancel.rs`, `total_shares = prize_shares.iter().sum()` in tournament initialization, tournament round/board math, and time conversions. Do not patch harmless arithmetic without a concrete reason.
6. **Governance and emergency operations**: Reconcile `recover_stuck_delegation`, `claim_stale_dispute`, `dispute`, `resolution`, `resolve`, `request_force_undelegate`, `force_undelegate_after_timeout`, manual treasury withdrawal, update ELO, and all fixed-authority instructions. Prove the relationship between requested `game_id`, stored `game.game_id`, `dispute.game_id`, PDAs, challenger, bonds, participants, and treasury recipient. Check hard-coded key role separation, leak/rotation plans, multisig options, audit events and incident playbooks. Do not conflate a trusted admin with an arbitrary unprivileged attacker.
7. **MagicBlock trust boundary**: Read the *exact pinned* `ephemeral-rollups-sdk=0.16.2` code, the delegation-program API, generated macro behavior, runtime callback contract, and newer 0.17.3 release. Check canonical undelegation buffer, signer and owner, program IDs, seed derivation, buffer size, account ownership transitions, final committed state, validator rollback, rent payer, and commit frequency. Reproduce delegate/undelegate/recovery failures on a supported local ER validator before accepting callback constraint changes.
8. **Tournaments and Token-2022**: Cover registration, shard indexing/duplicates, start and cancel, Swiss and elimination rounds, match creation/result authenticity, pairing reassignment, permissionless advancement, session create/join, tournament prize calculation and redirection, pass binding, pass-eligibility record ownership and mint authority, burn/hook behavior, and on-chain mint/program selection. Assert only funded prizes are paid and that an NFT/pass cannot be reused through alternate accounts.
9. **CPI and token-program correctness**: All CPIs must target the expected program, enforce token mint/decimals and source/destination owner or ATA constraints as required, and use signers only for the intended PDA. Audit optional token accounts and disabled branches. Examine the `WithdrawExpiredWager` token path: normal Game initialization sets `wager_token = None` in the inspected code; prove whether another reachable production instruction ever writes it before declaring the token path dead or changing it.
10. **Deployment and upgrade authority**: Check `Anchor.toml`, `Cargo.toml`, `Cargo.lock`, SBF shim, feature gates (`production`, `mainnet`, `devnet`, `localnet`, `cranks`, `move-validation`, `no-entrypoint`), IDL artifacts, keys/constants, `DEPLOYED.md`, and security.txt. Verify deployed devnet program upgrade authority and source provenance through independent chain data when network access exists. Do not present the extraction build as reproducible: `DEPLOYED.md` explicitly documents a 440-byte difference from the deployed binary and incomplete attribution.

### Dependency and toolchain workstream (separate PR)

**Mandatory objective: migrate every dependency in the contract workspace to its latest stable published version, not merely the newest version accepted by the old constraints.** This covers direct dependencies, transitive lockfile dependencies, optional and feature-gated crates, on-chain and host-only dependencies, test/dev/build tools, MagicBlock integration, Anchor CLI and packages, Solana toolchain and supported client interface packages. The independent full XFChess application has many unrelated dependencies; do not silently expand this contract-focused task into an unrelated Bevy/desktop/backend rewrite. For clients that build or decode these contract instructions, check and migrate their contract-facing Anchor/Solana dependencies and generated bindings together.

**Required execution stages:**

1. **Inventory every dependency.** Parse every workspace `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` if present, SBF shim, toolchain pin, build/test script, IDL/client package file and any Git or vendored source. Include dependencies reachable only through `production`, `cranks`, Token-2022, `move-validation`, and localnet/devnet/mainnet feature combinations. Generate machine-readable rows for package, version, source, direct/transitive, features, dependency parents, latest **published stable**, selected latest **compatible**, advisory status, and migration status. Refresh version data from official registries and repositories at execution time; the dated examples below are **not** frozen target versions.
2. **Upgrade all direct packages to the absolute latest stable releases.** Upgrade Anchor and the matching Anchor CLI/IDL clients in one coordinated change; MagicBlock SDK and magic-program APIs; the Solana program/tooling dependency families; SPL Token/Token-2022, interface and associated-token packages; the vendored engine's compatible upstream revision; and all other listed utility/dev crates. Do not silently leave `tokio`, `serde`, `bincode`, `bytemuck` or any low-level package pinned to an old major merely because `cargo update` accepted existing constraints. Preserve API/serialization/on-chain behavior through targeted migrations.
3. **Upgrade transitive lockfile packages too.** Resolve to the latest stable release compatible with the newly upgraded direct dependency graph. For packages held back by a parent, upgrade or replace the parent and re-resolve. Inspect duplicated majors and unwanted feature activation with `cargo tree -d` and full feature trees. Do not use `[patch]`, vendored copies or dependency downgrades to conceal an old version; a temporary compatibility shim/fork needs documented approval, upstream tracking, and its own tests.
4. **Fix breaking changes explicitly.** Expect changes to `Pubkey`, `AccountInfo`, `CpiContext`, Anchor account macros, `Program<System>`, SDK CPI builders, Token-2022 interfaces, test validators, rustc/MSRV, and the SBF build. Change call sites individually while retaining seeds, signer constraints, instruction discriminator/arg order, account order, Game and Profile serialized layouts, payer/recipient policy and deployed program ID. If a new version changes on-chain serialized bytes or instruction ABI, stop for an approved migration rather than changing behavior silently.
5. **Validate each upgrade tier.** Build/test Anchor plus Solana first, then MagicBlock+ER lifecycle, then token/prize flows and tournament passes, then utility/vendored crates and host tests. Re-run the full local validator and actual ER integration matrix after merging the dependency branch. Track CPU/CU, stack frame, rent, ProgramData binary size, instructions, IDL, account metas and client compatibility before/after. Test production SBF and all supported feature combinations.
6. **Prove the final versions are current.** Run a fresh `cargo outdated` or equivalent source-backed registry audit (including dev and transitive packages), `cargo metadata`, `cargo tree`, `cargo audit`, and lockfile reconciliation. Record every package that is not at latest stable with its exact blocking dependency and upstream evidence. **Do not mark the upgrade complete while any required dependency remains outdated.** If no compatible current release exists, report **BLOCKED: LATEST RELEASE INCOMPATIBLE**, include a minimal reproduction and an upgrade/fork/upstream coordination path, and leave deployment blocked. Never force incompatible versions into a program handling escrow.

Use official crates.io/docs.rs, RustSec, GitHub release notes, Anchor docs, Anza release notes, MagicBlock docs and exact source tags. Avoid summaries from unverified blogs.

Versions verified on 23 October 2026:

| Package/tool | Snapshot pin | Stable release known on 23 Oct 2026 | Mandatory action |
| --- | --- | --- | --- |
| `anchor-lang` / `anchor-spl` | `=1.1.2` | `1.2.1` | Move both, Anchor CLI and contract clients to latest stable in lockstep; prove ABI/IDL |
| `ephemeral-rollups-sdk` | `=0.16.2` | `0.17.3` | Move to latest stable, test Anchor features and delegate/commit/undelegate callbacks |
| `solana-program` | `=3.0.0` | `4.1.0` exists | Pursue latest v4; resolve the SDK's Solana-v3 dependency conflict through verified compatible releases/upstream changes; otherwise explicitly block |
| Agave/Solana CLI | deployment used `3.0.6` | Agave `4.3.2` stable | Move to latest supported stable CLI and platform tools, prove SBF and validator compatibility |
| `solana-security-txt` | `=1.1.3` | `1.1.3` seen as latest | Confirm latest at execution time and verify SBF metadata |
| `magicblock-magic-program-api` | `=0.13.11` optional | verify current independently | Upgrade to latest stable and test crank ABI/feature compatibility |
| `solana-program-test` / RPC crates | 3.x, split pins | verify current independently | Upgrade complete Solana test/RPC family; eliminate incompatible duplicates |
| `nimzovich_engine` | local vendored | external upstream now separate | Compare with latest upstream; advance safely with `no_std` parity tests and recorded vendor revision |

Other direct dependencies that **must** be upgraded, not simply inspected: `bincode`, `tokio`, `base64`, `serde`, `thiserror`, `futures-lite`, `anyhow`, `bytemuck`, `solana-sdk`, `solana-rpc-client`, `solana-rpc-client-api`, `solana-commitment-config`, `solana-system-interface`, `anchor-spl`, and the dependencies of `xfchess-rules`. Enumerate and migrate every other package found in the workspace/lockfile, including optional test helpers. Treat `xfchess-rules` and `nimzovich_engine` as local packages whose **external** dependencies can be upgraded; a vendored engine source update requires behavior-parity evidence. Verify the exact runtime workspace dependencies, not just one package manifest. Build an updated lockfile only in the separate dependency branch, and record changes to the deployed SBF output. Evaluate known advisories with exploit reachability, not blind suppression.

Official source links to use and recheck before upgrades:

- Anchor crate: https://docs.rs/crate/anchor-lang/latest
- MagicBlock SDK releases: https://github.com/magicblock-labs/ephemeral-rollups-sdk/releases
- MagicBlock SDK Rust integration: https://github.com/magicblock-labs/ephemeral-rollups-sdk/blob/main/rust/sdk/README.md
- Solana Program crate: https://docs.rs/crate/solana-program/latest
- Agave stable releases: https://github.com/anza-xyz/agave/releases
- RustSec advisory DB: https://rustsec.org/
- Solana Foundation compatibility guidance: https://github.com/solana-foundation/solana-dev-skill

Do not infer that `solana-program=4.1.0` and Anchor 1.2.1 can be combined with MagicBlock 0.17.3 simply because all are newer. First compile a compatibility spike; compare `cargo tree -d`, features, class of Pubkey/AccountInfo compatibility, BPF/SBF behavior, generated IDL and runtime CPI account layouts. **The objective remains the newest stable versions**; incompatibility requires actual migration work or an explicit blocker, not acceptance of old pins as "good enough".

### Mandatory verification plan

Use a clean test environment and preserve actual outputs in `docs/security/verification-YYYYMMDD.md` or equivalent. If a command cannot run, write **NOT RUN** with the blocker; never invent successful checks.

**Build/format/static checks:**

```bash
git status --short
cargo fmt --all -- --check
cargo check -p xfchess-game --all-targets
cargo clippy -p xfchess-game --all-targets -- -D warnings
cargo test -p xfchess-game
cargo test -p nimzovich_engine
cargo test -p xfchess-rules
cargo tree -d
cargo audit
cargo build-sbf --arch v0 --manifest-path programs/xfchess-game-sbf/Cargo.toml
```

In the separate dependency-upgrade branch, also run `cargo metadata --locked --format-version 1`, a current dependency-outdated audit covering **workspace, dev, build and transitive** packages, and `cargo tree -e features`. If `cargo outdated` or `cargo deny` is not installed, install a vetted tool version in the isolated build environment or record an equivalent registry-backed result; do not falsely report a command as passed. Re-run every check after resolving the latest-release versions and commit the resulting lockfile. Include an explicit before/after list of **all** upgraded packages, not just the top-level Anchor and MagicBlock rows.

Adjust the host/testing target only when documented by the repo; some `solana-program-test` integration tests require a previously built `target/deploy/xfchess_game.so`, supported OpenSSL/platform tools, and certain devnet authority keyfiles that must **not** be placed in the audit repo. Do not treat an ignored test as a pass. Separately build/test `--features production --no-default-features` and supported devnet/localnet configurations; prove `production` cannot compile without `move-validation`.

**Instruction-level adversarial tests:**

- Unauthorized governance signer; an **authorized** governance signer attempting forged payout recipients; wrong game/escrow/treasury PDA, wrong owner, duplicate recipients, and empty Game recovery with no authenticated player data (must preserve all escrow).
- `InitProfile` account substitution targeting a writable Game PDA; identity field changes on the authentic Profile PDA; no loss of immutable counters, authority or profile verification.
- Undelegation with wrong `base_account`, wrong canonical buffer and spoofed System Program; caller-controlled malformed seeds; valid MagicBlock callback and rent restoration.
- Global session creation, exhaustion, expiry, revoke, rotation, reauthorization, new session key and stale key rejection; confirm a fresh **player-wallet-signed** authorization resets `total_spent`, while a session-only signature or active-session retry cannot reset it.
- Wager expiration overflow/boundary and retained escrow; wrong white player; end-game race.
- Duplicate settlement/finalization, simultaneous timeout/draw/resign/dispute, replayed moves and stale ER commits.
- Tournament repeated payouts via different instructions, forged extra accounts, second place paid as first, fake mint/ATA, pass reuse, duplicate shard registration and cancelled-tournament refunds.
- All money-moving failures must be atomic; verify lamport/token balances and data bytes before and after failing instructions.

**Property-based and differential tests:** Check conservation of each isolated pool under arbitrary legal instruction sequences; `total_spent <= spending_limit` under the chosen allowance model; prizes claimed at most once; Game PDA never replaced by an unrelated account; board legality matches the vendored engine; safe arithmetic for boundary values. Use a deterministic seed and shrink/display counterexamples. If fuzz tools are unavailable, record it and add targeted fixtures.

**ER integration:** On a supported validator test actual `delegate_game`, `undelegate_game`, callback processing, force recovery after timeout, missing validator, duplicate commit, malformed buffer, fresh rent payer, and replay, including byte-for-byte account state round-trip. Host-only tests are insufficient for the 2/5 findings.

**Compatibility and deployment review:** Compare generated IDLs, instruction discriminators, account metas, event layouts, serialized account sizes, PDA addresses, error mappings and binary size against the deployed snapshot. Verify frontend/backend builders construct exactly the expected transactions. Produce a deploy plan with preflight validation, authority signature review, reproducible artifact manifest, staged devnet testing, pause/rollback procedure, and post-deploy monitoring. Do not deploy or modify program upgrade authority during this task.

### Required deliverables

1. `SECURITY_FINDINGS.md`: a table with severity, exploitability, verified evidence (file and line), attacker role, affected assets, steps to reproduce, fix status, test and unresolved question. Distinguish **CONFIRMED**, **DISPROVED**, **POLICY CONFIRMED**, **BLOCKED ON VERIFIED DATA/IMPLEMENTATION**, and **NEEDS DYNAMIC TEST**.
2. A minimal security-only draft PR against the extraction and a port plan for `XFChess` source-of-truth. If the extraction has diverged, do not copy files wholesale. Include a line-by-line behavior/ABI preservation review.
3. A separate dependency-upgrade implementation PR (or ordered migration PRs) that actually moves **every dependency to the latest stable release**, with source-backed version inventory, full lockfile diff, required API migrations, compatibility matrix, test logs, advisory triage, SBF bytes/CU effects, and execution order. Merely recommending an upgrade is insufficient. Any package left outdated keeps the workstream **BLOCKED** with an exact incompatibility explanation.
4. A test report listing commands, exact pass/fail/ignored counts, platform toolchain versions, test keys required but absent, and coverage gaps. Do not claim audit completion or deployment readiness without ER integration and funding-path tests.
5. A decision/implementation record quoting the **confirmed** requirements: emergency recovery pays only authenticated original players, and wallet-signed global-session reauthorization starts a fresh allowance. For recovery, document the verified identity source, historical-data fallback/fail-closed behavior, approved schema/proof design and tests. For sessions, preserve the existing reset and prove wallet-signature and key-rotation guards.
6. A release-readiness verdict: **BLOCKED**, **CONDITIONALLY READY**, or **READY TO DEPLOY**, with precise conditions and responsible approvers. No external security certification claim is permitted from static review alone.

### Stop conditions

The owner has **already approved the desired recovery recipient policy**, the per-wallet-authorization allowance reset, and the objective of upgrading all dependencies. Do not re-ask whether arbitrary emergency recipients or lifetime session caps are acceptable; they are not. Still stop for approval of a specific new recovery-record schema/proof mechanism, storage/backfill plan for historical games, authority-key rotations, altered session lifetime limits, custom replacement of MagicBlock, state/ABI migrations, fee changes, program ID changes, or any network deployment. Migrate to Solana v4 crates as part of the separate latest-dependency goal **only after** validating compatibility and obtaining approval for any unavoidable ABI/state transition. If the latest stable dependency graph is impossible today, provide the smallest upstream/blocker resolution and mark the project blocked; do not claim that forcing unsafe pins completes the request.

## Audit observations and limits

The preliminary review read the 117 Rust files under `programs/xfchess-game/src` from the audited commit, plus selected tests, documentation, and upstream SDK implementation. It was a source review, **not** a completed formally scoped audit of the entire Rust workspace, all tests, MagicBlock runtime, network deployment, backend clients, or compiled binary. Source patterns alone are not exploit proofs. The owner has resolved the two policy questions, but the recovery identity proof, contract changes, tests and latest-dependency migration still require implementation.

Other issues worth testing, **not confirmed vulnerabilities**:

- `account_ix/profile.rs`: `now - date_of_birth` is unchecked, and the caller controls `date_of_birth`; extreme values can trap under overflow-checked builds. Determine whether it is merely transaction-local DoS.
- `game_ix/cancel.rs`: `now - game.updated_at` is unchecked. Analyze ER-committed timestamps and whether corrupted future timestamps can become reachable.
- `tournament_ix/lifecycle/initialize.rs`: sum of ten `u16` prize shares can overflow before the `<= 10000` check; signed authority inputs may trigger a failed instruction, but exploitability is unproven.
- `account_ix/withdraw.rs`: token wager accounts have little binding in the optional token branch. Trace whether `wager_token: Some` is reachable in production before classifying this as a token-account substitution issue.
- `tournament_ix/lifecycle/cancel.rs`: compare the `usdc_prize_returned` event value with post-CPI balances, and check refund source selection and replay behavior.
- `game_ix/cancel.rs`: `already_cancelled` allows repeated evaluation of escrow refunds if the escrow receives new lamports after cancellation. Verify accounting assumptions and whether this can redirect money, rather than automatically rejecting valid idempotent retries.
- `docs/THREAT_MODEL.md`: it explicitly warns that some authority keys were leaked in history and require rotation before mainnet. Verify rotation status from authorized operators; do not infer the current deployed keys have already been rotated.

The core security question is whether an untrusted signer can select another user's state account or payout destination, and whether a privileged incident path deliberately accepts that trust. The first is a code defect; the second requires an explicit operational model.
