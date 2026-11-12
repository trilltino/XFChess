# Friends architecture

Friends live entirely in the backend. The `xfchess-game` program has no friendship account or instruction, so a friend action costs no rent, no network fee and no wallet prompt. This replaces an earlier design that kept a second, on-chain graph.

## What exists

| Layer | Implementation | Responsibility |
| --- | --- | --- |
| Client cache and polling | [social/service.rs](../crates/xfchess-client-bevy/src/multiplayer/social/service.rs) | Contacts, requests, invitations, blocks; one write at a time; refreshes about every 10 s. |
| Client window | [social/ui.rs](../crates/xfchess-client-bevy/src/multiplayer/social/ui.rs) | Friends, Requests, Invitations and Blocked tabs. Block and remove ask for confirmation. |
| HTTP adapter | [vps/profile_social.rs](../crates/xfchess-client-bevy/src/multiplayer/network/vps/profile_social.rs) | Signs every call with the profile's node token. The pooled client also carries the wallet JWT, and requests name the connected wallet. |
| Routes | [social/routes.rs](../services/backend/src/signing/social/routes.rs) | Node ownership, wallet verification, addressee checks, block checks, invitations. |
| Store | [social/friends.rs](../services/backend/src/signing/social/friends.rs) | SQLite requests, contacts, wallet friendships, wallet-to-node map, blocks, invitations. |
| Schema | migrations `011_friends`, `015_social_invites`, `016_wallet_social` | One idempotent file per area. |

## Identities

- A **node ID** (the device's Iroh key) identifies a profile on a device. Contacts, requests and invitations are addressed to node IDs.
- A **wallet** counts only when the bearer token proves it. A claimed wallet that the token does not prove is dropped, never recorded.
- Node IDs and wallets are both base58 32-byte keys, so the API takes typed fields (`to_node_id`, `to_pubkey`) and never guesses from the text.

## Wallet friendships follow the wallet

When both sides of an accepted request are verified wallets, the backend stores a `wallet_friendships` row (sorted pair). It outlives either device:

- Presence heartbeats from a verified wallet record the node it is playing from (`wallet_nodes`). The write happens only when the node changes or after a 30 s gap.
- `GET /friends` with a wallet token calls `sync_wallet_contacts`. It creates contacts for friends this device has not met and refreshes the route to friends who moved to another device. Blocked players are skipped.
- Wallet friends can invite each other without a node-level request on the current device.
- Removing a friend deletes the wallet friendship too, so it does not come back.

Limit: a friend appears on a new device only once the backend has seen their wallet play (it needs their last node). A friend who has never connected a wallet to the backend is not listed.

## Adding by username

Accounts are added by username; guests (no account, no wallet) are added by ID.

- `POST /friends/requests` accepts `to_username` instead of `to_node_id`/`to_pubkey`. The backend resolves it, exact and case-insensitive, to the account wallet in `users` and addresses the request to that wallet. Erased accounts and email-only accounts with no wallet never match.
- Lookups reveal whether a username exists, so they are rationed: 10 per node per minute and 300 per minute overall (node keys are free to create, so the per-node limit alone is not enough). Over the limit returns 429.
- The request stores the addressee's name (`to_display`), so the sender sees a name in the Requests tab. Contacts, blocks and invitations show the stored display name and fall back to a short ID.
- A request addressed to a wallet is visible only when the client names that wallet when listing (`pubkey` query) and the bearer token proves it. The client now does this.
- The client Add field takes a username or an ID. A 32-byte ID is 43+ characters and usernames are at most 30, so the two cannot be confused. Rows have Copy ID as a fallback.

## Blocking

`blocks` is directional and keyed by the blocker's node, with both wallets recorded when known, so a block holds when the blocked wallet returns from another device.

Blocking someone: removes the contact in both directions, deletes requests between the pair, deletes their invitations, and deletes any wallet friendship. While the block stands, requests in either direction, accepting an old request, and invitations are refused. The refusal messages do not say who blocked whom.

Routes: `GET/POST /friends/blocks`, `DELETE /friends/blocks/{blocked_node_id}`.

## Requests and invitations

1. `POST /friends/requests` (to a node ID or wallet). Duplicate pending requests are rejected; old rejected or accepted rows are cleared so a removed friend can be re-added.
2. The addressee (the node, or the wallet it was addressed to) answers with `PUT /friends/requests/{id}`. Accepting is one transaction: it updates the request, creates both contacts and, for two verified wallets, the wallet friendship.
3. `POST /friends/invite` requires an accepted friendship (contact or wallet friendship), no block, and, for public lobbies, an open, free, passwordless lobby hosted by the sender. Invitations persist in SQLite with a 15-minute expiry and a cap of 50 per recipient.

## Not done

- A friend list for a wallet that has never been seen by the backend.
- Merging two separate nodes' contact lists when no wallet is involved; those graphs stay per node.
- Rate limits on requests and invitations.
- Request delivery and presence still require the backend; there is no peer-to-peer fallback.
