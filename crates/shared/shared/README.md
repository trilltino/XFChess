# shared

Protocol and document types shared between the game client and the backend.

## Modules

| Module | Contents |
|--------|----------|
| `protocol.rs` (re-exported at crate root) | The multiplayer message vocabulary: `LobbyMessage` (matchmaking/lobby traffic), `GameMessage` (in-game traffic), the `Channel1` channel marker, and `ProtocolPlugin` for registering the types with a Bevy app |
| `crdt.rs` | Lamport-clock-tagged message document types used for ordered chat/message sync: `LamportTimestamp`, `MessageOperation`/`MessageOpType`, `MessageEntry`, `MessageState` |

## Usage boundaries

- Both the game client (`src/multiplayer/`) and the backend deserialize these types,
  so changes here are wire-format changes — keep them backward compatible or version
  the message.
- This crate may depend on Bevy (for `ProtocolPlugin`). Types that the **web frontend**
  needs must instead go in [`backend-types`](../backend-types/), which is serde-only.
