# FIDE implementation verification and blocked handoff

**Environment follow-up:** Docker Desktop was installed but stopped. Starting
the existing application restored its Linux engine (28.3.2). Its Linux virtual
disks are present on C:. The Ubuntu registration still points to the missing D:
disk, but Docker is an available alternative for provisioning the pinned ER
stack. The infrastructure failure recorded below is historical, not a current
requirement to repair Ubuntu before proceeding. No `ubuntu:24.04` image is cached
yet; validator/ER provisioning and execution have not been verified. The feature
remains incomplete for the implementation and verification reasons listed below.

**Overall: BLOCKED / incomplete. No live game mode has been upgraded.**
The assignment permits stopping safely for an infrastructure blocker. This
attempt implements the independently testable rules foundation and stops before
choosing a deployed history layout or migrating accounts. It is not a claim of
end-to-end FIDE support. In particular, the existing application and program
still auto-draw at 100 halfmoves because their coordinated upgrade is unfinished.

## Branches and source provenance

| Repository | Checkout | Branch / source |
| --- | --- | --- |
| XFChess | `C:\Users\isich\XFChess-fide` | `feat/fide-adjudication`, based on fetched `40e78cb4308303b4f07f991d835de5205d6e2dc8`; inventory and this report |
| xfchess-contracts | `C:\Users\isich\xfchess-contracts-fide` | `feat/fide-adjudication`, based on fetched `d8dea0a9d41c0a401add75cb22b580128360700b` |
| Implemented rules source | contracts commit | `0fcf579980d6279bcd5857cf1638db610d39343b`, tree `3700590ec95e0626bf41e821e54d1a524508e5ae` |

The original `C:\Users\isich\XFChess` working directory and its substantial
uncommitted changes were preserved. A separate detached contracts worktree at
`C:\Users\isich\xfchess-contracts-fide-baseline` was used to compare pre-existing
Clippy failures. No remote branches were pushed and no PR was opened: the
assignment's full test gate is unmet. No deployment, upgrade, funding, wallet
key use, settlement transaction or mainnet mutation occurred.

The application engine pin remains
`08e430699297f7d13f94fae32128fc0bafedf778`. The new local engine commit is not
published or selected by the application. Do not point Cargo at an unpublished
revision and describe the cross-repository build as coherent.

## Concrete infrastructure blocker

Commands executed:

```text
docker info --format '{{.ServerVersion}}'
wsl -d Ubuntu-24.04 -- sh -lc 'command -v solana-test-validator; command -v ephemeral-validator; command -v cargo; uname -r'
```

Docker cannot open `//./pipe/dockerDesktopLinuxEngine` (system cannot find the
file). Ubuntu fails before launching a shell:

```text
Failed to attach disk 'D:\WSL\Ubuntu-24.04\rootfs\ext4.vhdx' to WSL2:
The system cannot find the path specified.
Wsl/Service/CreateInstance/MountDisk/HCS/ERROR_PATH_NOT_FOUND
```

Neither `solana-test-validator` nor `ephemeral-validator` is available as a
native Windows command. The repository's pinned ER fixture stack requires Linux
executables. Restoring the missing WSL disk/distro, or making the Docker Linux
engine usable, is the prerequisite for measured L1/ER account-growth,
delegation/commit/undelegation and settlement tests. Native SBF compilation works;
the blocker is not Rust compilation. The prompt explicitly requires storage/CU/
delegation measurements before selecting the deployed storage representation.
No migration choice has been presented as verified without those measurements.

## Implemented code and change map

All code changes are in the contracts repository; the application branch holds
the authority inventory and blocked verification record.

