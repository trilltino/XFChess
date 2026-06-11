# TLAPS Plan: An Unbounded Proof of NoFork

Phase 5 of [tla-causal-chain.md](tla-causal-chain.md), at parity depth with
[production-hardening.md](production-hardening.md) and
[causal-authentication.md](causal-authentication.md).

**What this buys:** TLC has checked `NoFork` exhaustively for *bounded*
instances — 2–3 moves, 2 agents (15.2M states for the fix). That is strong
evidence, not a proof. A peer could in principle craft a 4th move, or a 50th,
that forks. This plan removes the bound: a deductive proof that `NoFork` holds
for **any number of moves**, via the TLA+ Proof System (TLAPS).

**The one honest caveat up front:** of the three remaining asterisks, this is
the only one with real schedule risk. The replica-submitter and operational
work are standard engineering. A mechanical unbounded proof can stall for weeks
on a single missing lemma. This plan is structured specifically to *de-risk*
that — it inserts a symbolic-checking phase (Apalache) before committing to
TLAPS, so we validate the hard part (the inductive invariant) cheaply before
paying for the full proof.

---

## The core problem: `NoFork` is not inductive

TLAPS proves `[]Inv` by induction over steps:

```
1.  Init => Inv                      (base case)
2.  Inv /\ [Next]_vars => Inv'       (inductive step)
----
    Spec => []Inv
```

For step 2 to go through, `Inv` must contain *enough* state to imply its own
preservation. `NoFork` alone does not. Here is a concrete state that satisfies
`NoFork` but takes a step into a state that violates it:

```
accepted[B] = << m1 >>                  \* one accepted move, version = vA1
head[B]     = w   where w # vA1         \* head is INCONSISTENT with the log
NoFork holds  -- vacuously: there is no index i >= 2 to constrain.

Now Receive(B, m2) with m2.seq = 2, m2.parent = w (= head[B]).
  The guard `m.parent = head[B]` passes.
  accepted[B]' = << m1, m2 >>.
  NoFork' obligation at i = 2:  m2.parent = m1.version  =>  w = vA1.
  But w # vA1.   NoFork' is VIOLATED.
```

The state above is not actually reachable (the real protocol keeps `head` equal
to the last accepted version), but `NoFork` cannot *see* that. The proof needs
an invariant that rules it out. That invariant is the whole game.

---

## The inductive invariant

We strengthen `NoFork` with the facts the protocol actually maintains. Stated
against the real variables of [CausalChain.tla](../../specs/CausalChain.tla)
(`own_seq, last_version, net, recv_last_seq, head, accepted`):

```tla
\* (I1) Everything is well-typed.
TypeOK == ... (already in the spec)

\* (I2) Accepted moves carry real (non-genesis) versions whose author is the
\*      sender. This is what makes head # Genesis after the first accept.
RealVersions ==
  \A p \in Agents : \A i \in 1..Len(accepted[p]) :
     /\ accepted[p][i].version.agent = accepted[p][i].sender
     /\ accepted[p][i].version.agent \in Agents

\* (I3) THE KEY LEMMA. The receiver's head slot equals the version of the last
\*      move it accepted (or Genesis if it has accepted nothing). This is the
\*      fact NoFork-as-stated cannot see.
HeadMatchesLog ==
  \A p \in Agents :
     head[p] = IF accepted[p] = << >>
               THEN Genesis
               ELSE accepted[p][Len(accepted[p])].version

\* (I4) Per-sender sequence counter equals the number of moves accepted from
\*      that sender. In 2-party play, the single remote sender's count = Len.
RecvSeqMatchesLog ==
  \A p \in Agents : \A q \in Agents :
     recv_last_seq[p][q] = Cardinality({ i \in 1..Len(accepted[p]) :
                                           accepted[p][i].sender = q })

\* (I5) Sequence numbers in the accepted log are contiguous from 1 (per sender).
SeqContiguous ==
  \A p \in Agents : \A i \in 1..Len(accepted[p]) :
     accepted[p][i].seq = Cardinality({ j \in 1..i :
                              accepted[p][j].sender = accepted[p][i].sender })

\* The full inductive invariant.
Inv == TypeOK /\ RealVersions /\ HeadMatchesLog /\ RecvSeqMatchesLog
              /\ SeqContiguous /\ NoFork
```

