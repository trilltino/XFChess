# Networking Hardening Plan — P2P / dual-transport audit

**Status:** P0 shipped 2026-08-02. Braid-first-class migration (supersedes
P1/P2/P3 below) also shipped 2026-08-02 — see "Braid-first-class migration"
section below. P4/P5 remain backlog.

## Context

End-to-end audit of the P2P networking layer (`src/multiplayer/`,
`crates/zarathustra_net/`, the backend VPS relay, and the on-chain
disconnect/timeout/dispute path) requested to harden move delivery, fix
disconnect/reconnect handling, and connect the UI more tightly to
on-chain settlement outcomes for wagered games. The request called out five
specific risk areas to verify; each is addressed explicitly below.

Money moves through this system via wager escrow settled on-chain
(`programs/xfchess-game/src/game_ix`, `governance_ix`). The P2P layer itself
never touches funds directly, but a P2P bug that desyncs a client's local
board from what actually happened can produce a game state the player
disputes, or a client that never learns it should call a settlement
instruction — both are real (if indirect) money-adjacent failure modes,
which is why the dual-transport seam got the deepest pass.

## What shipped now (P0)

**The seam:** every online move is broadcast over both Iroh gossip and the
VPS relay (`src/multiplayer/network/online_game_session.rs`'s "Dual
transport" comment), with independent, uncorrelated latency — confirmed
against iroh-gossip's own docs, which make no delivery-order guarantee
(epidemic broadcast, not a reliable ordered channel). Before this fix, the
receiver's replay check in `handle_network_events`
(`src/multiplayer/systems.rs`) only rejected `nonce < expected` and accepted
anything else, jumping `expected` forward — so if move N+1 ever arrived
before move N, N was permanently rejected as a "replay" it never was. This
is exactly the heisenbug class flagged: fine 99% of the time, silent
desync the other 1%, worst in the disconnect/reopen and close/reopen paths
because those are where transport timing diverges most.

**The fix** (`src/multiplayer/network/reorder.rs`, new file):
- `NonceSequencer<T>` — a pure, Bevy-free per-game reorder buffer. Out-of-order
  arrivals are held (bounded at 8 entries, `PendingMoveBuffer::MAX_BUFFERED`
  in `src/multiplayer/types.rs`) and released in strict nonce order once the
  gap fills, instead of skipping past and orphaning the gap.
- A bounded buffer alone can't catch a *genuinely* dropped move with no
  further traffic (the mover whose turn it is has nothing to send until they
  see the move that would fill the gap — the buffer never grows past one
  entry). `sweep_stale_move_buffers` (`systems.rs`) runs every frame and
  calls `NonceSequencer::expire()` once a buffered entry has waited past
  `PendingMoveBuffer::STALE_AFTER` (5s), forcing a
  `NetworkMessage::ResyncRequest` — which the existing
  `handle_resync_request`/`handle_resync_response` pair already answers by
  overwriting the receiver's board with the authoritative FEN
  (`systems.rs`, pre-existing, previously unused for this purpose).
- Wired into `handle_network_events` via a two-queue design (`incoming` →
  gate → `ready`) specifically so a message released from the buffer is
  never re-fed through the gate a second time (which would misclassify it as
  a duplicate — an actual bug caught during implementation, before the fix
  landed).
- **Side fix, same code path:** `Resign` was sharing this nonce gate via a
  hardcoded `nonce: 0` (both the Resign-button and exit-to-menu paths). Since
  `expected` never drops below 1, every resign was silently dropped the
  moment any real move had been sent for that game — resigning never
  actually reached the opponent over P2P. Resign is a terminal, idempotent,
  order-independent signal, not part of the ordered move stream, so it's now
  exempted from the gate entirely rather than needing a correct nonce.
- `OnlineNetworkState.expected_nonces` (the old, now-fully-superseded
  mechanism) was removed rather than left as dead code.

**Test:** `dual_transport_interleave_duplicate_drop_fuzz`
(`src/multiplayer/network/reorder.rs`, 500 trials, deterministic xorshift PRNG
— no new `proptest`/`rand` dependency added to the workspace for one test file).
Feeds a `NonceSequencer` arbitrarily interleaved, duplicated, and
single-transport-dropped arrivals and asserts the applied sequence is always
strictly increasing (no fork, no regression, no duplicate application) and,
when no resync ever triggers, an exact gap-free `1..=N` run. Six additional
targeted unit tests cover single reorder, deep multi-message reorder,
duplicate rejection, permanently-dropped-triggers-overflow, and the
sparse-gap/no-further-traffic case the count bound alone can't catch.
`cargo test --lib multiplayer::` — 28/28 pass.

## Answers to the five flagged risk areas

1. **Dual-transport dedup seam** — fixed above. Root cause confirmed via code
   read, not speculation: two independent, uncoordinated dedup mechanisms
   existed (`expected_nonces` per-game, `CausalChainState.last_seq` per-agent)
   and neither buffered — both just accepted-and-skipped or rejected-and-dropped.
2. **`version_hash` truncation/collision** — confirmed real
   (`crates/zarathustra_net/braid_chess/src/patch.rs`, sha256 truncated to 8
   bytes) but `BraidPatch::from_message`, the only call site that would ever
   hit a non-move-event collision, has **zero callers** anywhere in the repo —
   real resign/draw/offer events go through `NetworkMessage` directly, not
   `BraidPatch`. Backlog item P3 below: delete the dead path rather than
   patch a seed nobody uses.
3. **Gossip trust model** — confirmed no authentication gates who can
   subscribe to or broadcast into a game's gossip topic before on-chain join;
   roster membership is a first-two-pubkeys-seen race
   (`CausalChainState.roster`, `systems.rs`). Backlog item P1.
4. **Clock authority** — confirmed *not* a money-safety issue: `NetworkMessage::Clock`
   is self-asserted and cosmetic-only (spectator display). On-chain
   `ClaimTimeout` (`programs/xfchess-game/src/game_ix/timeout.rs`) is fully
   self-contained — `game.turn` parity + `game.updated_at` (written only by
   verified on-chain move records) + Solana's own `Clock::get()`. No
   client-reported clock value ever reaches a payout decision.
5. **On-chain disconnect knowledge vs. UI wiring** — confirmed the contract
   side is solid and self-contained (see above), and the *client* already has
   real disconnect UI (`opponent_disconnect_ui`, `disconnect_recovery_banner`,
   ping chip, online/offline dot — `src/ui/game/game_ui.rs`,
   `src/ui/game/player_bar.rs`) plus a correctly-labeled abandonment result on
   the game-over screen (`WhiteWonByAbandonment`/`BlackWonByAbandonment` →
   "opponent disconnected", `src/ui/menus/game_over_popup.rs`). The gap is
   that **nothing in `src/` ever calls `ClaimTimeout`** — the on-chain half
   that actually resolves the wager is fully unwired from the disconnect UI
   that detects the problem. Backlog item P4 (decision recorded: client
   should auto-submit once the local timeout window elapses).

## Braid-first-class migration (shipped 2026-08-02, same day as P0)

Follow-on work, same session: replaced the poll-based VPS move-relay
(`p2p_send_message`/`p2p_poll_messages` used *for moves specifically*) with
a durable, push-based Braid transport. Full design rationale in this
session's plan file (now executed); summary here for anyone landing on this
doc cold.

**What shipped:**
- `crates/zarathustra_net/braid_chess/src/patch.rs`'s `version_hash` widened
  from 8-byte to full 32-byte SHA-256 hex — it's now the durable,
  dispute-evidence-grade causal-chain key, not just a P2P equivocation
  check. `BraidPatch`'s single-parent chain documented as deliberately
  linear (chess has no concurrent-move branching) — this closes old P3's
  "delete the dead truncated-hash path" differently than planned: the path
  turned out to be worth making real instead of deleting.
- New `backend/src/signing/routes/game_log.rs` + `backend/migrations/027_game_event_log.sql`:
  a durable, ordered, per-game event log (`GET`/`PUT /game/:id/moves` and
  `/game/:id/chat`) with real SQLite persistence (survives a backend
  restart — the old relay/chat state didn't) and server-side causal-chain
  validation (rejects a PUT whose claimed parent doesn't match the log's
  true head — the third independent layer, alongside the P2P equivocation
  guard and the on-chain `parent_nonce` check, enforcing the same
  invariant). `chat.rs`'s old ephemeral `ChatRelayState` (empty snapshot on
  every reconnect, no persistence) was deleted and replaced by this same
  infrastructure.
- **Does *not* use `xfchess_braid_server::ResourceHub`/`AppendLog`** despite
  that being the obvious-looking shared-crate tool — found via a real
  regression test that its JSON-Patch-wrapped live-update bodies and
  bulk-array snapshot don't decode through `ChessSubscriber` (which only
  ever parses a chunk body as one bare `ChessMessage`, since nothing
  previously subscribed to an `AppendLog` through it). `game_log.rs` keeps
  its own `broadcast::Sender<BraidUpdate>` map instead — same wire shape
  chat always used, now backed by SQLite. See the module's doc comment for
  the full reasoning; `every_broadcast_body_decodes_as_a_bare_chess_message`
  is the regression test.
