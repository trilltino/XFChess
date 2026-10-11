---
name: braid
description: Change XFChess Braid HTTP subscriptions, Version/Parents resume, patches, durable application acknowledgments and pending writes. Use for multiplayer synchronization and spectator recovery.
metadata:
  provenance: docs-derived
---

# Braid synchronization

Read `crates/zarathustra_net/UPSTREAM.md`, the local braid-http/braid-iroh/braid_chess
modules, backend game-log handlers and the client application acknowledgment path.
These are maintained Rust implementations with application-specific game contracts.

- Check the upstream draft and reference implementation for Version, Parents,
  Subscribe and patch framing. Preserve quoted version identifiers and frontiers;
  do not interpret a frontier as a single monotonically increasing timestamp.
- Parsers must survive fragmented/coalesced reads, malformed framing, truncated
  bodies and explicit size limits. Do not treat an arbitrary HTTP chunk as one update.
- Reconnect with the last application-confirmed frontier. Delivery may enter a
  pending queue; advance durable acknowledgment after the chess engine accepts it.
- Persist pending writes before claiming durability; correlate acknowledgments
  with their exact versions and handle duplicate/reordered delivery idempotently.
- A history retention gap must trigger explicit resynchronization. Do not silently
  drop unavailable parents or pretend that partial replay reconstructed the game.
- Preserve cancellation, heartbeat deadlines and bounded retry/backoff. Keep
  spectator retry configuration explicit and test live-feed recovery.
- Braid distributes versions and patches. Legal moves, turn order, seat authority
  and settlement are enforced by the game/backend/program; text-CRDT merge policy
  cannot resolve incompatible chess moves.

Run affected Rust transport tests and the existing JavaScript interoperability
fixtures when wire behavior changes. Verify resume through the normal VPS endpoint
as well as the Iroh bridge when both carry the affected resource.

Sources: [Braid drafts](https://github.com/braid-org/braid-spec),
[reference implementation](https://github.com/braid-org/braid-http),
[fault tests](https://github.com/braid-org/braid-fuzz).
