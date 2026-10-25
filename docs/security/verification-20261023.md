# Contract verification — 23 October 2026

**Verdict: BLOCKED.** This records completed checks and explicit gaps, not an
external security certification. Local SBF funding tests have completed. Actual
ER lifecycle validation and the dependency release graph remain unresolved.

## Provenance and environment

- Repository `https://github.com/trilltino/XFChess.git`, `main`, HEAD
  `70a25c5c4e2d2fef18bb30f54f4260a74e27fadb`; initial tree was dirty, including
  contract and desktop work. Existing edits were preserved.
- Extraction main `25c1b31b1e8d2692189b795e694caac8a3959658`; inspected security
  branch `ceb3f540cdc856ff028b3b108683f315c12f448a`. Four narrow diffs were reviewed,
  then adapted to the source of truth rather than copied wholesale.
- Read-only finalized devnet verification matched deployed code hash
  `f9a6d1c3f417c33e602a036fccdd8a01c970d092978319dd401c3192bc7f70c3`, original
  1,094,544-byte payload, deployment slot 513888640 and the authority named in
  DEPLOYED.md. ProgramData is 1,424,109 bytes including its 45-byte header;
  remaining payload padding was all zero. See `deployed-program-20261023.json`.
- Windows PowerShell; rustc 1.99.0, Cargo 1.99.0, Anchor CLI 1.1.2,
  solana-cli 3.0.6, cargo-build-sbf 4.4.0 / platform-tools v1.57,
  cargo-audit 0.22.0, cargo-outdated 0.17.0.
- WSL Ubuntu could not attach its configured missing
  `D:\WSL\Ubuntu-24.04\rootfs\ext4.vhdx`. No local ER validator was available.
  No keyfile was read, deployment performed, authority rotated or network transfer
  submitted. No privileged signing material is required for the added host tests.

## Results

| Check | Result | Scope / limit |
|---|---|---|
| `cargo check -p xfchess-game --all-targets --locked` | PASS | Actual workspace, including pre-existing and added tests; initial build took 39m26s, final incremental check 3.76s. |
| `cargo build-sbf --arch v0 --manifest-path programs/xfchess-game-sbf/Cargo.toml` | PASS | Actual deployable shim, devnet/default features. Artifact manifest records final bytes/hash and all 82 instruction discriminator occurrences. Not deployed. |
| Native program library tests | 101 passed, 0 failed, 0 ignored | Actual source, isolated manifest retaining pinned runtime dependencies; separate host lockfile, not the full shared application graph. |
| Native account security dispatch | 18 passed, 0 failed, 0 ignored | Real Anchor dispatch, fixture signer flags/sysvars. Includes forged recovery recipients, empty legacy recovery, no mutations, profile substitution, callback constraints, renewal/stale keys, v2 join binding and ID-reuse rejection. CPI stubs reject unsupported calls instead of pretending transfers succeeded. |
| Instruction layout fixtures | 4 passed, 0 failed, 0 ignored | Existing captured bytes; v2 account prefix/argument preservation; error append ordering. |
| Real desktop builder layout fixtures | 3 passed, 0 failed, 0 ignored | Compiles the actual builder module; exact create/join v2 metas and expired-withdraw account order. |
| Existing profile/session host fixtures | 3 passed, 0 failed, 0 ignored | Statistics/verification preservation and budget arithmetic. |
| Existing game settlement host fixtures | 4 passed, 0 failed, 0 ignored | Existing move application, opponent and initialization guards. |
| SBF recovery fixtures | 8 passed, 0 failed, 0 ignored | Includes authenticated-record nonzero payout conservation, extra escrow retention, treasury rent movement and either forged recipient rejecting with all observed accounts unchanged. Authority-role fixtures use metadata execution, explicitly without cryptographic signature verification. |
| SBF global funding and wallet lifecycle | 8 passed, 0 failed, 0 ignored | Session vault funds game/record rent and wagers; exact combined minimum preserves vault rent reserve; underfunding rejects; wallet create/join/cancel/retry refunds each player once and closes the record. |
| SBF sponsored profile fixtures | 2 passed, 0 failed, 0 ignored | Legitimate zero-funded sponsored initialization and malicious Game substitution with no data/lamport/username changes. |
| SBF tournament creation fixtures | 3 passed, 0 failed, 0 ignored | Authority seats registered players with a Ready identity; wrong authority/unregistered/self seats reject; tournament session funds game/record/wager at the exact combined rent minimum. |
| SBF existing settlement fixtures | 5 passed, 0 failed, 0 ignored | Fees/rent/refunds and forged fee-payer/recipient checks with required trailing identity account. |
| Rules library | 2 passed, 0 failed, 0 ignored | Standalone manifest points at the actual source. |
| Pinned engine library | 73 passed, 0 failed, 4 ignored | Separate copy of the exact pinned engine revision with its workspace versions expanded; expensive existing perft tests remain ignored. |
| Strict isolated `cargo clippy --all-targets -- -D warnings` | PASS | Includes current library and host/client layout test targets. Does not replace full workspace/SBF/ER tests. |
| `cargo check -p backend --lib --locked` | PASS | Actual backend, including migrated instruction builders, setup co-signing and dispute/payout routes; completed in 3m38s. Does not replace full desktop/CU builder application builds. |
| Before-fix comparison | Expected FAIL: 10 passed, 5 failed | Same v2 implementation with only profile/expiry/callback/legacy recovery guards reverted to source HEAD. Five regressions fail, including profile overwrite and timestamp panic. No source-of-truth files were reverted. |
| IDL print tests | 21 passed, 0 failed, 101 filtered | Lint explicitly skipped for interface generation; raw types/instructions came from Anchor, not invented signatures. |
| Normal IDL safety-comment lint | FAIL | Pre-existing `friends_ix.rs:party_a` lacks a CHECK justification. Skipping for generation is not lint clearance. |
| Client IDL assembly | PASS | 82 source instructions; old Game/Profile layouts and old discriminators checked; existing stale register_player elo argument reconciled against source HEAD. Runtime-validated trailing accounts explicitly declared. |
| Latest core dependency CPI spike | PASS | Anchor/SPL 1.2.1, SDK 0.17.3, solana-program 5.1.0. Entire graph still not all-latest; see separate blocked workstream. |
| Registry audit | PASS lookup coverage | 955 entries, 790 registry names, 0 request failures, 684 outdated entries; two Anza Quinn names have no stable release. Feature activation is not inferred from a lockfile. |
| `cargo audit --json --file Cargo.lock` | FINDINGS | 28 workspace vulnerabilities, 21 contract-closure matches, plus informational/yanked warnings; no exploit reachability or suppression asserted. |
| Production SBF, no default features | FAIL CLOSED | Existing mainnet program-ID and authority placeholders deliberately trigger six const-evaluation guards. Production automatically includes move-validation and has an explicit compile guard. No keys/IDs were changed to bypass this. |
| Full workspace `cargo fmt --all -- --check` | FAIL | Unrelated pre-existing dirty `src/rendering/board/spawn.rs`; task files formatted individually. No mass-format of the user's other work. |