- `src/multiplayer/network/braid_transport.rs` (new): publish helpers
  (`publish_move`/`publish_resign`/`publish_chat`/`publish_session_info`)
  and a reconnect-wrapped `ChessSubscriber` wrapper (bounded exponential
  backoff; `ChessSubscriber` itself has zero reconnect logic anywhere it's
  used, including chat/tournament streams). Because the backend replays
  full history to every new subscriber, a reconnect is automatically a
  correct catch-up.
- `CausalChainState.applied_versions` (new field, `types.rs`): Braid-delivered
  moves don't carry a P2P `nonce` (`ChessMessage::Move`/`MovePayload` is a
  different wire type with no such field), so they can't go through
  `NonceSequencer` — they're dispatched directly (Braid is already
  ordered). This set catches the resulting cross-transport case: a move
  delivered by both gossip and Braid must not double-apply to the board.
  Checked/inserted on both the gossip path (`systems.rs`'s existing
  causal-chain block) and the Braid path (`braid_transport::drain_braid_messages`).
- `relay_bridge.rs` deleted — its 6 call sites (`Move`, `Resign`, `Chat`,
  `Ping`, `Pong`, `SessionInfo`) migrated: `Move`/`Resign`/`Chat` to Braid;
  `Ping`/`Pong` and `SessionInfo` scoped out of the Braid migration (see
  below) and now gossip-only, each with a specific replacement liveness
  mechanism (see next point) rather than a silent capability loss.