| Modified or added paths | Purpose |
| --- | --- |
| `Cargo.toml`, `Cargo.lock` | Register new local adjudication crate; no broad version upgrade |
| `.github/workflows/contracts.yml` | Run its tests, no-default-feature build and strict Clippy in CI |
| `crates/xfchess-adjudication/{Cargo.toml,README.md}` | Canonical allocation-free library, optional existing-version shakmaty adapter and explicit integration contract |
| `crates/xfchess-adjudication/src/lib.rs` | Strict rule thresholds, agreement eligibility, typed reasons and terminal precedence |
| `crates/xfchess-adjudication/src/identity.rs` | Exact version-1 34-byte identity; pieces, side, rights and normalized legal EP; excludes clocks/padding |
| `crates/xfchess-adjudication/src/repetition.rs` | Borrowed 151-position reversible window, initial occurrence, ordering, checked bounds, prospective queries and checkpoints |
| `crates/xfchess-adjudication/src/clock.rs` | Pure integer per-side clocks, increments, flag boundary, formal-claim pause and two-minute penalty |
| `crates/xfchess-adjudication/src/claims.rs` | Post-authentication procedure, game/ply/nonce checks, no withdrawal, one-time resolution/penalty, mandatory intended move and claim-as-offer state |
| `crates/xfchess-adjudication/src/shakmaty_adapter.rs` | Host identity uses `Chess::legal_ep_square()` |
| `crates/xfchess-adjudication/tests/{rules,claims}.rs` | Threshold, history bound, recovery, procedure, ordering, rejection, clock and penalty fixtures |
| `crates/nimzovich_engine/Cargo.toml`, `src/lib.rs` | Depend on the shared core and expose checked adapters |
| `crates/nimzovich_engine/src/on_chain.rs` | Equality for detecting stale/forged derived bitboards |
| `crates/nimzovich_engine/src/on_chain_moves.rs` | Reject malformed/uppercase/irrelevant/missing promotion choices; check compact state, king capture, EP victim and castling rook |
| `crates/nimzovich_engine/src/on_chain_rules.rs` | Validate compact representation/king safety, legal-EP identity and sound directional material proofs |
| `crates/nimzovich_engine/src/on_chain_replay.rs` | Atomic legal board/history transition with exact post-board verification and automatic terminal precedence |
| `crates/nimzovich_engine/src/hash.rs` | Castling/EP-sensitive search identity and independent recomputation tests |
| `crates/nimzovich_engine/src/search/{make_unmake,alphabeta}.rs` | Maintain metadata keys across moves, undo and null turns; clear EP during null turns; preserve heuristic/legal distinction |
| `crates/nimzovich_engine/tests/{fide_adjudication,fide_replay}.rs` | Independent shakmaty differential legality/identity, replay atomicity, 100/150 behavior and explicit dead-position gap |
| XFChess `docs/rules/fide-standard-chess-adjudication.md` | Initial inventory, authority boundaries, online adaptations, risks and integration order |
| XFChess `docs/rules/fide-verification-2026-10-31.md` | This handoff and exact acceptance status |

The pure claim procedure does **not** implement seat cryptography or prove an
intended move itself. Its documented adapter must authenticate and legally
validate before receipt. It is not an exposed endpoint or an on-chain claim
instruction. Similarly, its integer clock requires a defined trusted timestamp
source and durable serialization; that does not yet exist in live games.

## Verification performed

Commands below ran in the contracts feature checkout unless stated otherwise.
Host tools: Cargo 1.99.0, rustc 1.99.0, anchor-cli 1.1.2, solana-cli 3.0.6,
cargo-build-sbf 4.4.0; SBF platform-tools v1.58, arch v0. Anchor *library* pin
remains 1.2.1. The SBF builder used locked dependencies and default program
features. Tooling version differences are recorded, not silently upgraded.

