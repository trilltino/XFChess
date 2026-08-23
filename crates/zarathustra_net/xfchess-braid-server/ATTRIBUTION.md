# Attribution

This crate implements the [Braid-HTTP protocol](https://braid.org/) (HTTP 209,
`Subscribe`, version/parents update framing).

The **protocol design** is by the [Braid working group](https://braid.org), specified in
[`draft-toomim-httpbis-braid-http`](https://datatracker.ietf.org/doc/html/draft-toomim-httpbis-braid-http).

The **Rust implementation** is by the XFChess author, seeded from their own
[braid-reborn](https://github.com/braid-org/braid-rs), licensed MIT OR
Apache-2.0. A copy of those licenses is preserved in `LICENSE-MIT` and
`LICENSE-APACHE` at the workspace root.

> **Licensing note.** The workspace root `LICENSE` is AGPL-3.0 (the game), while
> `[workspace.package]` declares `MIT/Apache-2.0` and these crates derive from
> MIT/Apache-2.0 code. The `braid-*` crates are MIT OR Apache-2.0; the AGPL
> covers the game, not this protocol stack. Resolve the per-crate `license`
> fields before publishing any of them.

XFChess-specific changes:
- Replaced chat/FS resource model with chess tournament resources.
- Replaced bcrypt auth with Solana session-key auth.
- Removed CRDT text-merge; uses JSON Patch (RFC 6902) for standings/pairings.
- Append-log backend for ordered move sequences.
- Bridge to existing iroh gossip infrastructure: the hub's gossip sink carries
  each published Braid update to P2P peers, so HTTP and gossip subscribers
  receive the same versioned update.

## Known doc drift, corrected

Earlier revisions of these docs described two things this code does not do:

- **`Subscribe: keep-alive`** — the current draft leaves the value open ("may be
  blank, set to `true`, or contain arbitrary data"); this workspace sends
  `true`. Servers here accept both spellings.
- **multipart/mixed framing** — Braid has no MIME boundaries. This crate did
  emit them once; our own parser skipped the boundary lines, so nothing here
  noticed, but no braid.org tool could read the streams. Fixed; see
  `src/resource/protocol.rs`.
