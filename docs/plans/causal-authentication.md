# Causal-Layer Authentication, Multi-Party Model, and Replicated Persistence

> **STATUS — executed.** Gap A (A1–A4) and Gap B are implemented and shipped in
> the client; the multiplayer test suite passes (34/35; the one failure is an
> unrelated pre-existing MagicBlock rollup test). The fixes are tied to the
> formal model: `specs/CC_impersonation_current.cfg` shows the forged move lands
> without the fix, `specs/CC_impersonation_fixed.cfg` shows it is rejected with
> it. Gap C is verified in the model (`specs/SF_replicas.cfg`) and remains an
> operational deploy (run N replicas). What changed vs. the original plan: the
> signing key and the identity turned out to be **different keys**, so A1 was
> implemented as "receiver substitutes the verified signer for the claimed
> `agent_id`" (`bind_identity`) rather than "require `agent_id == session_pubkey`."
> See [Executed summary](#executed-summary) at the foot of this document.


Three gaps that sit *outside* the guarantee proven in [specs/](../../specs/). The
TLA+ model verified the causal chain is fork-free **under three assumptions it
could not itself discharge**. This document is how we discharge them in code.

The assumptions, stated explicitly in [specs/README.md](../../specs/README.md#scope-and-limitations):

1. A Byzantine peer acts *as itself* — it cannot forge another peer's `agent_id`.
   **(Gap A — the big one. Currently NOT enforced.)**
2. Exactly two participants, so the single `head_version` slot is sound.
   **(Gap B)**
3. Persistence is a given. **(Gap C — the VPS is a single point of failure.)**

Gap A is the priority. It is larger than the genesis-bypass bug already fixed,
because that bug let a *known* peer fork the chain, whereas this one lets an
*unknown* peer impersonate a known one — and the formal proof silently assumes
it away.

---

## Gap A — Authentication is integrity-only, not identity-bound

### What exists today

The send/receive path already signs messages
([src/multiplayer/systems.rs:116-128](../../src/multiplayer/network/../systems.rs#L116)
send, `:222-240` receive). `SignedNetworkMessage`
([protocol.rs:266-388](../../src/multiplayer/network/protocol.rs#L266)) carries an
Ed25519 signature and a `session_pubkey`, and `verify()` checks the signature.

This is genuinely good — it gives **tamper-evidence**. The two existing tests
(`tampered_message_rejected`, `tampered_signature_rejected`) confirm a mutated
move or signature is rejected.

### The gap

`verify()` only proves *"some valid signature exists for the key embedded in the
message."* It does **not** prove *"the right key signed it."* Three checks are
missing:

```rust
// protocol.rs verify() — what it does:
verifying_key.verify_strict(&signable, &signature).is_ok()
//   ^ signature matches self.session_pubkey  ✅ integrity
//   ✗ self.session_pubkey == msg.agent_id ?  -- NOT checked
//   ✗ self.session_pubkey is white/black of this game ?  -- NOT checked
```

The causal fork-check in
[systems.rs:341-398](../../src/multiplayer/systems.rs#L341) keys on
`msg.agent_id`. Nothing binds `agent_id` to the verified `session_pubkey`.

**The attack:**
1. Attacker generates a fresh keypair `K_evil`.
2. Builds `Move { agent_id = victim_node_id, seq, parent_version, ... }`.
3. Signs with `K_evil` → `session_pubkey = K_evil.public()`, signature valid.
4. Receiver runs `verify()` → **true** (signature is valid for `K_evil`).
5. Causal check trusts `agent_id = victim_node_id`. The attacker has injected a
   move *as the victim*, advancing/forking `last_seq` and `head_version` for the
   victim's identity.

Signing made the channel tamper-proof but left identity unauthenticated. This is
precisely the `Equivocate(p)` action in `CausalChain.tla` being restricted to
`sender |-> p`: the model assumes you can only equivocate as yourself. Today's
code does not enforce that.

### The fix

Three changes, smallest first.

**A1 — Bind `agent_id` to the signing key.**
Make the verified `session_pubkey` *be* the causal identity. Stop reading a
separate `agent_id` field from the message body; derive it from the verified
key. In `systems.rs` receive path, after `signed.verify()` succeeds:

```rust
// The causal identity IS the key that signed the message. An attacker
// cannot set agent_id independently of the key they actually hold.
if signed.msg_agent_id() != signed.session_pubkey {
    reject("agent_id does not match signing key");
}
```

Or better, drop the wire `agent_id` field entirely and have the receiver pass
`signed.session_pubkey` as the causal key into the fork-check. One source of
truth, unforgeable by construction.

**A2 — Roster check: the key must belong to a player in this game.**
A valid signature from *some* key is not enough; it must be white's or black's
registered session key. At game start the client already knows both players'
session pubkeys (from `create_game` / `join_game`, or the on-chain
`session_delegation` PDA). Capture them into a per-game roster and check:

```rust
let roster = causal.roster.get(&game_id);  // {white_session_pk, black_session_pk}
if !roster.contains(&signed.session_pubkey) {
    reject("signer is not a participant in this game");
}
```

This is the same `session_delegation.session_key` the Solana program already
enforces in
[record.rs:26](../../programs/xfchess-game/src/moves_ix/record.rs#L26)
(`constraint = session_delegation.session_key == player.key()`). We are bringing
the P2P layer up to the same standard the chain already holds.

**A3 — Disable the 0x01 plaintext fallback in production.**
The receive path still accepts `0x01` JSON-encoded *unsigned* `NetworkMessage`
([systems.rs:240-242](../../src/multiplayer/systems.rs#L240)). An attacker simply
sends `0x01` and skips authentication entirely. Gate it:

```rust
match body[0] {
    0x02 => { /* signed path */ }
    0x01 if cfg!(feature = "allow-unsigned-p2p") => { /* dev only */ }
    _ => reject("unsigned messages are not accepted"),
}
```

Default build rejects unsigned. Keep `allow-unsigned-p2p` for local testing
only.

**A4 — Fix and extend the unit tests.**
The test module ([protocol.rs:288-324](../../src/multiplayer/network/protocol.rs#L288))
constructs `NetworkMessage::Move { game_id, turn, move_uci, next_fen, nonce,
timestamp_ms }` — **without** the `agent_id`, `seq`, `parent_version` fields the
struct now requires. These tests are stale and will not compile against the
current struct. Update them, then add the regression test that matters:

```rust
#[test]
fn impersonation_rejected() {
    // Victim's identity, signed by the attacker's key, must be rejected
    // once A1 binds agent_id to the signing key.
    let attacker = SigningKey::from_bytes(&[9u8; 32]);
    let victim_id = SigningKey::from_bytes(&[1u8; 32]).verifying_key().to_bytes();
    let msg = NetworkMessage::Move { agent_id: victim_id.to_vec(), /* ... */ };
    let signed = SignedNetworkMessage::sign(msg, &attacker.to_bytes());
    assert!(signed.verify());                  // signature is valid for attacker...
    assert!(!accept_with_identity_binding(&signed)); // ...but identity check rejects it
}
```

### Verification (extend the formal model)

Update `CausalChain.tla` to *remove* the no-impersonation assumption and prove
the fix restores safety:

- Add a constant `AuthBinding` (BOOLEAN).
- Let `Equivocate(p)` set `sender` to **any** agent when `~AuthBinding`
  (models the current unauthenticated wire `agent_id`).
- In `Receive`, when `AuthBinding`, require the message's claimed sender equals
  its signing identity (model the signature as an unforgeable `signer` field a
  Byzantine agent can only set to itself).
- Expect: `AuthBinding = FALSE` → `NoFork`/`SeqMonotonic` **violated** (the
  impersonation fork). `AuthBinding = TRUE` → holds. This makes the model's
  hidden assumption an explicit, checked switch — and ties the code fix to a
  TLC result, exactly as we did for the genesis bypass.

---

## Gap B — Multi-party / spectator head model

### The gap

`causal.head_version` is a single `HashMap<game_id, String>` — one head slot per
game ([systems.rs:370](../../src/multiplayer/systems.rs#L370),
`:391`). With exactly one remote sender (2-party PvP) this is sound: the slot
always reflects that one opponent's chain. With **three or more** writers on the
same topic — a future team/relay variant, or a spectator channel that also
accepts moves — the slot flip-flops between senders, and the parent check
(`parent_version == our_head`) compares against whichever move arrived last,
regardless of author. That spuriously rejects honest moves and could admit
crossed-up ones.

The 2-party assumption is **safe today** (spectators are read-only, per
`spectator.rs`), but it is undocumented in the code and silently relied upon.

### The fix

Make the head per-sender, matching how `last_seq` is already keyed by
`(game_id, agent_id)`:

```rust
// today:   head_version: HashMap<u64, String>            // one slot per game
// change:  head_version: HashMap<(u64, Vec<u8>), String> // one per (game, agent)
```

Then the parent check becomes "does this move extend *this sender's* head," and
each participant's chain is tracked independently. For a true multi-writer game
you additionally need merge/DAG semantics (which of several concurrent heads is
canonical) — out of scope until such a mode exists, but the per-sender head is
the prerequisite and is cheap to do now.

### Verification

Re-run `CausalChain.tla` with `Agents = {A, B, C}` and `MaxSeq` small. The
single-slot model will fork (demonstrating the latent hazard); the per-sender
model will hold. This converts "we assume 2 parties" from a code comment into a
checked boundary.

---

## Gap C — VPS persistence is a single point of failure

### The gap

Persistence is one SQLite file on one Hetzner box
([backend/src/infrastructure/database.rs](../../backend/src/infrastructure/database.rs)).
If that node dies, no moves are recorded until it returns. This is already
flagged as P0 in [production-hardening.md](production-hardening.md#gap-1--tournament-state-is-lost-on-vps-restart),
but that plan only covers *snapshot + backup*, not *availability*.

### The deeper fix — a replicated log with a defined consistency level

The earlier plan
([braid-gap-analysis.md](../braid-gap-analysis.md)) identified the architecture:
the system already has a natural linearization point — **Solana slot order**.
Use it.

Target consistency model, stated precisely:
- **Linearizable reads:** the authoritative move history is whatever Solana has
  finalized. Any client can reconstruct it from `MoveEvent` logs; this is the
  single, totally-ordered source of truth. Already true — `record.rs` emits
  `MoveEvent` per move.
- **At-least-once writes:** a move is durably recorded if it reaches *any one* of
  N replica VPS nodes, each of which independently submits `record_move`. The
  on-chain nonce check de-duplicates: only the first submission at nonce k+1
  lands; the rest fail harmlessly with `InvalidNonce`. This is exactly the
  `SF_normal` property already proven in `SolanaFinality.tla` — concurrent
  submitters cannot fork the chain.

Implementation:
1. Run ≥2 backend replicas behind the existing reverse proxy, each with its own
   SQLite (no shared DB needed — Solana is the shared state).
2. Each replica subscribes to the game's gossip topic and submits `record_move`
   independently. Nonce conflicts are expected and benign.
3. On read, reconstruct from Solana `MoveEvent` logs (authoritative) and use
   local SQLite only as a cache.
4. Snapshot + backup (production-hardening Gaps 1–2) still applies per replica.

The elegant part: **no consensus protocol is needed between the replicas**
because Solana already provides it. The replicas are stateless submitters racing
to a linearizable log. This is the "replicated log with defined consistency"
the research framing called for, and `SolanaFinality.tla` already proves its
safety.

### Verification

Extend `SolanaFinality.tla`: model `N` replica submitters (not just players) all
forwarding the same honest moves plus reordering. `ChainLinearizable` should
continue to hold — confirming N redundant writers never corrupt the log. This is
a small change to `SubmitCap`/author set in the existing spec.

---

## Priority and effort

| Priority | Gap | Effort | Verifiable by |
|---|---|---|---|
| **P0** | A1 — bind agent_id to signing key | 2 hours | `CausalChain.tla` `AuthBinding` switch |
| **P0** | A3 — reject unsigned 0x01 in prod | 1 hour | — |
| **P1** | A2 — roster check against session keys | 3 hours | matches on-chain `record.rs` constraint |
| **P1** | A4 — fix stale tests + impersonation test | 2 hours | `cargo test -p xfchess` |
| **P2** | B — per-sender head map | 2 hours | `CausalChain.tla` with 3 agents |
| **P2** | C — N replica submitters | 1 day | `SolanaFinality.tla` multi-writer |

A1 + A3 together close the impersonation hole and are half a day. They are the
highest-value security work remaining, and unlike most hardening they come with
a formal check that the fix is correct — the same loop that caught the genesis
bypass.

---

## Why this ordering

The genesis-bypass fix closed a hole an *authenticated* peer could exploit.
Gap A closes a hole an *unauthenticated* peer can exploit — strictly more
dangerous, and it is the assumption the existing proof rests on. Until A1/A2
land, the statement "the protocol is formally verified fork-free" carries an
asterisk: *assuming peers cannot impersonate each other.* Closing Gap A removes
the asterisk.

---

## Executed summary

What was implemented in this pass (all compiling; multiplayer auth tests pass):

| Item | Change | Where |
|---|---|---|
| **A1** | Receiver substitutes the verified signer for the claimed `agent_id` | `bind_identity` in `src/multiplayer/systems.rs` |
| **A2** | Per-game roster of allowed signer keys, populated from `SessionInfo`; non-participant moves rejected | `CausalChainState.roster` + causal block |
| **A3** | Unsigned (`0x01`) messages rejected by default; `allow-unsigned-p2p` feature for dev | `process_gossip_stream` + `Cargo.toml` |
| **A4** | Stale `protocol.rs` tests fixed; impersonation regression test added | `protocol.rs` tests, `systems.rs::auth_tests` |
| **Gap B** | `head_version` keyed per `(game, agent)` so one identity cannot poison another's lane | `CausalChainState.head_version` |
| **Model** | `AuthBinding` switch + forging `Adversary` + `OnlyAuthenticAccepted` invariant | `specs/CausalChain.tla` |
| **Gap C** | N redundant submitters verified non-forking | `specs/SF_replicas.cfg` |

Formal results (`specs/`, all reproduced):
- `CC_impersonation_current` (AuthBinding=FALSE) → `OnlyAuthenticAccepted` **violated** (the forged move lands).
- `CC_impersonation_fixed` (AuthBinding=TRUE) → **holds** (1.3M states, no forged move accepted).
- `SF_replicas` → `ChainLinearizable` holds with 4 concurrent submitters.

**Correction to the original A1 sketch:** the plan assumed `agent_id == session_pubkey`
(same key). They are different keys — `agent_id` is the iroh node id, the signing
key is the Solana session key. A1 was therefore implemented as *substitute the
verified signer*, not *compare the two fields*. The effect is the same (forged
identities cannot land) and arguably stronger (one source of truth).

**Residual (honest):** the roster's trust anchor is `SessionInfo`, which is sent
only after the VPS confirms a session. A fully trustless roster would read
white/black session keys from the on-chain game account directly. That is the
remaining hardening; it does not affect the genesis-bypass or
opponent-impersonation closures, which are complete.

**Gap C remains an operational deploy:** the safety is proven, but actually running
N replica submitters is infrastructure (run two backends, each submitting
`record_move`), not a client code change.
