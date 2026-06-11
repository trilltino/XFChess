# TLA+ Plan: Formally Verifying the XFChess Causal Chain

## Reading List — Do This First

Work through these in order. Each builds on the previous.

### 1. Understand what TLA+ is (1–2 hours)

**The Amazon paper — why this matters in practice**
https://www.amazon.science/publications/how-amazon-web-services-uses-formal-methods

Read this first. It's written for engineers, not academics. It explains exactly
why AWS uses TLA+ on real distributed systems and what kinds of bugs it finds.
This gives you the motivation before you touch any syntax.

**Lamport's own introduction — 15 minutes**
https://lamport.azurewebsites.net/tla/tla.html

One page. Read it. Lamport is the inventor and his framing of what TLA+ is for
is clearer than anything anyone else has written.

---

### 2. Learn the language (1–2 weeks)

**Primary resource: learntla.com by Hillel Wayne**
https://learntla.com

This is the best practical introduction that exists. Hillel Wayne is the author
of *Practical TLA+* (the book) and this site covers the same material online.
Work through it in this order:

1. **Introduction** — state machines, actions, invariants
2. **PlusCal** — the higher-level algorithmic language that compiles to TLA+.
   Start here rather than raw TLA+ syntax. PlusCal looks like pseudocode and
   is much easier to write. You translate a concurrent algorithm into PlusCal,
   the toolbox compiles it to TLA+, and TLC checks it.