**Two near-misses caught before landing, worth knowing about:**
1. **Almost broke lobby connectivity entirely.** `backend/src/signing/p2p_relay/`
   was deleted in an early pass of this work, on the assumption it only
   backed move-relay. It doesn't — `p2p_send_message`/`p2p_poll_messages`
   are *also* the lobby-level JOIN_ACK handshake used pervasively across
   `src/multiplayer/network/p2p_vps.rs` and `src/states/main_menu/screens.rs`
   (10+ call sites) for basic game connection setup, unrelated to moves.
   Caught via `grep` before shipping, restored from git (`git checkout --`,
   nothing was lost), left fully intact. **Do not remove `p2p_relay` again
   without migrating those call sites first** — it is not dead code, only
   its move-relay *usage* was superseded.
2. **`SessionInfo`/`Ping`-`Pong` almost lost their dual-transport fallback
   for real, previously-diagnosed bugs.** Comments already in the codebase
   documented that gossip-only `SessionInfo` was a reproduced failure
   ("Opponent pubkey unavailable" forever, game never settles) and
   gossip-only heartbeat falsely declares a "relay-only" game disconnected.
   Rather than re-adding the old relay fallback (out of scope, being torn
   out) or leaving these regressed, both got small, targeted fixes:
   `SessionInfo` is now a `ChessMessage::SessionInfo` variant carried on the
   moves stream (small, additive shared-crate change — reuses the
   already-subscribed moves stream rather than a new `ChessStream`
   variant); heartbeat timeout in `tick_heartbeat` (`systems.rs`) now also
   checks `BraidTransportState::is_connected()` and doesn't fire if the
   Braid subscription is confirmed alive, even if gossip Ping/Pong isn't
   arriving.

**Known scope cuts (deliberate, not oversights):**
- `OfferDraw`/`AcceptDraw`/`DeclineDraw` remain gossip-only — `ChessMessage`
  has variants for them but `braid_transport.rs` doesn't publish/consume
  them yet. Same shape of fix as moves/resign if this becomes a real gap.
- Real SQLite persistence covers `move`/`resign`/`offer_draw`/`accept_draw`/
  `decline_draw`/`chat`/`session_info` kinds. `Clock`/`EngineAnalysis`
  aren't part of this migration at all (separate `ChessResource` streams,
  untouched).
