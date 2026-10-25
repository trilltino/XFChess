# Networking migration: braid-reborn import

Source: [braid-reborn](https://github.com/trilltino/braid-reborn) `master` @
`b53066b2926c5b0134f2d7043a1b1aa61fefdd7c` (MIT OR Apache-2.0). Target base:
`origin/main` @ `d0bac0cedb2ff7bd981779ac0b26aee802715308`. File-by-file mapping:
[`INVENTORY.csv`](INVENTORY.csv). Provenance and the dependency table:
[`libs/zarathustra_net/UPSTREAM.md`](../../libs/zarathustra_net/UPSTREAM.md).

## Before and after

| Layer | Before | After |
| --- | --- | --- |
| Wire codec, client, retry | in-tree `braid-http` (older braid-reborn lineage) | `braid-http` = braid-reborn snapshot + four documented XFChess additions |
| Reliable channel, journal | none | `ReliableChannel`, `ChannelJournal`/`FileJournal`, payload and credential scoping (imported) |
| Publishing chess events | blocking `reqwest` loop per stream, `Journal` outbox, publisher lock files | `braid_chess::ChessOutbox`: one `ReliableChannel` per stream, durable before send |
| Move/chat subscription | one task for both, shared cursors | `braid_chess::StreamHandle` per stream: own task, queue, cursor, state |
| Iroh / H3 | Iroh 1.3.0, in-tree `iroh-h3*` forks, no peer check | Iroh 1.4.1, published `iroh-h3-axum` 0.4.0 / `iroh-h3-client` 0.5.0, default-deny `AccessPolicy` |

## Event ordering and trust

* The backend game log stays the single ordered ledger. Nothing here merges
  chess positions; the generic CRDT/merge parts of braid-reborn are not imported.
* **Identity of a logical event** is its `content_version` (a hash of the
  position/move). Author, payload, version and signature never change after the
  event is queued.
* **Parent** is a property of a *transport attempt*. The outbox stores the parent
  that was the stream head when the event was queued. If the backend answers
  `409 {expected_parent}` the attempt is repeated with that parent; the stored
  event is untouched. The backend checks whether a version is already committed
  *before* it checks the parent, so an attempt that follows a lost acknowledgement
  is recognised, not duplicated (`retry_after_lost_ack_reuses_original_sequence_even_after_head_moves`).
* **Who may write** is still decided by the backend: wallet/session token, seat
  lease and device id. A 409 `seat_superseded`, 401 or any non-retryable 4xx stops
  that stream in `RepairRequired` with the outbox intact. The app re-offers
  credentials every 30 s (`retry_pending_writes` → `ChessOutbox::resume`).
* **Iroh transport identity is not player identity.** The H3 interface denies every
  peer by default; read and write are granted separately and a revoked peer's open
  subscription ends. Chess legality, wallet pairing and settlement are unchanged.

### Durable states of an event

1. *Queued*: committed to `outbox.<stream>.json` (atomic write, writer lease)
   before the first send. May be ambiguous after a send whose response was lost.
2. *Backend-confirmed*: the channel observed an accepted PUT; the app records the
   new stream head and then calls `commit_ack`, which removes the entry.
3. *Engine-applied* and *finality-confirmed* are the application's own states and
   are never inferred from a network receipt.

## Receive side

`StreamHandle` hands the engine one update at a time and does not read the next
frame until the application accepts or rejects it. The cursor advances only after
the engine accepted the move **and** the journal checkpoint was written
(`confirm_move`). A 410 clears only that stream's cursor; sibling cursors and the
outbox are untouched. Moves are drained before chat.

## Stored state and rollback

| File (per game, per device, under `game-sync/<hash>/`) | Owner | Notes |
| --- | --- | --- |
| `state.json` | legacy `braid_chess::journal::Journal` | still holds applied checkpoints; `pending` is drained into the channel outbox |
| `outbox.moves.json`, `outbox.chat.json` (+ `.lock`) | `ReliableChannel` | new; atomic replace; first migration backup via `backup_original` is not needed because no format changes in place |

**Upgrade.** On first use a stream's channel adopts every legacy pending write:
queue (idempotent by version) → commit → then remove the legacy entry. A crash in
between leaves the event in both places and the migration simply runs again; the
backend deduplicates by version, so nothing is sent twice as a distinct event.

**Rollback to an older build.** The older build ignores `outbox.*.json`. Events
that were adopted into the channel and not yet confirmed would not be retried by
it. To roll back safely: let the app run online until the outbox is empty (the
move list in the backend matches the local game), or copy the pending events back
by hand. Nothing is deleted by the upgrade.

## Limits and known gaps

* The H3 interface authorises *peers*, not chess players or resources; per-resource
  ACLs from braid-core were not imported (no reachable XFChess use).
* Queue-full (64 events per stream) is reported as an error to the caller and
  logged; the game does not yet surface it in the UI.
* Gossip remains best-effort. Authoritative state repair goes through the backend.
* Relay/NAT partition behaviour is not tested here (needs real network
  infrastructure); braid-reborn lists the same case as open.
* The `wasm` feature of `braid-http` is imported but the WASM target was not
  built as part of this change.
* A permanently undecodable event on the move stream is re-delivered after every
  reconnect, as before; it is logged but not yet quarantined.
