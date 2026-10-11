# Protocol models

Bounded TLA+ checks for multiplayer acceptance, delivery and on-chain ordering.
These are executable abstractions, not generated specifications or proofs of the
Rust implementation. Keep them when changing the corresponding protocol rules.

| Directory | Model | Code boundary |
|---|---|---|
| `causal/` | Per-sender sequence, parent and identity checks | [gossip acceptance](../src/multiplayer/systems/events.rs) |
| `braid/` | Established participant authorization; duplicate delivery and application checkpoints | [game log](../backend/src/signing/routes/game_log.rs), [transport](../src/multiplayer/network/braid_transport.rs), [board application](../src/game/systems/network_move.rs), [journal](../crates/zarathustra_net/braid_chess/src/journal.rs) |
| `settlement/` | Session authorization, strict nonce ordering and optional parents | [record_move](../programs/xfchess-game/src/moves_ix/record.rs), [move application](../programs/xfchess-game/src/moves_ix/apply.rs) |

Identity binding lives in [mesh.rs](../src/multiplayer/systems/mesh.rs);
[CausalChainState](../src/multiplayer/types.rs) owns the sender lanes and reservations.

Run from the repository root with Python 3.10+ and Java 11+:

```powershell
just check-specs --setup
just check-specs
just check-specs --suite braid
```

On other platforms, use `python3 specs/check.py` with the same arguments.
`--setup` downloads the checksum-pinned [TLC 1.7.4 release](https://github.com/tlaplus/tlaplus/releases/tag/v1.7.4)
into `specs/.tools/`. Checking never downloads or upgrades tools implicitly.
`--list` lists cases; `--case NAME` runs one case; `--timeout SECONDS` changes
the per-case limit. Logs and counterexamples go to ignored `specs/.runs/` directories.
The [CI workflow](../.github/workflows/specs.yml) runs the same suite on changes
to the models and their implementation boundaries, retaining TLC logs as artifacts.

Each configuration names its expected outcome in its first line. Normal cases
must finish successfully. `mutant-*` cases deliberately remove one safeguard and
must violate the named invariant: an unrelated error or unexpected success fails
the runner. These counterexamples replace the old saved attack trace and the
ambiguous `current`/`fixed` configurations.

The checks cover:

- **CausalChain:** sender lanes remain parent-linked, sequences have no gaps,
  and unauthenticated messages cannot enter a lane. Honest liveness assumes
  weak fairness and no permanent message loss. Byzantine cases check safety.
- **BraidAuth:** with an established participant set, only its members append
  causally linked events. Deduplication is checked separately in `Delivery`.
- **Delivery:** gossip/Braid copies share pending reservations; board acceptance
  precedes durable checkpointing. Rejection, replay and restart cannot make a
  resume cursor skip unapplied positions or apply a duplicate within the current
  board history. A fresh process clears its cursor and replays history; the model
  does not assume restoration from the local journal or assert exactly-once
  execution of side effects across crashes.
- **SolanaFinality:** committed nonces are contiguous; supplied parents match;
  authors are authorized. Replica submitters relay player-authorized transactions
  and do not become extra authorized players.

Scope matters: content hashes and signature verification are abstracted as
records and booleans. Participant bootstrap, key expiry, chess legality/turns,
stream framing, multi-stream frontiers, RPC/ER settlement and disk failure
behavior are outside these models. Delivery models a single ordered move stream
and atomic checkpoint/FEN persistence; it does not model pending outgoing writes.
The implementation's tests remain necessary for those boundaries.

Bounds are visible in each `.cfg`: causal adversarial exploration caps the
in-flight set at three messages; settlement caps submissions. These constraints
exclude larger executions. TLC exhaustively checks the configured finite state
space, not arbitrary game lengths. No historical state counts are treated as
current verification results.