3. **Temporal logic** — the `[]` (always) and `<>` (eventually) operators.
   These are how you write liveness properties (the system eventually makes
   progress, not just safety properties (the system never does something bad).
4. **Model values and symmetry** — how to shrink the state space so TLC can
   finish in reasonable time.

**Lamport's video course — 10 one-hour lectures**
https://lamport.azurewebsites.net/video/videos.html

Lamport teaches TLA+ himself. Lectures 1–6 are essential. Watch them after
reading learntla.com — Lamport's style is mathematical and the practical
foundation from learntla.com makes the lectures land better.

**The book — free PDF**
https://lamport.azurewebsites.net/tla/book.html

*Specifying Systems* by Lamport. The reference. You don't read this cover to
cover — you use it to look things up. Chapter 2 (The Simple Math of TLA) and
Chapter 14 (Advanced Examples) are the most useful once you're writing specs.

---

### 3. Install the tooling (30 minutes)

**TLA+ Toolbox (IDE + TLC model checker)**
https://github.com/tlaplus/tlaplus/releases

This is the standard IDE. It has a PlusCal translator built in, a TLC runner,
and a spec editor. Download the latest release for Windows.

**VS Code extension (alternative to Toolbox)**
https://marketplace.visualstudio.com/items?itemName=alygin.vscode-tlaplus

If you prefer VS Code over the dedicated IDE. Has syntax highlighting,
PlusCal translation, and TLC integration. This is what to use if you want
the spec to live alongside the Rust code in the repo.

**TLAPS — the TLA+ Proof System (optional, for proofs not just checking)**
https://tla.msr-inria.inria.fr/tlaps/content/Home.html

TLC does model checking (finite state exhaustive search). TLAPS does deductive
proof (proves the invariant holds for infinite state spaces). You don't need
this initially — start with TLC. Come back to TLAPS if you want publication-
grade proofs.

---

### 4. Read a real protocol spec (1–2 hours)

**Paxos in TLA+ — Lamport's own spec**
https://github.com/tlaplus/Examples/blob/master/specifications/Paxos/Paxos.tla

Don't try to understand every line on first read. Look at the structure: the
VARIABLES declaration, the Init predicate, the Next action, the Spec formula,
and the invariants at the bottom. This is the template you'll follow.

**Raft consensus in TLA+**
https://github.com/ongardie/raft.tla

Raft is simpler than Paxos and the spec is well commented. Read Diego Ongaro's
spec alongside the Raft paper to see how a real protocol maps to TLA+.

---

## What We Are Modelling

The XFChess causal chain has four components to specify:

### The state each peer maintains

```
head_version : STRING   -- version_hash of last accepted move
last_seq     : Nat      -- last accepted sequence number from opponent
```

### The messages in the network

```
Move {
  agent_id       : AgentId
  seq            : Nat
  parent_version : STRING
  next_fen       : STRING
  turn           : Nat
}
```

### The actions

| Action | Description |
|---|---|
| `LocalMove(peer)` | A peer generates a new move and adds it to the network buffer |
| `ReceiveMove(peer, msg)` | A peer receives a buffered move and either accepts or rejects it |
| `Equivocate(peer)` | A Byzantine peer sends two different moves with the same parent |
| `Drop(msg)` | A message is dropped (network partition simulation) |
| `Reconnect(peer)` | A peer rejoins after a partition |

### The invariants we want to prove

```
-- Safety: no two accepted moves share the same parent_version
NoFork == \A p1, p2 \in Peers :
  accepted[p1] # {} /\ accepted[p2] # {} =>
    \A m1 \in accepted[p1], m2 \in accepted[p2] :
      m1.parent # m2.parent => m1 = m2

-- Safety: seq numbers from a given agent are strictly increasing
SeqMonotonic == \A p \in Peers :
  \A i, j \in DOMAIN history[p] :
    i < j => history[p][i].seq < history[p][j].seq

-- Liveness: if no messages are dropped, both peers eventually agree
EventualConvergence ==
  (no_drops = TRUE) => <>(head[Alice] = head[Bob])
```

---

## Step-by-Step Implementation Plan

### Phase 0 — Foundation (before writing any TLA+)
*Estimated time: 2 weeks*

- [ ] Complete learntla.com through the PlusCal section
- [ ] Watch Lamport video lectures 1–6
- [ ] Install TLA+ Toolbox and run the die-hard example from learntla.com
- [ ] Read the Raft TLA+ spec alongside the Raft paper
- [ ] Write one throwaway spec of your own (model something simple: a
      two-process mutex, a producer-consumer queue) just to learn the tooling

---

### Phase 1 — Model the happy path
*Estimated time: 1 week*

Write the simplest possible spec that models two honest peers exchanging moves.
No Byzantine behaviour, no drops.

**File:** `specs/causal_chain.tla`

```
VARIABLES
  head,        -- [peer -> STRING]  current head version per peer
  last_seq,    -- [peer -> Nat]     last accepted seq per peer
  network,     -- set of in-flight messages
  history      -- [peer -> Seq(Move)] accepted moves in order

CONSTANTS
  Peers,       -- { Alice, Bob }
  MaxMoves     -- small bound (3 or 4) to keep state space finite
```

Actions to implement first:
1. `Init` — both peers at head "0", empty history
2. `LocalMove(p)` — peer p generates a move, increments seq,
   sets parent_version = current head, adds to network
3. `ReceiveMove(p, m)` — peer p receives m, checks seq and parent,
   advances head if valid
4. `Next == \E p \in Peers : LocalMove(p) \/ \E m \in network : ReceiveMove(p, m)`

Invariants to check at this stage:
- `TypeOK` — all variables have the right types
- `SeqMonotonic`
- `NoFork` (should trivially hold with no Byzantine behaviour)

Run TLC with `MaxMoves = 3`, `Peers = {Alice, Bob}`. It should find no
violations and finish in under a second.

---

### Phase 2 — Add Byzantine behaviour
*Estimated time: 1 week*

Add the `Equivocate` action: a Byzantine peer sends two different moves
both claiming the same `parent_version`.

```
Equivocate(p) ==
  /\ byzantine[p] = TRUE
  /\ \E m1, m2 \in PossibleMoves :
       /\ m1.parent = m2.parent
       /\ m1 # m2
       /\ network' = network \cup {m1, m2}
  /\ UNCHANGED << head, last_seq, history >>
```

Now run TLC. Without the causal verification in `ReceiveMove`, `NoFork`
will be violated — TLC will show you the exact execution sequence.

Then add the causal checks to `ReceiveMove`:
```
ReceiveMove(p, m) ==
  /\ m \in network
  /\ m.seq = last_seq[p] + 1          -- seq continuity check
  /\ m.parent = head[p]               -- parent matches our head
  /\ head' = [head EXCEPT ![p] = version_hash(m)]
  /\ last_seq' = [last_seq EXCEPT ![p] = m.seq]
  /\ history' = [history EXCEPT ![p] = Append(history[p], m)]
  /\ network' = network \ {m}
```

Run TLC again. `NoFork` should now hold even with equivocation.
TLC will have checked every possible interleaving and confirmed the
invariant holds in all of them. That is the proof.

---

### Phase 3 — Add network faults
*Estimated time: 1 week*

Add message drops and reconnection to model real network behaviour.

```
Drop(m) ==
  /\ m \in network
  /\ network' = network \ {m}
  /\ UNCHANGED << head, last_seq, history >>
```

Now check `EventualConvergence` with a fairness condition:
```
Spec == Init /\ [][Next]_vars /\ WF_vars(Next)
```

`WF_vars(Next)` (weak fairness) says: if an action is continuously
enabled, it eventually fires. This prevents TLC from exploring executions
where peers just never send anything. With fairness, `EventualConvergence`
should hold — TLC will verify that in all fair executions, the peers
eventually agree.

Without fairness, convergence doesn't hold (a peer can be dropped
forever). That's the expected result — document it.

---

### Phase 4 — Model the Solana finality layer
*Estimated time: 1–2 weeks*

Add a `Chain` component representing Solana:

```
VARIABLES
  chain_nonce,   -- Nat  (game.nonce on-chain)
  chain_fen,     -- STRING (committed board state)
  pending_tx     -- set of unconfirmed record_move transactions
```

