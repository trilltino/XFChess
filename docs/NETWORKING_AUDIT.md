# XFChess Networking Audit

_Date: 2026-06-15. Scope: every networking surface in the game client (`src/multiplayer/`)
and backend (`backend/src/`), plus the `braid-*` / `iroh-*` crates. Read-only audit — no
behavioural changes made. Items marked **⚠ verify** were inferred from code/comments and
should be runtime-confirmed before acting._

## TL;DR

The **live gameplay layer is genuinely strong** — Ed25519-signed gossip with identity
binding, replay protection, and causal equivocation checks (TLA+-backed). The weak spots are
all on the **HTTP/backend edge**: permissive CORS, several fully-unauthenticated endpoints
(including presence/friends), and auth middleware that **fails open** when secrets are unset —
which, per the auth-hardening notes, is the *current live state*. None of it holds private
keys, but the surface is wider than it should be.

---

## 1. Transports inventory

| # | Transport | Carries | Auth | Server needed? |
|---|-----------|---------|------|----------------|
| 1 | **Iroh QUIC gossip** (`iroh-gossip`) | Live moves, clock, chat, resign, draw, snapshots | Ed25519 signed (`SignedNetworkMessage`) | No — direct P2P |
| 2 | **HTTP relay fallback** (`p2p_relay` / client `p2p_vps`) | Lobby discovery + opaque relayed messages (poll-based) when hole-punch fails | **None** (payloads still signed end-to-end) | Yes |
| 3 | **Braid-HTTP / HTTP-209** (`xfchess-braid-server`, `ChessSubscriber`) | Browser-facing live state: tournament standings/pairings, chat | None (read streams) | Yes |
| 4 | **VPS REST** (Axum / reqwest blocking) | Session-key signing, identity/KYC, matchmaking, ratings, tournaments, presence, friends, rates, wallet balance | Mixed — see §3 | Yes |
| 5 | **WebSocket** (`/ws/auth`) | Auth/login sync | JWT-gated handshake; **stub payload loop** | Yes |
| 6 | **Solana RPC + Ephemeral Rollups** | On-chain game state, wager settlement, finality | On-chain (program authority / session keys) | RPC yes; trustless |

