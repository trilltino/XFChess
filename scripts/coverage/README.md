# Coverage and surface inventory

Phase 0 of [the coverage plan](../../docs/TEST_COVERAGE_PLAN.md) measures
the existing suite before new behavior tests. The committed baseline separates
measured packages from pending ones; missing data is never reported as 0% or 100%.
HTML and LCOV reports are build/CI artifacts, not committed files.

Install `llvm-tools-preview` with rustup, then
`cargo install cargo-llvm-cov --version 0.9.1 --locked`. Run from the repo root:

```text
node scripts/coverage/run-rust.mjs application
node scripts/coverage/run-rust.mjs contract
node scripts/coverage/run-rust.mjs workspace
node scripts/coverage/run-rust.mjs proxy
node scripts/coverage/run-rust.mjs desktop
```

The contract lane needs `target/deploy/xfchess_game.so` and the two hash-verified
fixtures fetched by `docs/security/fetch_{pass,dlp}_fixture.py`, exactly as in CI.
The desktop lane needs both Tauri frontend builds. Each group has its own target
directory to avoid mixing features or overwriting instrumentation. The `coverage`
Cargo profile retains debug symbols and disables dependency optimization, overriding
the normal stripped, optimized development profile.
`engines` is a small local smoke group, not a replacement for the application lane.

Windows program-test dependencies can spend a long time rebuilding vendored
OpenSSL with nmake. An existing **exact-version** static build may be reused by
setting `OPENSSL_DIR` to its install directory, `OPENSSL_NO_VENDOR=1` and
`OPENSSL_STATIC=1` for the local test process. Verify `openssl/opensslv.h` against
the locked openssl-src version first; do not substitute the system installation
silently. This is an optional local build optimization, not a CI requirement.

Rust branch coverage is **unmeasured** on stable. Run the group with `--branch`
under a nightly default toolchain to measure it; do not count region coverage as
branch coverage. The on-chain `.so` cannot supply Rust line coverage: instruction
evidence and mutation tests remain separate requirements. Only its host-compiled
chess/state helpers can be meaningfully measured.

Each frontend supports `npm run test:coverage`, using V8 and including unimported
source files. The website currently has no Vitest tests: its initial measurement
uses `--passWithNoTests` to record 0%, while the existing Playwright suite remains
the functional gate. Do not use that flag for new suites to hide removed tests.

```text
node scripts/coverage/collect.mjs rust target/coverage/application/llvm.json application target/coverage/application/summary.json
node scripts/coverage/collect.mjs js tauri/wallet-ui/coverage/coverage-summary.json wallet-ui target/coverage/wallet-ui.json
node scripts/coverage/check-baseline.mjs target/coverage/wallet-ui.json
node --test scripts/coverage/tooling.test.mjs
```

The ratchet compares exact covered/total ratios. Rust floors are keyed by OS and
feature group; Windows results cannot establish Linux floors. First measurements
are informational until their reviewed rows are committed in
`docs/coverage-baseline.json`. Subsequent PRs are compared to the **base commit's**
baseline as well as the proposed baseline, preventing a PR from silently lowering
its own floor. Maintainers can apply `coverage-allow-decrease` for an intentional
removal; test or instrumentation failures still fail the job.

## Matrices

```text
node scripts/coverage/instructions.mjs --check
node scripts/coverage/ipc.mjs --check
node scripts/coverage/ignored.mjs --check
node scripts/coverage/routes.mjs backend --runtime target/coverage/application/surfaces/backend-routes.json --check
node scripts/coverage/routes.mjs bridge --runtime target/coverage/desktop/surfaces/bridge-routes.json --check
```

Run without `--check` to regenerate markdown after reviewing the corresponding
JSON evidence. References use `path/to/test.rs::test_function`. Instruction name
matches are only candidate counts. Reviewed matrix cells require assertions for
that specific case; neither a name match nor a 2xx response fills one automatically.
Existing gaps are explicit backlog, not a claim of full coverage.

Backend and bridge inventories inspect the **constructed Axum router**, so aliases,
nesting, conditional routes and methods reflect registration at runtime. Axum 0.8
has no public iterator: the test adapter reads its Debug representation and fails
if that format changes. It must be checked against synthetic routers and dispatch
when upgrading Axum. Inventory tests never run application handlers and do not
count as success/failure evidence.

The backend's opt-in `coverage` feature adds a middleware that writes method,
matched path, status and test thread name to per-process JSONL files when
`XFCHESS_COVERAGE_DIR` is set. No concrete URL, body, bearer token or wallet is
logged. E2E requests through the shared harness are observed; inline tests that
construct independent routers still need this layer attached before they can
contribute runtime evidence. `--hits DIRECTORY` produces an observation report
for review. It never automatically upgrades a matrix cell.

The ignored-test audit distinguishes child-process fixtures, live tests, offline
nightly tests, manual tools and quarantines. Do not run blanket `--ignored`:
there are child fixtures and an infinite fuzzer. Explicit live runs now fail when
their prerequisite is missing. Individual owner assignments and funded nightly
RPC credentials remain operational work; component roles in the audit are not
invented individual assignments.

## CI and completion

`coverage.yml` uploads per-group JSON, LCOV and HTML. The manually triggered
`test-soak.yml` executes the existing full CI split ten times and preserves logs
and failures by test name. It stops on build/prerequisite failures, which are not
evidence of flakes. A passing soak has to be recorded before Phase 0 is complete.

Do not mark M0 complete until all package reports, both runtime HTTP snapshots,
reviewed matrix counts, the ignore audit and the ten-run flake report exist.
See `docs/coverage/EXECUTION.md` for measured results and outstanding milestones.