Once `Inv` is proven invariant, `NoFork` follows immediately (`Inv => NoFork`).
The induction now closes: in the `Receive` case, `HeadMatchesLog` supplies
`head[p] = Last(accepted[p]).version`, the guard supplies
`m.parent = head[p]`, and together they give `m.parent = Last(accepted[p]).version`
— exactly the new top-index obligation of `NoFork'`.

> The 2-party scope (Gap B of causal-authentication.md) is *visible* here:
> `RecvSeqMatchesLog`/`SeqContiguous` are clean because each receiver has a
> single remote sender. Generalising to N>2 means proving per-sender, and is
> the multi-party model's job — out of scope for this proof, in scope for that
> one.

---

## Phase 0 — Reading (TLAPS-specific, ~1 week)

General TLA+ is assumed (the earlier plan). These are about *proving*, not model
checking:

- **TLAPS tutorial — the canonical starting point**
  https://tla.msr-inria.inria.fr/tlaps/content/Documentation/Tutorial.html
  Work the whole thing. It teaches the `BY`, `DEF`, `<1>`, `<2>` proof-step
  syntax and how obligations get dispatched to backends.

- **"Proving Safety Properties" — Lamport**
  https://lamport.azurewebsites.net/tla/proving-safety.pdf
  The definitive guide to exactly this kind of proof: inductive invariants for
  safety. Read it twice. The whole method of this document is from here.

- **The TLA+ Hyperbook, proof chapters**
  https://lamport.azurewebsites.net/tla/hyperbook.html
  Worked inductive-invariant proofs with TLAPS.

- **A real worked proof to imitate: Byzantine Paxos in TLAPS**
  https://github.com/tlaplus/tlaps-examples  (and `tlaplus/Examples`)
  The structure — `THEOREM Spec => []Inv`, case split on actions — is the
  template. Byzantine Paxos is the closest analogue to our adversarial setting.

- **SequencesExt / TLAPS standard library**
  https://github.com/tlaplus/CommunityModules
  Lemmas about `Append`, `Len`, `Last` that you will need and should not
  re-prove. `SequenceTheorems` is essential — sequence reasoning is the single
  biggest time sink in TLAPS, and these lemmas remove most of it.

---

## Phase 1 — De-risk with Apalache (symbolic, ~1 week)

Before writing a single TLAPS proof step, validate that `Inv` is *actually
inductive* using Apalache, the symbolic (SMT-backed) model checker. This is the
highest-leverage step in the plan.

- **Apalache** https://github.com/apalache-mc/apalache
- Apalache can check an *inductive invariant* directly:

```bash
# 1. Inv holds in all initial states:
apalache-mc check --init=Init --inv=Inv --length=0 CausalChain.tla

# 2. Inv is preserved by one step (the inductive step, symbolically, over the
#    full unbounded data domain for a fixed parameter set):
apalache-mc check --init=Inv --inv=Inv --length=1 CausalChain.tla
```

If step 2 passes, `Inv` is inductive: it holds after any single step from any
`Inv`-state — which by induction means it holds for **any number of steps**.
This is already a major strengthening over TLC's bounded-depth result, and it
costs days, not weeks. Crucially, when `Inv` is *not yet* inductive, Apalache
returns a concrete counterexample-to-induction (a CTI): an `Inv`-state with a
bad successor. Each CTI tells you exactly which conjunct is missing. You iterate
`Inv` against CTIs until step 2 is clean.

**This is where the real intellectual work happens** — and Apalache makes it a
fast feedback loop instead of a TLAPS guessing game. Do not skip it.

