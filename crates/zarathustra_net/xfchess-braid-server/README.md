# xfchess-braid-server

Backend Braid-HTTP resources for tournaments and game logs.

`ResourceHub` holds JSON documents and append-only logs. Initialization is atomic and
preserves existing state. A subscription captures its snapshot and receiver under the
same resource lock, then streams HTTP 209 updates and 20-second heartbeats. Subscribers
that fall behind receive a fresh snapshot before live patches resume. Gossip callbacks
run after releasing the hub's resource map lock.

`bridge` publishes tournament metadata, standings, pairings and results. The resource
codec uses `braid-http`, including JSON Patch content types and parent versions.
Consumers apply patches against the stated parent and replace state on a snapshot.
The HTTP router exposes resource reads; application services own resource writes and
authorization. Logs retain their full history and require application-level lifecycle
management when tournaments are archived.

See [ATTRIBUTION.md](ATTRIBUTION.md) for protocol provenance.
