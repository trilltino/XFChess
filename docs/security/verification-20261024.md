# Latest-dependency and release follow-up â€” 24 October 2026

Status: latest-stack verification passed for the checks recorded below; devnet upgrade finalized on 25 October. Outstanding audit/dependency limits remain explicit. This file records current work separately from
the historical 23 October pinned-dependency results. Pending checks are not passes.

The owner authorized the recovery schema/client migration, devnet deployment,
contracts-repository main push and an XFChess PR. Mainnet and authority rotation
are outside that authorization. The initially dirty desktop/Tauri work is retained;
the review checkout includes only the recorded contract/client/dependency scope.

The actual workspace now selects Anchor/SPL 1.2.1 with `solana-v4`, ER SDK 0.17.3,
Solana Program 5.1.0, SDK 5.0.0 and Agave test/RPC libraries 4.3.2. The SDK's unused
Anchor integration is disabled; its required System interface is explicitly enabled.
Crank encoding uses the current MagicBlock instruction serializer instead of a
direct bincode dependency. No new vendored dependency or compatibility fork was added.

The latest engine dependency manifest passed 73 unit tests, with four existing
ignored tests, and was pushed to contracts main as
`08e430699297f7d13f94fae32128fc0bafedf778`. XFChess pins that revision.
Engine source behavior was not changed. A recursive contract-root update then
refreshed the compatible transitive lockfile entries.

The registry-backed inventory covers the lockfile closure of both contract crates,
rules and engine, including resolved optional/dev/build dependencies. It is broader
than the deployed normal-feature graph. Every external direct dependency of these
roots matches latest stable. The current inventory still has 342 older entries;
this does **not** satisfy the owner's all-transitive-latest requirement.
The published bincode 3.0.0 fails `cargo check` even in a separate empty project:
its source contains a deliberate `compile_error!`. The SDK also constrains older
Solana compatibility packages, base64ct and the delegation API. Silently changing
version strings cannot repair these upstream interfaces or preserve their wire ABI.

Current cargo-audit reports 11 workspace findings, four in the contract closure.
The normal SBF dependency tree includes rkyv 0.7.46 via delegation API 3.1.0 and
ER SDK 0.17.3; old ed25519-dalek is absent from that normal-feature tree.
Reachability of each advisory still requires review; no advisories were suppressed.

Build sequence: the first resumed SBF build uses the pre-recursive-update candidate
and old engine revision. Its output is diagnostic only. Deployment requires a new
SBF build from the final lockfile and source, followed by the actual runtime tests.
The intermediate all-target metadata build was stopped after the recursive update
to avoid verifying an obsolete graph. Test compilation now uses the updated graph.