| Command / check | Result |
| --- | --- |
| `cargo test -p nimzovich_engine -p xfchess-adjudication --locked` | PASS: 101 tests, zero failures; six existing slow/diagnostic/benchmark tests and twelve existing documentation examples ignored |
| `cargo test -p xfchess-adjudication --all-features --locked` | PASS: 13 tests, zero failures |
| `cargo check -p nimzovich_engine -p xfchess-adjudication --no-default-features --locked` | PASS; engine emits pre-existing unused/dead-code warnings |
| `cargo clippy -p xfchess-adjudication --all-targets --all-features --locked -- -D warnings` | PASS |
| `cargo fmt -p nimzovich_engine -p xfchess-adjudication -- --check` | PASS |
| `NO_DNA=1 cargo build-sbf --arch v0 --tools-version v1.58 --manifest-path programs/xfchess-game-sbf/Cargo.toml -- --locked` | PASS, native Windows; compilation evidence only, not executed ER evidence |
| `py -3 docs/security/inspect_sbf_frames.py target/deploy/xfchess_game.so --objdump C:\Users\isich\.cache\solana\v1.58\platform-tools\llvm\bin\llvm-objdump.exe --output C:\Users\isich\xfchess-fide-frames.json` | PASS: 35,317 direct accesses, offsets -4096 through -1, none outside frame; not a full loader/verifier or dynamic-address proof |
| `cargo clippy -p nimzovich_engine --lib --locked -- -D warnings` | BLOCKED: 83 errors; clean default-head comparison gives the same 83 diagnostics, no added diagnostic messages |
| Contracts `cargo fmt --all -- --check` | BLOCKED by existing unrelated import-order differences in contract tests; changed packages pass |
| XFChess `cargo test -p xfchess-rules --locked` | PASS: ten baseline tests |
| XFChess `powershell -NoProfile -File scripts/ci/check_rust_safety.ps1 -InventoryOnly` | PASS: source extraction inventory clean and ignored-test audit successful |
| XFChess `cargo fmt --all -- --check` | BLOCKED: existing Tauri newline-style differences in the clean checkout; application Rust source was not changed |
| New Rust additions / `git diff --check` | PASS: no added panicking extraction and no whitespace errors |

The final engine SBF artifact is `target/deploy/xfchess_game.so`, 1,352,752 bytes,
SHA-256 `b66f1396c3d15be2d33f5f97fe044e141e21feadde83a2bee82641a9c5c452d4`.
Baseline XFChess compilation produced 1,347,896 bytes, SHA-256
`8ff3975cc5dbb52863bbf107fd78ad1c7001b4c63cda84e6a31748d9d1d15670`.
These are different source/workspace provenances, not a controlled CU or size
regression comparison. Neither artifact was deployed. The new clock/claim/history
paths are not wired into the program and this build does not prove their runtime
cost or instruction behavior.

Raw command logs are beside the checkouts as `C:\Users\isich\xfchess-fide-*`;
portable copies of the final test output and frame report accompany this report
under `evidence/`. The new tests are discovered normally and CI now runs them.
No new ignored test was introduced. Instruction/Agave tests, complete program
suite, runtime CU, rent, ER round-trip, settlement, UI/platform and two-client
network tests have **not** been executed for a completed feature: that feature
has not been integrated. They remain mandatory gates, not assumed passes.

## Named fixture coverage

- Starting knight cycle through third and fifth occurrences, including move-zero.
- Different move orders reaching the same position; returned rook with lost rights.
- Side/rights/piece differences and exact key byte/checkpoint round-trip.
- EP with a legal capturer, no capturer, vertical pin and horizontal discovered check.
- Explicit queen/rook/bishop/knight promotions; missing, bogus and
  irrelevant suffix rejection; malformed boards and stale bitboards fail atomically.
- All candidate moves compared with shakmaty's legal set at each visited position
  across twelve seeded games, up to 160 plies each; seed `0xF1DE2026`.
- Current/prospective threefold and fifty eligibility; continuation at 100/101;
  automatic draw at 150 and checkmate on that ply taking precedence.
- Formal clock pause, exact flag boundary, monotonic timestamps, replay nonce,
  stale/wrong-seat/wrong-game envelopes, malformed intent, one-time penalty,
  claim-as-offer persistence and mandatory move blocking.
- Illegal/wrong-board/duplicate/out-of-order/capacity updates leave board and
  tracker unchanged; full 151-position storage, rights/capture reset and restore.
- K+B and K+N against a bare king; K+NN is not declared dead; one-sided bare-king
  impossibility; search full/incremental/undo/null keys.
- Explicit remaining gap: `4kb2/8/8/p1p1p1p1/P1P1P1P1/8/8/4KB2 w - - 0 1`.
  Pawns are mutually blocked; each bishop is trapped behind its own colour wall,
  and neither king can cross the attacked gaps. Material-only proofs return
  `Unproven`, demonstrating why they cannot certify strict-FIDE completion.