**Key architectural fact:** live PvP traffic was deliberately migrated *off* Braid-HTTP onto
Iroh gossip (see [braid_pvp.rs](../src/multiplayer/network/braid_pvp.rs#L1-L6)), keeping only
Braid's *versioning model* (`version_hash` + `parent_version`) for catch-up. Braid-HTTP now
lives only in the browser-facing corners (#3). This is what makes gameplay server-optional.

---

## 2. The message layer

[protocol.rs](../src/multiplayer/network/protocol.rs) defines `NetworkMessage` (~30 variants)
and the `SignedNetworkMessage` wrapper.

**Wire format** ([systems.rs:113](../src/multiplayer/systems.rs#L113)): a 1-byte version prefix —
- `0x02` = bincode `SignedNetworkMessage` (secure path)
- `0x01` = JSON `NetworkMessage` (legacy plain)

**Receive path** ([systems.rs:227-303](../src/multiplayer/systems.rs#L227)): verifies the
signature; unsigned messages are **dropped unless `--features allow-unsigned-p2p`** (dev only).

**Variant families** (worth a cleanup pass to confirm all are live):
- Core: `Move`, `Resign`, `DrawOffer/Response`, `FlagTimeout`, `GameStart`, `Clock`, `Chat`
- Liveness: `Ping/Pong`, `Rematch*`, `Pause/Resume`
- Catch-up: `GameSnapshot`, `BraidResyncRequest/Response`, **and** older `ResyncRequest/Response` — **⚠ two resync mechanisms coexist**; confirm the old pair isn't dead.
- ER co-signing: `SessionInfo`, `BatchPropose/Accept/Reject`, `TxMessage`, `TxSignature`, `Committed`, `BatchConfirmation` — only meaningful on the `solana`/rollup path; **⚠ verify still wired**.

---

## 3. Security findings

### 🔴 High

1. **CORS is `CorsLayer::permissive()` on the entire router** ([router.rs:108](../backend/src/infrastructure/router.rs#L108)) — any origin may call any endpoint. Auth is Bearer-header (not cookie) so this isn't classic CSRF, but it means any website can drive the open endpoints from a victim's browser. Lock to known origins (game client, web-solana, tournament-admin).

2. **Presence & social endpoints are fully unauthenticated** ([social/routes.rs](../backend/src/signing/social/routes.rs)). `PUT /presence` trusts the body's `node_id`/`display_name` verbatim → **anyone can inflate or spoof the online count** (this directly affects the indicator just added to the menu), post friend requests as any node, and enumerate `/social/poll`. At minimum: rate-limit, and bind the heartbeat's `node_id` to the authenticated identity.

3. **Auth fails open when secrets are unset.** `require_relay_or_jwt` / `require_relay_secret` ([auth_middleware.rs:125,78](../backend/src/infrastructure/auth_middleware.rs#L125)) pass the request through with a one-time warning when neither a JWT nor `RELAY_SHARED_SECRET` is present. The auth-hardening notes record that **`RELAY_SHARED_SECRET` is not set in live infra** → the session-key signing endpoints currently rely on the network firewall alone. Decide: fail-closed in release builds, or confirm the firewall is real and documented.

4. **Committed secrets in git history** (tracked separately in the secret-exposure notes) — out of scope to fix here, but it dominates the risk picture; rotate before mainnet.

5. **Program ID inconsistency** across `declare_id` / backend `config.rs` default / `.env.example` (auth-hardening notes). A backend pointed at the wrong program ID silently talks to the wrong/again-nonexistent program. Pin one source of truth.

### 🟡 Medium

6. **`/ws/auth` post-auth loop returns hardcoded stubs** ([auth_ws.rs:85-89](../backend/src/signing/auth_ws.rs#L85)) — `"token": "updated_token"`, `"wallet_pubkey": "updated_pubkey"`. The JWT handshake gate is real, but the actual sync payload is placeholder. Finish it or remove the route (dead/incomplete surface).

7. **HTTP relay is unauthenticated** ([p2p_relay/routes.rs](../backend/src/signing/p2p_relay/routes.rs)). Lobby announcements can be spoofed and the relay can drop/reorder/inject — *injection is defeated by end-to-end signatures*, but enumeration, lobby spam, and the `host_node_id` privacy model (hidden until join) all assume an honest relay. Add lightweight auth or rate-limiting.

8. **`ADMIN_API_KEY` defaults to `"dev"` in debug builds** ([auth_middleware.rs:35](../backend/src/infrastructure/auth_middleware.rs#L35)). Correctly returns 503 in release if unset — just ensure prod always sets it.

9. **JWT default TTL is 7 days** (now `JWT_TTL_SECS`-configurable; revocation table exists). Consider shortening now that a kill-switch is in place.

### 🟢 Good (keep)

- **Gossip move security is excellent**: `bind_identity` overwrites client-supplied `agent_id` with the *verified* signer ([systems.rs:193](../src/multiplayer/systems.rs#L193)); nonce replay protection ([systems.rs:357](../src/multiplayer/systems.rs#L357)); causal seq-gap + `parent_version` equivocation check; participant **roster** built from `SessionInfo` so strangers can't inject into a game they're not in. This closes the impersonation gaps the TLA+ model assumed away.
- **On-chain validation** re-applies each move and requires the client board to equal the engine result (auth-hardening notes).
- Constant-time secret comparison; bcrypt-hashed room passwords; `verify_strict` Ed25519.

---

## 4. Reliability

- **HTTP client timeout is 120s** ([client.rs:69](../src/multiplayer/network/vps/client.rs#L69)) — very long for interactive calls. Calls run on `IoTaskPool`, so the frame isn't blocked, but a hung backend ties up tasks for 2 min. Consider per-call timeouts (e.g. 5–10s for presence/lobby polls, longer only for tx submission).
- **TTLs / sweep cadence**: presence 5 min (no scheduled sweep — `sweep_stale` exists, uncalled); relay cleanup every 30s, 5-min stale; friends/presence client poll 15s; settlement worker 30s. Reasonable and consistent.
- **Reconnection / catch-up**: strong — `GameSnapshot` broadcast on `NeighborUp`, `BraidResyncRequest/Response` for gap recovery keyed on `version_hash`. **⚠ verify** the `/ws/auth` client has reconnect/backoff (not inspected).
- **Error handling**: client VPS helpers uniformly return `Result<_, String>` and callers degrade gracefully (e.g. presence count `.unwrap_or(0)`), so backend-down doesn't crash gameplay. Good for the server-optional goal.

---

## 5. Staleness & inconsistencies

1. **Two chat paths contradict each other.** [braid_pvp.rs:1-6](../src/multiplayer/network/braid_pvp.rs#L1) says chat is gossip-*exclusive* (`NetworkMessage::Chat`); [social.rs:113](../src/multiplayer/social.rs#L113) `LobbyChatSession` says chat uses the Braid-HTTP `/chat/:game_id` resource. Likely lobby-chat (HTTP) vs in-game-chat (gossip) — but at least one header comment is stale. **⚠ trace which is live.**
2. **`network/braid.rs` vs `network/braid_pvp.rs`** — with live traffic on gossip, `braid.rs` may be partly dead. **⚠ verify.**
3. **Duplicate resync mechanisms** (`Resync*` vs `BraidResync*`) — see §2.
4. **`/ws/auth` stub payload** — see §3.6.

---

## 6. Server-optionality matrix

| Capability | Works without backend? |
|---|---|
| Play vs computer / local | ✅ |
| Direct P2P game (after discovery) | ✅ gossip |
| Lobby discovery / matchmaking | ❌ relay |
| Relay fallback (NAT-blocked peers) | ❌ |
| Presence / friends / online count | ❌ |
| Ratings / identity / KYC | ❌ |
| Wagered (Solana) games | ❌ tx building + RPC |
| Tournaments | ❌ |

---

## 7. Recommended priority order

1. Lock down CORS to known origins (#1).
2. Authenticate/rate-limit presence + social + relay (#2, #7) — cheap, closes spoofing incl. the online count.
3. Decide fail-open vs fail-closed for signing endpoints in release; document the firewall assumption (#3).
4. Finish or delete `/ws/auth` stub (#6).
5. Reconcile the chat-path and resync duplications; drop dead `braid.rs` paths (§5).
6. Tighten per-call HTTP timeouts (§4).
7. (Out of band) secret rotation + program-ID single-source (#4, #5).