| Check | Current evidence |
|---|---|
| Actual contract library, Anchor 1.2.1 solana-v4 cohort | Passed before final recursive lock refresh |
| Engine unit tests with updated dependency manifest | 73 passed, four ignored |
| Registry inventory | 953 entries; zero registry errors; 342 older entries |
| Cargo audit | Failed: four contract-closure findings; 11 workspace findings |
| Latest bincode isolated compatibility check | Failed as published; exit 101 |
| Final latest-lockfile SBF build and stack inspection | Passed; devnet SHA f60ce59fc4ac7cd02a83e520c54b75ea9d44c6127a986fa8efb01b3e431f74fe; direct frame accesses bounded to 4096 bytes |
| Latest contract host/SBF tests and strict Clippy | 263 tests passed; strict Clippy passed with warnings denied |
| Normal IDL generation with safety lint enabled | Passed, 21 print tests; 82 instructions/47 types; old layouts and ABI preserved |
| Latest-stack local base-chain/ER round trip | Passed, including schedule/cancel task CPIs; localnet-only routing and SDK 5 signed transactions |
| Latest client/backend integration checks | Passed: backend all targets (including CU benchmark dependency); three wire-boundary and eight rules tests |
| Devnet upgrade and independent ProgramData hash verification | Finalized at slot 514786564; exact code hash and zero padding independently verified |
| Contracts source/IDL synchronization and main push | Full source, clients, IDL and dependencies pushed to [568b153](https://github.com/trilltino/xfchess-contracts/commit/568b15365965455785fbe65af07b4f0157d78bf8); independent extraction rebuild/tests pending |
| XFChess PR | [Draft #52](https://github.com/trilltino/XFChess/pull/52) |

The local ER test uses fixed loopback RPCs and freshly generated in-memory keys.
Its localnet-only routing override selects the official local ER identity; devnet
routing is unchanged. It simulates signed transactions, performs a legal move,
rejects a stale move and checks exact committed bytes, recovery identity and wager
escrow after undelegation. The earlier pinned-stack run completed undelegation but
the test's expected bytes omitted the handler's two fee accruals. That assertion
has been corrected; it must pass on rerun before this check can be marked passed.

The mixed-prize regression failed against the former SBF implementation and passed
after the guard was added. Treasury and external-ELO fixtures no longer read
authority secret files or return successful early skips. Privileged fixtures use
the real public role address through Banks metadata execution, which omits crypto
signature verification; ordinary negative fixtures retain real signing. The Agave
4.3.2 source still uses the unsigned `prepare_entry_batch` metadata path. These
fixtures prove instruction authorization behavior, not possession of privileged keys.

Host verification uses `--config docs/security/host-test-profile.toml` to disable
Bevy's blanket host dependency optimization and debug symbols. It keeps debug
assertions and overflow checks and changes no dependency or release/SBF setting.
The official Cargo profile override mechanism is documented at
https://doc.rust-lang.org/cargo/reference/config.html#profile.
The normal safety-linted IDL build uses an isolated native manifest generated from
the actual workspace declarations and actual source paths, without the Banks/RPC
dev graph. Full program-test compilation remains a separate required check.

The local stack uses the verified Agave 4.3.2 and ER 1.1.1 binaries and eight
official genesis programs. Initial Linux fixture downloads totaled about 165 MiB;
the Windows Agave CLI archive adds 105.5 MiB and is bounded below 128 MiB, with its
SHA-256 verified against the official release. CLI-only extraction avoids another
large optimized source build. Platform-tools v1.58 was already cached.

Final shared patch pins also advance borsh 1.8.0 to 1.8.1, chrono 0.4.44 to
0.4.45 and serde_bytes 0.11.17 to 0.11.19. These workspace constraints otherwise
kept compatible contract dependencies below their published patch releases.
Normal Anchor safety-linted IDL generation passed (21 print tests), and assembly
validated all 82 instruction discriminators/argument lists and the Game/Profile
layouts. It is being rerun after those final shared patch pins.

Advisory target triage uses the actual platform-tools v1.58 cargo/rustc with
`--target sbpf-solana-solana --edges normal`. crypto-bigint 0.5.5,
curve25519-dalek 3.2.0 and ed25519-dalek 1.0.1 are absent from that target's
normal dependency graph. Their presence in host/test lockfile closure is not
evidence of inclusion in the deployed program. rkyv 0.7.46 remains a dependency
of the delegation API, but that API's `diff` feature is disabled. Its only rkyv
source use is `util::AlignedVec` behind that disabled module; the reported
Rc/Arc archive-validation path is not called by this program. This is a scoped
reachability assessment, not a waiver or a claim the dependency is updated.

Current latest-stack results: the complete default program and client-boundary
suite passed 263 tests with zero failures and four explicitly ignored network
tests. The ignored local ER target was then run explicitly and passed: real
delegation, legal move, stale rejection, commit/undelegation, exact Game bytes,
unchanged permanent identity and untouched 20,000,000-lamport wager escrow.
Measured simulations included 67,130 CU for delegation, 24,283 for the move and
44,980 for undelegation. These single fixtures are not worst-case CU benchmarks.

The missing external pass fixture initially caused eleven integration setup
failures. Fetching the actual deployed devnet pass program, independently pinned
to SHA-256 `06a784b08d55cdff44c7b38272bfbf8d1e0f7ad368e4dbd1ed6b494ecd4b57ff`,
resolved that prerequisite. Eleven pass-bridge tests, twelve pass-registration
tests, four prize tests and seven treasury tests all passed, followed by the full
suite above. CI now fetches that reviewed fixture and fails on any hash change.
No pass-program upgrade was performed.

Solana's current host `solana-get-sysvar` returns UnsupportedSysvar outside SBF
and no longer calls the old stub registry. The eighteen account-security
regressions were moved into the actual VM. A closed Game is purged by the VM;
its retry now correctly fails the Game-owner guard. Anchor's absent optional
accounts use the current program ID, which the fixture must not overwrite.
These fixture corrections retain exact error and balance/state assertions. All
eighteen now pass against the latest binary.

SDK 5 and stable Agave 4.3 expose different Rust transaction/account types. The
new client boundary uses current wincode and Agave's public encoded-transaction
decoder, without adding old dependency pins or a compatibility fork. Its three
tests verify real legacy/v0 signatures and message bytes and reject malformed
signature headers. Banks fixtures convert only their in-memory setup values and
their newly generated local payer; deployed keyfiles are never loaded.

Final follow-up: strict actual-workspace Clippy passed with warnings denied. The expanded live ER test passed in 5.34 seconds, including MagicBlock scheduled-task creation/cancellation before commit/undelegation and exact byte restoration. The final locked SBF rebuild retained SHA-256 f60ce59fc4ac7cd02a83e520c54b75ea9d44c6127a986fa8efb01b3e431f74fe. Inventory refreshed to 953 closure entries, 768 registry names, 342 older entries and zero registry failures.

Final backend `cargo check -p backend --all-targets --locked` passed. Current APIs now decode UI-account data explicitly, copy the five RPC account fields at the SDK boundary, use the split status-client types and preserve lowercase SHA-256 hex output. Client tests now check both directions of legacy/v0 wire conversion and malformed-header rejection. Rules suite: eight passed.

Authorized deployment finalized on 25 October 2026: [3u4N77sN31qvdzx8PFHuRuDp1LGyMGKw7wQc62qmc4iXoCkhJsLq5j9nShjeQju8ZrVVnmudum1APmjyDL7u4FSj](https://explorer.solana.com/tx/3u4N77sN31qvdzx8PFHuRuDp1LGyMGKw7wQc62qmc4iXoCkhJsLq5j9nShjeQju8ZrVVnmudum1APmjyDL7u4FSj?cluster=devnet), slot 514786564. ProgramData code SHA-256 matches the reviewed artifact exactly; authority unchanged. Upload resumed in its existing buffer after public RPC rate limiting, with an independent buffer hash check before upgrade. The buffer closed automatically. Net payer spend: 5,710,000 lamports (0.00571 devnet SOL). Source commit: fb168600806aca161ff423610bc9a6457476b494.