- Casual (non-wagered) games have no Solana wallet — `game_log.rs`'s
  `auth_ok` only requires a matching session token when `player_pubkey`
  parses as a real Solana pubkey; anything else (e.g. an Iroh node-id
  string) passes through, matching the old relay's identity-trust model
  exactly (same fail-open-when-not-applicable shape as
  `require_relay_or_jwt`, `infrastructure/auth_middleware.rs`). This is not
  a new hole — it's the same posture the code had before, just now with
  *real* auth for the wagered path, which had none.

**Verification status:** `cargo build --lib` / `--features solana`, and
`cargo test --lib` (225 client) / `cargo test -p backend --lib` (165
backend) all pass. **Not done:** a live two-client run exercising the
actual reconnect/catch-up path, and a Bevy-`App`-level integration test of
the cross-transport `applied_versions` dedup specifically (the unit tests
cover the gossip-only reorder logic and the backend's causal-chain
validation in isolation, but not the two paths racing end-to-end). Do this
before shipping to production — this session found real integration bugs
purely by tracing the actual wire format that unit tests alone didn't
catch; there's no reason to assume it's the last one.

## Identity-trust hardening: JOIN_ACK forgery + roster race (shipped 2026-08-02/03, third + fourth follow-on)

Asked "what stops someone stealing a node id" against everything shipped so
far. Surfaced three related gaps, all the same root cause: identity claims
trusted on assertion, not proof. Phase A + B shipped first (JOIN_ACK forgery
and the roster race for wagered games, where money is at stake); Phase C + D
were written up as backlog, then executed in a same-session follow-up —
all four now shipped.

**Phase A — JOIN_ACK is now signed.** `backend/src/signing/p2p_relay/routes.rs`'s
`send_message` (the lobby JOIN_ACK/GAME_START channel — separate from moves,
see the Braid migration section above) routed purely on a self-asserted
`from_node_id` string. Added `signature: Vec<u8>` to `SendMessageRequest`;
client signs `"{game_id}:{from_node_id}:{message}"` with the raw Iroh secret
key (`OnlineNetworkState::secret_key_bytes`, already in scope unused at
every one of the 6 call sites across `p2p_vps.rs`/`screens.rs`) via
`solana_sdk::signature::Signature` — no new crypto dependency, Iroh node_ids
and Solana pubkeys are both raw Ed25519 keys in the same base58 format, and
`Signature::verify` is already the established pattern for wallet-sig auth
elsewhere in this backend. Backend verifies before routing. Regression
tests: genuine signature accepted, forged claimed-identity rejected,
tampered-after-signing rejected, malformed bytes/pubkey don't panic.

