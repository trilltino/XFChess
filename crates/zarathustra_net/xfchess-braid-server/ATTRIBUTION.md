# Attribution

This crate implements the [Braid-HTTP protocol](https://braid.org/) (HTTP 209,
`Subscribe: keep-alive`, multipart/mixed version framing).

The protocol design is by the Braid working group.  The XFChess adaptation was
seeded from [braid-reborn](https://github.com/braid-org/braid-rs) by the Braid
Team, licensed MIT OR Apache-2.0.  A copy of those licenses is preserved in
`LICENSE-MIT` and `LICENSE-APACHE` at the workspace root.

XFChess-specific changes:
- Replaced chat/FS resource model with chess tournament resources.
- Replaced bcrypt auth with Solana session-key auth.
- Removed CRDT text-merge; uses JSON Patch (RFC 6902) for standings/pairings.
- Append-log backend for ordered move sequences.
- Bridge to existing iroh gossip infrastructure.
