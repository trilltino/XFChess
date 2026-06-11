# Closing the Three Gaps with Braid-Reborn + Braid-Org

This document explores how [trilltino/braid-reborn](https://github.com/trilltino/braid-reborn)
and the [braid-org](https://github.com/braid-org) specification ecosystem can address the three
architectural gaps in XFChess's current P2P networking stack.

---

## The Three Gaps

| # | Gap | Current exposure |
|---|---|---|
| 1 | No Byzantine fault tolerance — a malicious peer can lie about moves | P2P layer trusts gossip bytes; Solana catches cheating eventually but not immediately |
| 2 | VPS is a single point of failure for persistence | All game history and catch-up lives in one SQLite on one server |
| 3 | No formal verification of the consistency model | The nonce chain + resync protocol are correct by convention, not by proof |

---

## Gap 1 — Byzantine Fault Tolerance

### The problem today

Every `NetworkMessage::Move` arrives over iroh gossip as raw bytes. The receiving
peer checks the nonce, deserializes, and applies it to the board. A malicious client
can send any bytes it wants — it can replay moves, skip nonces, or broadcast two
different moves for the same turn to different peers (equivocation). Solana's on-chain
state will reject the bad commit, but by then the game UI has already rendered
the lie and the player's clock has ticked.

### What Braid-Reborn provides

`braid-iroh`'s `Update` envelope wraps every message in a content-addressed
`Version` string derived from the payload hash. The `Parents` field chains it
causally to the previous update. This gives you:

```
Version: ["sha256:9f3a..."]     ← hash of this update's body
Parents: ["sha256:7c2b..."]     ← must match the previously applied version
```

Any update whose `Parents` doesn't match the local head is immediately detectable
as either out-of-order or forked. A fork (two updates both claiming the same
parent) is detectable without trusting either party.

### What braid-org/diamond-types adds

Diamond Types (the CRDT at the heart of `braid-text`) uses `(agent_id, seq_number)`
tuples as causally unique identifiers — effectively a vector clock per peer. Each
operation is:

```
op_id  = (agent_id: PublicKey, seq: u64)
parents = [op_id, ...]   ← the frontier at the time this op was generated
body    = the move
```

Because `agent_id` is a **public key**, the signature on the operation proves who
sent it. You can't forge an operation without the private key, and you can't
replay an operation without the sequence number being wrong.

### How to apply this to XFChess

**Step 1 — Sign every gossip update with the session key**

This already exists partially (`SignedNetworkMessage` carries an Ed25519 signature
from the session key). What's missing is verifying the signature *before* applying
the move to the board — currently the verification happens in `handle_network_events`
but the nonce check has been the primary guard.

**Step 2 — Add a per-peer causal DAG using Diamond Types' `(agent_id, seq)` model**

Replace the flat nonce counter with a diamond-types-style causal graph entry per
move. Each `NetworkMessage::Move` carries:

```rust
pub struct CausalMove {
    pub agent_id: [u8; 32],   // session pubkey — proves identity
    pub seq: u64,              // monotonic per agent — proves ordering
    pub parents: Vec<String>,  // content-hash of previous move — proves chain
    pub body: NetworkMessage,  // the actual move
}
```

Any `CausalMove` where:
- `signature` doesn't verify against `agent_id` → **reject immediately** (forged)
- `seq` is not `last_seen_seq_for_agent + 1` → **reject or buffer** (replay/gap)
- `parents` doesn't include the current board head → **reject** (equivocation fork)

This is not full Byzantine fault tolerance (BFT consensus requires 2f+1 honest
nodes in a committee) but it gives you **cryptographic equivocation detection** —
a malicious peer can only cheat if they break Ed25519, which is computationally
infeasible.

**Step 3 — Use Solana as the external arbiter for detected forks**

When a fork is detected (two valid signatures claiming the same parent), neither
peer should advance their board. Instead both clients submit the conflicting updates
to the `dispute` instruction in `governance_ix/`. The on-chain program compares
FEN hashes and attributes the violation. This closes the loop: iroh detects the
equivocation fast, Solana settles it with finality.

### What braid-reborn doesn't give you

Braid assumes cooperative peers. It provides integrity (content hashing) and
ordering (version DAG) but not BFT consensus — there is no quorum voting. For
a two-player zero-sum game that's actually fine: you don't need 2f+1 honest nodes
because there are only two nodes and Solana is the external trust anchor.

---

## Gap 2 — VPS Single Point of Failure

### The problem today

`record_move` writes every move to one SQLite file on one Hetzner VPS. If that
machine goes down mid-game:
- Spectator catch-up breaks (the 2-second poll returns 503)
- `BraidResyncRequest` still works (it reads from the iroh node's in-memory store)
  but that store dies when the game client exits
- Tournament state (`tournament_store.rs`) is in-memory only — a backend restart
  loses all running rounds

### What braid-reborn provides: two-tier durable storage

`braid-blob` + `braid-core`'s storage model:

```
Write path:
  Update arrives → SHA-256 hash computed → stored in content-addressable blob store
                                        → metadata (version, parents, resource URL)
                                          recorded in SQLite

Read / catch-up path:
  GET /resource?since=sha256:abc → query SQLite for versions after abc
                                  → stream blob data from filesystem
```

This is exactly what XFChess's `BraidIrohNode.resources` HashMap does today, but
in memory only. Migrating to `braid-blob`'s two-tier store would make the iroh
node's resource history **durable across restarts**.

### Concrete migration for XFChess

**Replace the in-memory `resources` HashMap in `BraidIrohNode`:**

```rust
// today
resources: Arc<RwLock<HashMap<String, Vec<Update>>>>,

// with braid-blob
resources: Arc<BraidBlobStore>,  // SQLite + filesystem, survives restart
```

Every `node.put(url, update)` persists to disk. Every `get_updates_since(version)`
reads from SQLite instead of memory. The iroh node becomes a durable peer-hosted
replica.

**Eliminate the VPS as the sole persistence source:**

With durable iroh nodes, the game history exists on:
1. The host player's machine (iroh node's blob store)
2. The joining player's machine (iroh node's blob store — receives every gossip update)
3. The VPS (record_move REST — kept as the canonical Solana settlement record)

Two of these three are independent. The VPS can go down; players still have full
history on their own machines. Spectators can catch up from the proxy
(`localhost:8181`) against the host's durable store.

**Tournament state:**

`tournament_store.rs` is currently in-memory. Braid-reborn's `server` crate
models tournament/session metadata as Braid resources (versioned, subscribable).
Migrating tournament rounds to versioned Braid resources means every round
advancement is a durable `Update` rather than a volatile Rust struct. Any backend
restart can re-hydrate from the blob store.

### Replication without a central server

Because both players' iroh nodes store the same updates (gossip broadcasts to
all topic subscribers), the game log is already replicated to two machines for
free. The proxy bridge means a third replica is accessible to any web spectator
who pulls the full stream. This is a minimal distributed log with N=2 honest
replicas and no coordination overhead.