> Apalache still fixes the *constants* (`Agents`, `MaxSeq`). It proves unbounded
> *depth*, not unbounded *parameters*. Lifting the parameters is what TLAPS does
> in Phase 2. But arriving at TLAPS with an Apalache-validated `Inv` removes ~80%
> of the risk.

---

## Phase 2 — The TLAPS safety proof (~3–5 weeks)

Port the Apalache-validated `Inv` to a TLAPS deductive proof. Structure:

```tla
THEOREM Safety == Spec => []NoFork
<1>1. Init => Inv
  BY DEF Init, Inv, TypeOK, RealVersions, HeadMatchesLog,
         RecvSeqMatchesLog, SeqContiguous, NoFork
<1>2. Inv /\ [Next]_vars => Inv'
  <2>1. CASE LocalMove(p)        \* sender side: net grows, accepted unchanged
  <2>2. CASE Equivocate(p)       \* adversary: net grows, accepted unchanged
  <2>3. CASE Receive(p, m)       \* THE hard case: accepted grows by one
  <2>4. CASE Drop(m)             \* net shrinks, accepted unchanged
  <2>5. CASE UNCHANGED vars      \* stuttering
  <2>6. QED  BY <2>1, <2>2, <2>3, <2>4, <2>5
<1>3. Inv => NoFork
  BY DEF Inv
<1>4. QED  BY <1>1, <1>2, <1>3, PTL
```

**Obligation routing** (what each step dispatches to):

| Proof step | Backend | Difficulty |
|---|---|---|
| `Init => Inv` | SMT (Z3/CVC5) | trivial |
| `LocalMove`, `Equivocate`, `Drop`, stutter | SMT | easy — `accepted` unchanged, so the `accepted`-dependent conjuncts are preserved by `UNCHANGED` reasoning |
| `Receive` — `TypeOK'`, `RealVersions'` | SMT | easy |
| `Receive` — `HeadMatchesLog'` | SMT + `SequenceTheorems` | **hard** — requires `Last(Append(s,x)) = x` and `Len(Append(s,x)) = Len(s)+1` |
| `Receive` — `NoFork'` | SMT + the above | **hard** — the crux; needs `HeadMatchesLog` + guard |
| `Receive` — `SeqContiguous'`, `RecvSeqMatchesLog'` | SMT + `FiniteSetTheorems` (Cardinality) | **medium-hard** — cardinality-of-Append reasoning |
| final `PTL` | the temporal (Lamport) backend | trivial (pure propositional temporal glue) |

The two hard obligations are both about `Append`. Budget the bulk of the time
there. The standard tactic: prove small helper lemmas once —

```tla
LEMMA AppendLast == \A s, x : Last(Append(s, x)) = x
LEMMA AppendLen  == \A s, x : Len(Append(s, x)) = Len(s) + 1
```

— from `SequenceTheorems`, then feed them by name (`BY AppendLast, AppendLen`)
into the `Receive` case so the SMT backend never has to reason about sequences
from first principles.

---

## Phase 3 — SeqMonotonic, and the liveness decision (~1 week)

- **`SeqMonotonic`** falls out of `SeqContiguous` (already in `Inv`). A short
  corollary theorem: `BY Safety DEF SeqContiguous, SeqMonotonic`.

- **`Convergence` (liveness) stays in TLC.** Be honest about this: temporal /
  liveness proofs in TLAPS use the `PTL` backend plus explicit `WF`/`<>`
  reasoning, and they are genuinely research-grade hard — typically more effort
  than the entire safety proof. The standard, defensible engineering split is:
  **safety proven deductively (unbounded), liveness checked by TLC (bounded).**
  We adopt that split. Attempting the liveness proof is explicitly out of scope;
  if pursued later it is its own multi-week project with no guarantee of
  closing.

---

## Phase 4 — CI integration (~2 days)

A proof that isn't re-checked rots the moment the spec changes. Wire it in:

```yaml
# .github/workflows/formal.yml
- name: TLC bounded check
  run: |
    cd specs
    for cfg in CC_honest_safety CC_byzantine_current CC_byzantine_fixed; do
      java -cp tla2tools.jar tlc2.TLC -deadlock -config $cfg.cfg CausalChain.tla
    done
- name: TLAPS proof
  run: tlapm --toolbox 0 0 specs/CausalChainProof.tla
```