## Acceptance matrix for the requested end-to-end feature

| Scenario | Status and exact missing evidence |
| --- | --- |
| Human vs AI | BLOCKED: primitives pass; no session resource, claim controls, AI claim strategy or finality integration |
| Local PvP | BLOCKED: no live authoritative claim/clock transition wired into Bevy |
| Casual network PvP | BLOCKED: no authenticated typed claim events, durable decisions, forced-move/penalty replay or two-client partition tests |
| Free rated PvP | BLOCKED: result authority and persistence integration unfinished |
| Wagered / ER | BLOCKED: Linux ER unavailable; no versioned Game migration, claim instructions, exact clock authority or measured storage/settlement |
| Tournament match | BLOCKED: confirmed draw reason/scoring/advancement/payout propagation not implemented |
| Spectator / archive / PGN | BLOCKED: typed terminal reasons/replay metadata not integrated |
| Existing sessions/accounts | BLOCKED: deployed schema untouched; a tested versioned migration/legacy gate has not been built |
| All entrypoints | BLOCKED: existing `apply_recorded_move` and Bevy still auto-draw at 100; no fivefold history in live Game |
| Rule-complete terminal states | BLOCKED: general dead-position/unwinnability proof unresolved; resignation, timeout and early agreement corrections not integrated |
| Clock authority | BLOCKED: integer primitive tested, but no authenticated per-side account clock/pause/penalty serialization or ER proof |
| UI / accessibility | BLOCKED: no live UI controls or platform validation |
| Security / performance | BLOCKED: primitive rejection tests and compile/frame checks pass; instruction authentication/replay, CU, heap, rent and ER lifecycle gates outstanding |

Puzzles, replay and spectators are NOT APPLICABLE as initiators of live claims;
their read-only result/reconstruction behavior remains in scope and unverified.

## Release/migration continuation gate

1. Repair the missing Linux runtime and reproduce the pinned local L1/ER stack
   with disposable in-memory test identities and loopback RPCs only.
2. Measure baseline move CU, full-history key/claim CU, allocation/rent, stack/heap
   and delegation overhead. Exact key storage alone is 151 * 34 = 5,134 bytes;
   it is not a complete account layout and exceeds one 4 KiB SBF stack frame.
   Select inline boxed/zero-copy storage versus co-delegated companion state
   only after these results, with explicit ownership and atomicity proof.
3. Define Game versioning, all creation/migration paths, exact inclusion-time
   authority and legacy active-game handling. Preserve existing result
   discriminants and manual decoder offsets until a coordinated versioned update.
4. Integrate current/prospective claim instructions, replay-safe durable outcomes,
   two-minute penalties, forced moves, flag priority and terminal reasons.
   Correct every live 100/150 threshold, resignation, agreement and timeout path.
5. Resolve the general dead-position/mating-impossibility gap with a sound
   verified proof strategy. Shakmaty's static material check explicitly does not
   solve all dynamic cases; [CHA-Solver](https://chasolver.org/) is an upstream
   implementation to evaluate, not a dependency or SBF capability claimed here.
6. Publish the reviewed engine branch only when authorized gates are met, pin
   the exact tested revision in XFChess, and update its lockfile, ABI/IDL/decoders,
   host lifecycle, AI, network, backend, UI, history and tournament consumers.
7. Run the complete matrix, including real ER commit/undelegate/finalize/close,
   lamport conservation and retry races. Resolve full-project fmt/Clippy gates
   without suppressing warnings or overwriting unrelated user changes.
8. Only then open the reviewable PR series. Merge, program deployment, upgrade,
   funding and migration transactions still require separate authorization.

Until these gates pass, retain this as unshipped foundation work. Player-facing
instructions for the intended feature: claim on your turn before committing a
move; reaching 100 quiet plies alone will not stop upgraded games; a declared
claim move cannot be withdrawn. Those controls are not yet available in this
implementation and must not be advertised as released functionality.