**Phase B — wagered-game roster now checks on-chain truth, closing the
race.** `CausalChainState.roster` (gossip) and `GameLogState.roster` (Braid,
Finding 4's fix) were both "trust whoever's `SessionInfo` arrives first" —
an attacker racing a forged claim ahead of a real player's would win a
permanent trusted slot. New `backend/src/signing/solana/game_participants.rs`:
`GameParticipantsCache` reads a `Game` account's `white`/`black` directly
(reusing `settlement_worker::parse_game_account`'s pinned Borsh offsets,
minimal single-purpose parse) via the shared `AppState.solana_rpc`, cached
5 minutes (participants never change mid-match). `game_log.rs`'s
`GameLogState.put_event` now checks this **before** trusting a `SessionInfo`
claim into the roster (`learn_session_info_claim`) or authorizing a gameplay
action (`check_participant`) — a claim that doesn't match on-chain
`white`/`black` is discarded outright, not merely out-raced. Casual
(no-wallet) games fall through to the original first-seen bootstrap
unchanged, since there's no `Game` account to check against — same
documented, lower-severity, no-money-at-stake gap as before, now isolated
to exactly that case instead of covering wagered games too.

Regression test (`on_chain_verified_roster_beats_a_forged_first_claim`):
mallory posts a forged `SessionInfo` *first* — the post itself always
succeeds (posting was never gated), but her subsequent move is rejected
because the claim was never trusted; alice, arriving second, is correctly
recognized. This is the actual proof the race is closed, not just that the
check exists — caught a real bug in the test itself (first draft asserted
the `SessionInfo` PUT would fail, which was never the gate) before landing.

**Phase C — same on-chain check on the P2P/gossip client side.** New
`GET /game/{game_id}/participants` (`game_log.rs`) exposes Phase B's same
`GameParticipantsCache` lookup as `{white, black}` base58 strings (404 for a
casual game). New client-side `vps_client::fetch_verified_participants`,
spawned once per `GameStartedEvent` via a tokio-task-and-poll pair
(`spawn_verified_participants_fetch`/`poll_verified_participants_fetch`,
`multiplayer::solana::integration`, mirroring the existing profile-check
task pattern) into `CausalChainState.verified_wallets: HashMap<u64,
(String, String)>`. Discovered mid-implementation that the original framing
("seed the roster with wallet+session_key pairs") didn't quite hold: the
P2P roster (`CausalChainState.roster`) is keyed on `signing_pubkey`, a
fresh ephemeral gossip key generated per connection — it has no on-chain
record to check directly. The applicable fix instead gates roster admission
on the *accompanying* `SessionInfo.player_pubkey` (the wallet) matching
`verified_wallets` before its `signing_pubkey` is trusted in — same
"forged claim can't win the arrival-order race" property Phase B proved
server-side, now proved client-side by
`forged_player_pubkey_is_not_seated_on_the_roster` /
`genuine_player_pubkey_is_seated_on_the_roster` (`systems.rs`).

**Phase D — casual-game identity via the now-hardened JOIN_ACK.**
`accept_join` (`p2p_relay/routes.rs`) now calls new
`GameLogState::register_casual_identities(game_id, host_node_id,
joiner_node_id)` right after transitioning a lobby to `InProgress` — copying
the JOIN_ACK-verified (Phase A: cryptographically signed) pair into a small
non-evicting map on `GameLogState` itself, since `ActiveGame` stops getting
any relay traffic once Braid carries moves/chat and its 90s TTL sweep would
otherwise evict it mid-match. `game_log.rs`'s participant check gained a
`verify_claim` chain: on-chain check first (Phase B, wagered), then this
casual-identity check (Phase D) if the former found no `Game` account —
closing "anyone can PUT anything" for casual games that went through a real
JOIN_ACK handshake. A direct-connection game (never announced to the lobby,
so never calls `accept_join`) has no entry either way and keeps the
original trust-first-seen fallback unchanged — same documented,
lower-severity gap, now narrower still. Regression tests:
`casual_identity_check_beats_a_forged_first_claim`,
`no_casual_identity_entry_falls_back_to_trust_first`.

**Verification:** `cargo test -p backend --lib` (180 passed, up from 173),
`cargo build --lib --features solana` clean — both confirmed same session.

## Backlog (not built yet — prioritized)

### P1 — Gossip/relay trust hardening (DoS/griefing, not funds-critical but real)
- Roster slot assignment races on whichever `SessionInfo` arrives first
  (`systems.rs`, `CausalChainState.roster`) — a malicious peer could race to
  occupy a roster slot. **Fixed**, both sides: Braid/backend via Phase B's
  on-chain `white`/`black` check, P2P/gossip via Phase C's client-side
  `verified_wallets` fetch (see "Identity-trust hardening" above).
- Backend relay (`backend/src/signing/p2p_relay/routes.rs`) authenticates
  `send_message`/`poll_messages` only by a self-asserted `node_id` string
  matching what `announce`/`join` recorded — no wallet signature or session
  token. **Partially superseded**: `send_message` itself now requires a
  matching Ed25519 signature (Phase A); moves/resign/chat/session-handshake
  no longer flow through this endpoint at all (they're on `game_log.rs`'s
  real session-token + causal-chain-validated path now). `poll_messages`
  remains read-only and unsigned — lower severity, unchanged.
- Zero rate-limiting on any P2P/presence endpoint (`/p2p/announce`,
  `/p2p/join`, `/p2p/message`, `/p2p/poll`, `/p2p/heartbeat`, `PUT /presence`)
  — add `tower_governor` or an equivalent token bucket.
- CORS defaults permissive when `ALLOWED_ORIGINS` is unset
  (`backend/src/infrastructure/router.rs`) — tighten the default for prod.

### P2 — Backend relay resilience & observability
- Relay/presence state is in-memory only and silently dropped on backend
  restart (`backend/src/signing/p2p_relay/state.rs`,
  `backend/src/signing/social/presence.rs`) — no signal is pushed to clients
  when this happens, unlike the SQLite-backed matchmaking queue which already
  has a `hydrate()` restart path. **Superseded for moves/resign/chat** —
  `game_log.rs` now persists those to SQLite and survives a restart. Still
  fully open for `p2p_relay`'s own lobby/JOIN_ACK state and for presence.
- `PresenceStore::set_offline` (`backend/src/signing/social/presence.rs`) is
  defined but never called anywhere — presence only goes stale via TTL sweep,
  never actively flipped on disconnect.
- Single process-wide `RwLock<HashMap<String, ActiveGame>>`
  (`backend/src/signing/p2p_relay/state.rs`) serializes every game's
  announce/join/message/poll/heartbeat on one lock, and per-game message
  `Vec<String>` is never truncated (only evicted wholesale on TTL/finish) —
  shard by game_id or bound the vector.

### P3 — Cleanup
- ~~Delete `BraidPatch::from_message`~~ **Resolved differently**: instead of
  deleting the truncated-hash path, `version_hash` was widened to full
  32-byte SHA-256 and made load-bearing (the Braid-first-class migration's
  causal-chain key) — see that section above.
- `NetworkMessage::ResyncResponse.committed_turn` is hardcoded to `0` in the
  responder (`handle_resync_request`, `systems.rs`) — currently harmless
  because `handle_resync_response` only reads `committed_fen`, but it's a
  landmine for whoever adds a turn-number consumer later. Populate it from
  the engine's actual current turn while touching this code again.

### P4 — Wire on-chain settlement to the disconnect UI
Decision recorded: when the local client-side disconnect timeout elapses
(matching the on-chain 3×-base-time rule in
`programs/xfchess-game/src/lifecycle/clock.rs`), the client should
**automatically submit `ClaimTimeout`** on behalf of the still-connected
player rather than requiring a manual click — `ClaimTimeout` is already
permissionless by design (`game_ix/timeout.rs`), so this just automates what
any caller could already do, closing the loop opponent-disconnect →
on-chain resolution instead of leaving abandoned games unresolved until
someone manually cranks the instruction.
- No `emit!` events exist for `ClaimTimeout`, dispute open/resolve/stale-claim,
  resign, or finalize (only `MoveEvent`/`TreasuryWithdrawn` do,
  `programs/xfchess-game/src/events.rs`) — the UI can currently only detect
  these via account polling (`onAccountChange`), not event subscription. Add
  events alongside the auto-crank wiring so the UI gets a low-latency push
  instead of a poll.
- `HeartbeatState` (drives chess-clock flag + local `GameOverState`) and
  `P2PConnectionState` (drives the UI banners) are two independent
  disconnect-tracking systems that can theoretically disagree — audit for a
  race where one reports connected and the other reports timed out before
  wiring the auto-crank trigger to one of them.
- No reconnect-on-app-relaunch flow exists at all today (confirmed via
  search — nothing in `core/state_lifecycle.rs`/`core/plugin.rs`). A player
  who closes and reopens the app mid-game has no session-resume path; this
  is the other disconnect scenario explicitly in scope and still fully open.

### P5 — ER-specific (documentation only, no clear code fix)
`recover_stuck_delegation` (`programs/xfchess-game/src/governance_ix/`)
relies on the `dispute_authority` attesting player identities off-chain,
unverifiable on-chain, after a force-undelegate wipes the game PDA to zero
bytes (the delegation program's own design, not this program's choice). This
is a known, accepted trusted-authority dependency, not a P2P/client-signed
one — flagged for awareness. The self-serve ER-unavailability recovery path
(3 instructions + admin route + worker wiring) was already implemented
2026-07-27 in response to the MagicBlock undelegation advisory, but was not
yet devnet-deployed as of that work.

## Verification for the P0 change

```
cargo build --lib                          # clean, no new warnings
cargo test --lib multiplayer::             # 28/28 pass, incl. 500-trial fuzz test
cargo fmt -- src/multiplayer/systems.rs src/multiplayer/types.rs \
  src/multiplayer/mod.rs src/multiplayer/network/mod.rs src/multiplayer/network/reorder.rs
```

Not yet done: a live two-client devnet run exercising an actual gossip/relay
race (kill one transport mid-game, confirm the other recovers the board via
the new resync path). Recommended before this ships to production, since the
fuzz test proves the *sequencing logic* is correct but not the real
transport timing/loss characteristics.
