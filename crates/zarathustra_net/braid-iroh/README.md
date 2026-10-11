# braid-iroh

Iroh gossip and resource history for multiplayer. `BraidIrohNode` owns the endpoint,
router, subscription manager and background tasks. `shutdown()` stops tasks, flushes
configured storage and closes the router; dropping the node aborts background tasks.

| Module | Responsibility |
| --- | --- |
| `node` | Configuration, lifecycle and public resource operations |
| `discovery` | Real or isolated in-memory peer discovery |
| `subscription` | Gossip topic derivation, publishing and peer joins |
| `store` | Normalized resource keys, bounded history, local subscription fan-out |
| `persistence` | JSON load and atomic file replacement |
| `protocol` | HTTP resource routes and streaming Braid responses (`proxy` feature) |
| `proxy` | TCP bridge to local resources or a remote HTTP/3 peer (`proxy` feature) |

History retains 500 updates per resource and ignores repeated retained versions.
Version lookup uses exact identifiers. HTTP subscriptions replay retained history
before live updates. A subscription's `Parents` frontier excludes known updates and
their ancestors while retaining unseen branches. Unknown or expired heads return HTTP 410;
the caller must recover from an authoritative state rather than skip missing moves.
HTTP PUT accepts full payloads with Version/Parents metadata;
Braid range patches are rejected because this store does not implement a merge engine.

`BraidIrohConfig::secret_key` accepts an existing identity; absent keys are generated
randomly. `spawn_node` names are labels. It starts a bridge only when a port is supplied.
For persistent history, set `data_dir`. Corrupt storage fails startup instead of being
silently replaced. Call `shutdown()` to guarantee the final flush.

The bridge is used by desktop spectators. Local resources use the same router directly
because Iroh rejects self-connections. Remote resources stream through `iroh-h3-client`.
