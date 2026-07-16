# backend/src/signing/social

Friends, presence, and social routes. Complements the **on-chain** friends graph
(`Friendship` PDA in [programs/xfchess-game/src/state/friendship.rs](../../../../programs/xfchess-game/src/state/friendship.rs))
with fast off-chain presence and lookups.

## Files

| File | Contents |
|------|----------|
| [friends.rs](friends.rs) | `FriendManager` — friend lists and requests |
| [presence.rs](presence.rs) | `PresenceStore` — online/in-game status |
| [routes.rs](routes.rs) | HTTP endpoints for the above |

## Invariants

- Presence is ephemeral in-memory state; friendship of record is the on-chain PDA.