Actions:
- `SubmitTx(p, move)` — peer submits record_move with parent_nonce
- `ConfirmTx(tx)` — Solana confirms transaction, advances chain_nonce
- `RejectTx(tx)` — Solana rejects (wrong parent_nonce or nonce conflict)

New invariant:
```
ChainLinearizable ==
  -- The chain's history is always a prefix of at least one peer's history
  \E p \in Peers :
    \A i \in 1..chain_nonce :
      chain_history[i] = history[p][i]
```

This is the formal statement that Solana is the linearizable read path:
whatever is on-chain is a prefix of a correct peer's history.

TLC will verify this holds across all combinations of tx ordering and
Byzantine peer behaviour.

---

### Phase 5 — TLAPS proof (publication-grade)
*Estimated time: 4–8 weeks, optional*

TLC checks finite models. TLAPS proves the invariants for infinite state
spaces (any number of moves, any number of reconnects). This is what
turns model checking into a mathematical proof.

Install TLAPS:
https://tla.msr-inria.inria.fr/tlaps/content/Download/Download.html

The proof structure for `NoFork`:
```
THEOREM NoFork
  ASSUME TypeOK,
         /\ \A p \in Peers : valid_causal_check(p)
  PROVE  \A m1, m2 \in accepted_moves :
           m1.parent = m2.parent => m1 = m2
BY DEF NoFork, ReceiveMove, Equivocate
```

TLAPS discharges each proof obligation to a backend solver (Isabelle or Z3).
If all obligations are discharged, the theorem is proven for all possible
executions, not just the ones TLC explored.

This is the step that makes it publishable. A TLC result says "we checked
all states up to depth N and found no violation." A TLAPS proof says
"it is impossible for a violation to exist."

---

## File Structure in the Repo

```
specs/
  causal_chain.tla        -- Main spec (Phases 1–3)
  causal_chain.cfg        -- TLC configuration (constants, invariants to check)
  solana_finality.tla     -- Phase 4 extension
  causal_chain_proof.tla  -- Phase 5 TLAPS proofs
  README.md               -- How to run TLC, what each spec models
```

The `.cfg` file tells TLC what to check:
```
INIT Init
NEXT Next
INVARIANT TypeOK
INVARIANT NoFork
INVARIANT SeqMonotonic
PROPERTY EventualConvergence
CONSTANTS
  Peers = {Alice, Bob}
  MaxMoves = 4
```

---

## Mapping From Rust Code to TLA+ Spec

| Rust | TLA+ equivalent |
|---|---|
| `CausalChainState.head_version` | `head[peer]` |
| `CausalChainState.last_seq` | `last_seq[peer]` |
| `NetworkMessage::Move { seq, parent_version, .. }` | `Move` record |
| `handle_network_events` causal check block | `ReceiveMove` action |
| `publish_local_move` | `LocalMove` action |
| `Equivocate` action (Byzantine) | no Rust equivalent — this is the attack |
| Solana `record_move` | `SubmitTx` + `ConfirmTx` actions |
| `game.nonce` | `chain_nonce` |

The TLA+ spec is not the Rust code. It is a model of the *protocol* the
Rust code implements. If TLC finds a violation in the spec, you fix the
protocol design first, then update the Rust code to match.

---

## What TLC Will Tell You That Code Review Cannot

1. **Exactly which execution leads to a fork** — TLC produces a full
   counterexample trace showing the exact sequence of actions (with peer
   IDs, message contents, state at each step) that violates the invariant.
   This is an invaluable debugging artifact.

2. **Whether liveness holds under weak fairness** — `EventualConvergence`
   requires all fair executions to converge. TLC checks this over all of
   them. A human reviewer can miss the case where drops and reconnects
   conspire to prevent convergence forever.

3. **Whether the Solana parent_nonce check is sufficient** — Phase 4 will
   tell you whether `require!(parent_nonce == game.nonce)` alone is enough
   to ensure ChainLinearizable, or whether there are edge cases where two
   valid-looking transactions with different parent_nonces both get accepted
   due to a race in the Solana mempool.

---

## Estimated Total Time

| Phase | Time | Deliverable |
|---|---|---|
| 0 — Learning | 2 weeks | Confidence with PlusCal + TLC |
| 1 — Happy path | 1 week | `causal_chain.tla` with TypeOK + SeqMonotonic |
| 2 — Byzantine | 1 week | NoFork proven by TLC |
| 3 — Network faults | 1 week | EventualConvergence proven under fairness |
| 4 — Solana layer | 1–2 weeks | ChainLinearizable proven by TLC |
| 5 — TLAPS proof | 4–8 weeks | Mathematical proof, publication-ready |

Phases 1–4 together are the engineering deliverable — a mechanically
checked spec that gives high confidence in the protocol. Phase 5 is the
research deliverable — a formal proof suitable for a workshop paper.

You can stop at Phase 4 and already have something most production
distributed systems do not have.
