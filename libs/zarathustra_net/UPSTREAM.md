# Upstream networking

## braid-reborn import (2026-10-25)

| Item | Value |
| --- | --- |
| Source | [braid-reborn](https://github.com/trilltino/braid-reborn) `master` @ `b53066b2926c5b0134f2d7043a1b1aa61fefdd7c` |
| Licence of imported code | MIT OR Apache-2.0 (`braid-http/LICENSE-MIT`, `LICENSE-APACHE`) |
| Base | XFChess `main` @ `d0bac0cedb2ff7bd981779ac0b26aee802715308` |
| Imported crate | `braid-http` (src, conformance tests, JS codec fixtures) |
| Adapted, not copied | `braid-iroh` access policy (braid-reborn's version sits on its `braid-core` replica store, which is not imported) |
| Not imported | `braid-core`, `braid-blob`, `braidfs-*`, `server`, browser harness, upstream fuzz/interop harnesses |
| Upstream state when imported | CI run 40205988933 was red on `cargo fmt` and the fuzz lockfile; neither applies here: the imported crate is formatted with this repository's rustfmt and the fuzz harness is not imported |

Divergences from upstream `braid-http` (kept deliberately):

1. `server.rs` — `SubscriptionResponse`, `wants_subscribe`, `resume_from` for the game-log and spectate routes.
2. `protocol::format_heartbeat` and `MessageParser::for_subscription` — bare-CRLF keep-alive between frames.
3. Patch counts given only as `application/http-patches; count=N` are accepted, must agree with `Patches:`, and `message_to_update` treats any patch list as a patched update.
4. Subscription updates carry the resource path; the formatter refuses empty patch ranges.
5. Test-only `unwrap`/`expect` replaced by `?` so `scripts/ci/check_rust_safety.ps1` passes.

Dependency changes: `iroh`/`iroh-base` 1.3.0 → 1.4.1; in-tree `iroh-h3`, `iroh-h3-axum`,
`iroh-h3-client` removed in favour of the published `iroh-h3-axum` =0.4.0 and
`iroh-h3-client` =0.5.0; `iroh-gossip` unchanged at 0.101.0.

Migration matrix: [`docs/networking/INVENTORY.csv`](../../docs/networking/INVENTORY.csv).
Architecture, ordering rules, stored state and rollback: [`docs/networking/MIGRATION.md`](../../docs/networking/MIGRATION.md).

## Earlier review (2026-10-10)

Reviewed 2026-10-10. Inventoried the Braid organization's 36 public repositories;
inspected code and tests in the nine networking-related checkouts below, plus
the Diamond Types and BraidFS documentation. Checkouts and build logs stay outside
the repository. No upstream runtime implementation was vendored.

| Source | Relevant behavior | Decision |
| --- | --- | --- |
| [braid-http](https://github.com/braid-org/braid-http/tree/639b811ec91e5d1aae4a4a7d162616b499f95b07) | Reconnects, changing Parents, HTTP/2, single/multiple patch framing | Implemented in Rust; tested actual JavaScript/Rust traffic in both directions |
| [braid-fuzz](https://github.com/braid-org/braid-fuzz/tree/b79c23f877d2e6807e3925bc0165a4da139a4458) | Reset connections, truncated updates, transient failures, cancellation, version resume | Added equivalent native regression scenarios; did not claim the full external suite passed |
| [bad-braid](https://github.com/braid-org/bad-braid/tree/befbd35936f95cd9715dfb2893106cd293959dd5) | Fault injection through a network proxy | Used local fault scenarios; no public proxy dependency |
| [braid-spec](https://github.com/braid-org/braid-spec/tree/0541922135bea0d60ad6b539e8d1ec25c2527284) | Parents requests update a known frontier; Version and Subscribe exclude each other | Bridge resume walks retained ancestors and preserves unseen branches; invalid combinations fail |
| [braid-blob](https://github.com/braid-org/braid-blob/tree/4bc83a7da5ba217732908e06863256b871fc844e) | Binary representations, version metadata and blob conflict resolution | Kept binary interoperation; its conflict policy does not define legal chess moves |
| [braid-text](https://github.com/braid-org/braid-text/tree/1b50ed357dcbbf59a1f4554690f69d0458bc967e) | Simpleton, text patches, representation digest checks | Useful for future collaborative analysis; no text merge engine added to game transport |
| [antimatter](https://github.com/braid-org/antimatter/tree/bdc891e90d4f3c2a45c9a1744eb7ad33aef27dbc) | Prunes CRDT history with peer coordination | A 500-event cache is not Antimatter; expired cursors fail explicitly instead of silently dropping events |
| [braidjs](https://github.com/braid-org/braidjs/tree/52803d16617e7fe4e6163fa6071d155cce52ff92) | Older monorepo, JSON patches and merge prototypes | Current standalone transport/tests are the implementation reference |
| [braid-testsuite](https://github.com/braid-org/braid-testsuite/tree/22569ee04616f837304a2bebef6a46a4d3b84eea) | Protocol example collection | Used the active reference implementation and fault tests for executable coverage |

The editor extensions, views, filesystem tools and hosting demos are application
integrations rather than replacements for XFChess's native transport.
[Diamond Types](https://github.com/braid-org/diamond-types) is already Rust;
collaborative annotations could use it directly instead of translating a JavaScript CRDT.

## Implemented behavior

`client/stream.rs` owns HTTP subscription delivery and reconnection. Explicit retry
configuration enables bounded retries, server retry delays, heartbeat recovery and
Parents advancement after delivery, or after explicit acknowledgment when configured.
Parse/authentication errors terminate; dropping the receiver cancels every wait.
Gameplay subscriptions acknowledge moves after engine acceptance and durable checkpoint
storage; rejected moves retain the previous cursor.

The VPS game log uses database sequence numbers consistently in replay and live streams.
Parents selects subsequent events; unknown or wrong-stream cursors return 410. Database
queries recover broadcast lag and notification races. Spectator streams use stable
versions and retry before falling back to polling.

The game journal persists pending events before publication and removes them only after
backend confirmation. Per-game and stream locks serialize writes without letting chat
failures block moves; stable versions make
retries idempotent, and resumed sessions supply current credentials. Game resume includes
pending local moves in its validated replay. A fresh board is reconstructed before old
application checkpoints can suppress history.

The wire codec handles LF and CRLF, single range patches, MIME patch counts, complete
range expressions and update status lines. Conflicting lengths/counts and incomplete
EOF are rejected. Ordinary tests exercise every split boundary for representative frames.

Iroh and iroh-base are pinned together at 1.3.0, with compatible iroh-gossip 0.101.0.
[Iroh 1.3.0](https://github.com/n0-computer/iroh/releases/tag/v1.3.0) fixes router
protocol ordering and connection responsiveness. HTTP/3 setup no longer retains its
own cache; drivers live as long as clients or active responses. Reliable streams
remain the transport for durable game updates; new datagram APIs are optional future work.

## Follow-up boundaries

`Repr-Digest` verification must hash the reconstructed representation after patches,
not an individual patch or move packet. Implement it with the application's state
recovery if collaborative documents need it. Game authorization, move legality and
recovery beyond retained history remain application responsibilities.