Unique program checks above cover **159 passing tests**: 133 host (101 + 18 + 4 +
3 + 3 + 4) and 26 SBF runtime fixtures (8 + 8 + 2 + 3 + 5). Rules and pinned
engine add 75 passed and 4 ignored. IDL print tests and repeated runs are not
added to these counts.
Native fixture signer bits do not establish ed25519 signature verification.

The initial SBF funding run caught an `UnbalancedInstruction` in v2 vault-funded
record creation: direct lamport changes occurred before System allocate/assign
CPIs that synchronized only the credited target. The final implementation completes
those CPIs before debiting the program-owned vault. Both global and tournament
session funding now pass with the exact combined rent/wager reserve. This failure
was fixed; it is not an ignored or excluded test.

## Reproduction

`prepare_native_harness.py <temporary-directory>` creates a standalone manifest
pointing at the real program/test files, avoiding unrelated application build
locks and the program-test code-generation graph. It does not mutate application
dependency versions. Use its manifest with:

```text
cargo test --manifest-path <temporary>/Cargo.toml --lib --test account_security_tests --test client_layout_tests --test instruction_layout_tests --test profile_session_tests --offline
cargo clippy --manifest-path <temporary>/Cargo.toml --all-targets --offline -- -D warnings
```

`prepare_before_fix_harness.py` prepares the separate intentionally vulnerable
comparison fixture. Never deploy its artifact. Expect the guard regression tests
to fail. Registry inventory, advisory filtering, account/instruction extraction,
read-only deployed hash verification and IDL assembly have corresponding scripts
in this directory.

IDL generation uses actual `__anchor_private_print_idl` tests with
`ANCHOR_IDL_BUILD_PROGRAM_PATH` set to the real program, resolution TRUE and the
explicitly recorded `ANCHOR_IDL_BUILD_SKIP_LINT=TRUE`. Assembly checks the source
registry and adds the remaining-account declarations Anchor cannot infer.

Captured test/build result excerpts and full-log hashes are in
`verification-output-20261023.txt`. The initial funding-order failure and the
intentionally vulnerable comparison are retained and labeled; neither is counted
as passing evidence.

## Required remaining evidence

- Actual pinned/candidate SDK delegate → commit → undelegate → deserialize, force
  recovery, canonical buffer ownership/signature, rent reimbursement, stale commits
  and concurrent payout/cancel/dispute behavior under a supported ER validator.
- Finish semantic review of account-matrix rows, full client/backend build matrix,
  generated-interface lint, advisory reachability and all-latest dependency work.
- No CU benchmark, fuzz/property campaign or supported mainnet key rotation was
  completed. None is silently counted as a pass.

Historical wiped games remain locked without verifiable identities. The owner
approved the new schema/client transition, but no history proof, backfill, fork,
network deployment or privileged transfer was approved or performed.