---

## Gap 3 — Formal Verification of the Consistency Model

### The problem today

The consistency invariants in XFChess are maintained by convention:

- "nonces must be sequential" — checked in `handle_network_events` by hand
- "board FEN must advance monotonically" — checked in `apply_braid_resync_to_spectator` informally
- "both peers converge to the same state" — assumed, never proven

If any system (resync, reconnect, clock migration) violates these invariants, the
game silently diverges.

### What braid-org provides: IETF specifications as a formal baseline

The braid-org/braid-spec repository contains four IETF Internet Drafts that
formally specify the protocol:

| Draft | What it specifies |
|---|---|
| `draft-toomim-httpbis-braid-http-04` | Core: versioning, patches, subscriptions, merge-types |
| `draft-toomim-httpbis-versions-05` | Version ID semantics, DAG model, causal ordering |
| `draft-toomim-httpbis-merge-types-00` | Deterministic merge: same inputs → same output, always |
| `draft-toomim-httpbis-range-patch-01` | Patch syntax (bytes, JSON, text units) |

These drafts define the **invariants** precisely:

```
Invariant (from versions-05, §4.2):
  For any two peers A and B:
  If A knows versions V_A and B knows versions V_B,
  and both have applied all updates reachable from V_A ∪ V_B,
  then A's resource state == B's resource state.
  
  This holds iff the merge-type function is deterministic and total.
```

That's an informal but precise convergence theorem. The draft proves it holds
for the simpleton and diamond-types merge-types.

### What diamond-types provides: fuzz-tested CRDT correctness