The TLC steps already encode the expected pass/fail (the `byzantine_current`
config is expected to find the fork — invert its exit check). The `tlapm` step
fails the build if any obligation regresses. Now the proof is a guarded
invariant on the protocol, not a one-time artifact.

---

## Mapping to the code

The proof is about the protocol, but each invariant corresponds to live state:

| Invariant | Code it pins down |
|---|---|
| `HeadMatchesLog` | `causal.head_version` ([systems.rs:391](../../src/multiplayer/systems.rs#L391)) always equals the last accepted move's `version_hash` |
| `RecvSeqMatchesLog` | `causal.last_seq` ([systems.rs:387](../../src/multiplayer/systems.rs#L387)) counts accepted moves per agent |
| `SeqContiguous` | the `seq == last + 1` check ([systems.rs:355](../../src/multiplayer/systems.rs#L355)) |
| `NoFork` | the equivocation guard ([systems.rs:369](../../src/multiplayer/systems.rs#L369), as fixed) |

If a future code change breaks one of these correspondences, the proof's
assumptions no longer match reality — so the proof must be re-derived alongside,
and the CI `tlapm` step is what forces that.

---

## Priority and effort

| Priority | Phase | Effort | De-risks |
|---|---|---|---|
| P0 | 0 — Reading | 1 week | — |
| **P0** | **1 — Apalache inductive check** | **1 week** | **~80% of the risk: validates `Inv` is inductive before any TLAPS cost** |
| P1 | 2 — TLAPS safety proof | 3–5 weeks | unbounded parameters |
| P2 | 3 — SeqMonotonic + liveness decision | 1 week | — |
| P2 | 4 — CI integration | 2 days | regression |

**Total: ~6–8 weeks** for unbounded safety. Liveness deliberately excluded.

The decision point is after Phase 1. If Apalache validates `Inv` as inductive,
the system already has unbounded-*depth* assurance for a fixed configuration —
which for a 2-player game with a fixed protocol is most of the practical value.
Phases 2–4 (full TLAPS) are then a judgement call: pursue them for a publishable
result and parameter-independence, or stop at Apalache and bank the engineering
win. Either way, Phase 1 is unambiguously worth doing.

---

## Honest risk assessment

- **Phase 1 is low-risk, high-value.** Apalache either validates `Inv` or hands
  you the exact missing conjunct. Days of work, large payoff. Do it regardless
  of whether Phases 2–4 happen.
- **Phase 2 is the schedule risk.** The `Append`/`Cardinality` obligations can
  resist the SMT backend and require manual lemma decomposition. 3–5 weeks is a
  realistic-but-not-guaranteed estimate; a stubborn cardinality lemma could add
  a week.
- **Liveness is not attempted.** Stated plainly so no one expects it.
- **The proof is configuration-fixed at the parameter level even in TLAPS** for
  `Agents` if we keep the 2-party model. True parametric proof over N agents
  depends on the multi-party model (Gap B) existing first. Sequencing: fix the
  identity-binding (Gap A), build the multi-party head model (Gap B), *then* a
  parametric proof becomes meaningful.

---

## What all three asterisks look like once this lands

| Asterisk | Plan | Status after |
|---|---|---|
| Persistence availability | [causal-authentication.md](causal-authentication.md) Gap C + [production-hardening.md](production-hardening.md) Gaps 1–2 | concrete, executable |
| Operational hardening | [production-hardening.md](production-hardening.md) | concrete, executable |
| Bounded → unbounded | **this document** | concrete, executable; Phase 1 de-risks the rest |

All three now have parity-level, executable coverage. The remaining honesty is
about *effort and risk*, not about *whether a plan exists* — and that risk is
isolated to TLAPS Phase 2, explicitly bounded, and front-loaded with a cheap
Apalache validation that captures most of the value on its own.
