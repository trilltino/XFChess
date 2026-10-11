# Release verification

Record the source commit, platform, environment, program binary hash and test
results with the release or pull request. A compiled fix does not prove a live
wallet, device, VPS or MagicBlock flow. Identify ignored tests and mock services.

## Local checks

Run checks for the components changed:

```text
cargo check --bin xfchess --features solana
cargo test -p backend --lib
cargo test -p backend --test e2e_api
cargo test -p xfchess-tauri --features tournament-admin
cargo test -p cu_benchmark --all-targets
```

Build SBF before program-test: `just build-program`, then
`cargo test -p xfchess-game --no-fail-fast`. Confirm the runner loads the fresh
binary. [Protocol models](../../specs/README.md) test bounded abstractions; they
do not replace Rust integration tests. Run frontend tests/builds in the affected
npm project. [Ops validation](../../ops/docs/E2E_REMEDIATION.md) covers proxy,
release recovery, backup mechanics and monitoring fixtures.

## Two-client multiplayer rehearsal

Exercise casual and wagered games with independent clients. Inspect both boards,
the durable log and, for wagers, the authoritative account.

| Boundary | Required outcome |
| --- | --- |
| Disconnect and P2P/relay loss | Catch-up without duplicate moves or a false winner |
| Client crash and restart | Same-wallet resume restores legal board, ply and causal/chain cursors |
| Backend restart | Accepted participants and persisted moves survive; subscriptions recover |
| Duplicates, reordered delivery, lost acknowledgement | One application per move; resume never skips an unapplied move |
| Wallet switch or second device | Previous identity cannot keep writing; superseded device loses its seat |
| Cancel/join and terminal-action races | One legal outcome; no duplicate refund, payout or rating update |
| Wallet rejection or RPC timeout | Cancel rejected work; reconcile unknown submissions before retry |
| New match in the same process | No previous board, pending write, seat or cursor leaks into the next game |
| Spectator reconnect and replay | Ordered recovery without player authority or early access to delayed data |

Check checkmate, stalemate, repetition, fifty-move rule and insufficient material.
Restored per-side clocks need an explicit test; on-chain inactivity timeout and
displayed chess clocks are different boundaries.

## Wagers and tournaments

Follow [the MagicBlock devnet flow](../runbooks/magicblock-lifecycle-devnet.md)
and [program upgrade verification](../runbooks/program-upgrade.md). Exercise ER
failure at delegation, moves, commit and undelegation. For an unknown submission
result, inspect the signature and account owner/state before resending.

Verify participant/session authority, nonce/parent checks, terminal result,
commit/undelegation, escrow conservation, refunds, fees, ratings and replay
rejection. A transport acknowledgement is not payout proof. A closed Game PDA
needs transaction/escrow evidence during recovery.

Run complete single-elimination and Swiss tournaments, including registration,
automatic game creation, disconnect/forfeit, advancement, byes, prizes and safe
close. Reconcile guaranteed prizes and entry fees against the chain. Review when
anti-cheat analysis occurs relative to each payout; post-settlement analysis
cannot undo a paid 1v1 wager.

## External release gates

Test desktop wallet rejection, popup placement, concurrent signing and game
closure during a request. Verify the bridge/popup terminate with the game.
Android requires a physical-device wallet and background/resume pass.

On staging, verify TLS, private admin/metrics, health commit, readiness and a
stopped-service restore with compatible snapshots. Measure recovery time. Before
real-money launch, review keys, funding, deployed program/IDL compatibility,
recovery evidence and jurisdiction-specific requirements. This checklist does
not claim these external gates have passed.