Diamond Types (`braid-org/diamond-types`) comes with an extensive fuzz testing
suite that tests the core invariant: given any sequence of concurrent operations
by any number of agents, all peers converge to the same final state. From the
internals doc:

> The fuzzer generates random histories with random agent orderings and verifies
> that every linearisation of the causal DAG produces the same merged result.

This is property-based testing of the convergence property — stronger than unit
tests but weaker than formal proof. For a chess game where the "document" is a
sequence of moves (each move is an insert into an append-only log), diamond-types'
append-only CRDT semantics apply directly.

### What XFChess needs to do to adopt the formal model

**Step 1 — State the invariants explicitly in code**

Add a `debug_assert!` in `handle_network_events` that checks the convergence
invariant after every applied move:

```rust
debug_assert_eq!(
    braid_uri::version_hash(&next_fen, turn as u32),
    expected_version,
    "Version chain broken: board state diverged from Braid DAG"
);
```

This is not formal proof but it's machine-checked at every step during development.

**Step 2 — Adopt the braid-spec version headers throughout**

Every `NetworkMessage::Move` already carries a content-addressed version via
`braid_uri::version_hash`. Fully adopting the versions-05 model means also
tracking `Parents` headers so the version DAG is explicitly represented,
not just implied by sequence numbers.

**Step 3 — Use diamond-types' causal graph as the authoritative move log**

Instead of a `Vec<MovePayload>` in the VPS and a flat `Vec<Update>` in the iroh
node, represent the move log as a diamond-types causal graph where each node is
a signed `CausalMove`. The graph structure makes concurrent forks structurally
impossible to hide — any two moves claiming the same parent are represented as
two branches in the DAG, immediately visible.

**Step 4 — The Solana program as a linear extension of the DAG**

The on-chain program stores FEN + move history as a flat array. This is a
linearisation of the causal DAG. For a two-player sequential game (chess) the
DAG should always be a chain — any branch in the DAG is by definition cheating.
Adding an on-chain assertion:

```rust
// In record_move instruction:
require!(
    game.head_version == move_payload.parent_version,
    XFChessError::VersionMismatch
);
```

ties the on-chain FEN to the Braid version DAG, making divergence impossible
to commit even if it were accepted by the P2P layer.

---

## Architecture After All Three Gaps Are Closed

```
Local move
  │
  ├── Sign with session key (Ed25519)
  ├── Attach CausalMove { agent_id, seq, parents: [current_head_hash] }
  └── Send via iroh gossip → NetworkMessage::Move
  
Peer receives move
  │
  ├── Verify Ed25519 signature → reject if invalid (Gap 1: no forgery)
  ├── Check seq == last_seen + 1 → buffer/reject if wrong (Gap 1: no replay)
  ├── Check parents ⊆ local DAG frontier → reject if fork (Gap 1: equivocation detected)
  └── Apply to board + advance local version head
  
Persistence (both nodes)
  │
  ├── braid-blob: persist Update to SQLite + filesystem (Gap 2: two durable replicas)
  └── record_move REST: persist to VPS (Gap 2: third canonical replica)
  
Consistency check (debug)
  │
  └── assert version_hash(fen, turn) == expected (Gap 3: machine-checked invariant)
  
On-chain settlement
  │
  └── require head_version == parent_version in record_move instruction (Gap 3: chain-enforced)
  
Fork detected (rare)
  │
  └── Submit both conflicting signed updates to dispute instruction → Solana resolves (Gap 1 + 3)
```

---

## Summary

| Gap | Mechanism | Crate/spec |
|---|---|---|
| BFT / equivocation | `(agent_id, seq, parents)` causal move + Ed25519 signature verification before board apply | `diamond-types` causal graph model + existing `SignedNetworkMessage` |
| Persistence SPOF | Migrate `BraidIrohNode.resources` from in-memory HashMap to `braid-blob` two-tier store | `braid-reborn/braid-blob` |
| Consistency model | Explicit version DAG with `Parents` headers + on-chain `head_version` assertion + fuzz-tested CRDT | `braid-spec/versions-05`, `diamond-types` fuzzer |

None of these require a new server or a new transport layer. Everything builds on
the iroh gossip + Braid versioning stack already in place. The changes are
additive: more fields in messages, a storage backend swap, and a tighter
invariant in the Solana program.
