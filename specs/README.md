# XFChess Formal Specifications (TLA+)

Mechanically-checked models of the XFChess consistency protocol, executed with
the TLC model checker. This is the engineering deliverable from Phases 1–4 of
[docs/plans/tla-causal-chain.md](../docs/plans/tla-causal-chain.md).

**Headline result:** TLC found a real, exploitable gap in the live P2P
causal-chain code (the `parent_version == "0"` bypass), produced a concrete
attack trace, and then verified that the proposed fix closes it across 15.2
million states. See [Finding 1](#finding-1--genesis-parent-bypass-real-bug).

---

## What is modelled

Two specs, each mapped line-for-line to real code:

| Spec | Models | Source code |
|---|---|---|
| `CausalChain.tla` | P2P move gossip: seq continuity, equivocation guard, identity binding | `src/multiplayer/network/braid_pvp.rs:127` (send), `src/multiplayer/systems.rs` (`bind_identity` + causal/roster block) |
| `SolanaFinality.tla` | On-chain settlement: nonce + parent_nonce checks; N replica submitters | `programs/xfchess-game/src/moves_ix/record.rs:64-71` |

The model has three switches that select which version of the protocol is being
checked, each mapped to a code change:

| Switch | TRUE models | FALSE models |
|---|---|---|
| `GenesisBypass` | the original `parent_version != "0"` skip (the bug) | the fix (parent must match head once set) |
| `AuthBinding` | `bind_identity` + roster: only authenticated participants' moves land | the pre-fix wire `agent_id`, forgeable by anyone |
| `EnableAdversary` | a forging third node is present on the open topic | no impersonator |

The models are deliberately faithful, not idealised. Where the Rust code skips a
check, the model skips it too — which is exactly why TLC could find the bug.

---

## How to run

Requires Java 11+ (tested on OpenJDK 21). Download the TLC model checker into
this directory first (it is gitignored — a 4 MB binary):

```bash
curl -sL -o tla2tools.jar \
  https://github.com/tlaplus/tlaplus/releases/download/v1.8.0/tla2tools.jar
```

```bash
cd specs

# P2P causal chain — five configurations
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_honest_safety.cfg     CausalChain.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_honest_live.cfg       CausalChain.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_byzantine_current.cfg CausalChain.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_byzantine_fixed.cfg   CausalChain.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_byzantine_broken.cfg  CausalChain.tla

# Solana finality — two configurations
java -cp tla2tools.jar tlc2.TLC -deadlock -config SF_normal.cfg   SolanaFinality.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config SF_no_nonce.cfg SolanaFinality.tla
```

`-deadlock` disables deadlock detection: these are bounded models that reach a
legitimate terminal state (all moves made and delivered), which TLC would
otherwise report as a deadlock.

---

## Results

All runs reproduced on OpenJDK 21, TLC 2026.05.26.

| Config | Scenario | Result |
|---|---|---|
| `CC_honest_safety`        | 2 honest peers, network reorders/drops/replays | ✅ all invariants hold |
| `CC_honest_live`          | liveness: peers always converge (fairness, no loss) | ✅ `Convergence` holds |
| `CC_byzantine_current`    | **genesis-bug code** + 1 Byzantine peer | ❌ **`NoFork` VIOLATED** (Finding 1) |
| `CC_byzantine_fixed`      | genesis fix applied + Byzantine | ✅ `NoFork` holds (15.2M states) |
| `CC_byzantine_broken`     | equivocation guard removed + Byzantine | ❌ `NoFork` violated (necessity) |
| `CC_impersonation_current`| **pre-auth-fix** + forging adversary | ❌ **`OnlyAuthenticAccepted` VIOLATED** (Finding 3) |
| `CC_impersonation_fixed`  | `bind_identity` + roster + forging adversary | ✅ `OnlyAuthenticAccepted` holds |
| `SF_normal`               | Solana checks + Byzantine + reordering mempool | ✅ `ChainLinearizable` holds (43.2M states) |
| `SF_no_nonce`             | nonce check removed | ❌ violated (necessity) |
| `SF_replicas`             | 4 redundant submitters (2 players + 2 replicas), no consensus | ✅ `ChainLinearizable` holds (Gap C) |

---

## Status: fixes are in the code

Both findings have been fixed in the live client and tied to the configs above:

- **Finding 1 (genesis bypass)** — fixed in `src/multiplayer/systems.rs` (the
  equivocation guard now keys on whether a head exists, not on `parent != "0"`).
- **Finding 3 (impersonation)** — fixed by `bind_identity` (receiver substitutes
  the verified signer for the claimed `agent_id`), a per-game roster check
  populated from `SessionInfo`, per-sender head lanes, and rejection of unsigned
  messages by default (the `allow-unsigned-p2p` feature re-enables them for dev).
  The multiplayer test suite includes an impersonation regression test
  (`auth_tests::bind_identity_uses_verified_signer_not_claimed_agent_id`).

## Invariants checked

**`CausalChain.tla`**
- `NoFork` — every receiver's accepted log is a single linear parent-linked
  chain: move *i* names move *i−1*'s version as its parent. No fork is ever
  admitted. *(the central safety theorem)*
- `SeqMonotonic` — accepted sequence numbers are exactly 1, 2, 3, … with no
  gaps or repeats.
- `NoEquivocationAccepted` — no two distinct moves are ever accepted at the
  same sequence number.
- `OnlyAuthenticAccepted` *(Gap A)* — no forged (non-authentic) move is ever
  accepted; a peer can only advance the chain under an identity it holds the key
  for. Checked against a forging adversary in the impersonation configs.
- `Convergence` *(liveness)* — under weak fairness and no permanent loss, both
  honest peers' full move streams are eventually delivered.

**`SolanaFinality.tla`**
- `ChainLinearizable` — the committed on-chain history is a gap-free nonce
  sequence (1, 2, 3, …) with consistent parents, even under a reordering
  mempool and Byzantine submitters.

---

## Finding 1 — genesis parent bypass (REAL BUG)

`CC_byzantine_current.cfg` models the live code exactly and **fails**. TLC's
counterexample (saved in [`NoFork_counterexample.txt`](NoFork_counterexample.txt)):

1. Byzantine peer **A** publishes an honest move `seq=1`, `parent="0"`.
   Peer **B** accepts it; B's head becomes A's version 1.
2. A **equivocates**: publishes `seq=2` but with `parent="0"` (the genesis
   sentinel) and different content.
3. B receives the forged move. The seq check passes (`2 == 1+1`). The
   equivocation guard is **skipped** because the receiver code is:

   ```rust
   if !parent_version.is_empty() && parent_version != "0" {
       // ... only here is parent compared against our head ...
   }
   ```

   With `parent_version == "0"`, the whole guard is bypassed. B accepts a move
   whose parent is the root, not the previous move — a fork. `NoFork` is
   violated.

**Why it matters:** the parent-version check is the protocol's entire
anti-equivocation mechanism at the P2P layer. Any peer can defeat it by
attaching the literal `"0"` to a move that otherwise has the right sequence
number, causing the opponent's local head to diverge from the true game chain.
Solana still catches this at settlement, but the immediate-consistency
guarantee the causal chain is supposed to provide is broken.

**The fix** (verified by `CC_byzantine_fixed.cfg`, 15.2M states, no violation):
the `parent_version == "0"` escape must apply **only to the first move**, when
the receiver's head is still empty. Once a head exists, every move must name it
as parent — `"0"` included. Concretely, in
`src/multiplayer/systems.rs`, change the guard from skipping on
`parent_version != "0"` to skipping only when `our_head.is_empty()`:

```rust
// BEFORE (bypassable):
if !parent_version.is_empty() && parent_version != "0" {
    if !our_head.is_empty() && parent_version != &our_head { reject }
}

// AFTER (fixed):
if !our_head.is_empty() {
    // game has progressed — parent MUST match our head, "0" or not
    if parent_version != &our_head { reject }
}
```

---

## Finding 2 — nonce check, not parent_nonce, gives linearizability

`SF_no_nonce.cfg` removes the strict `nonce == game.nonce + 1` check and
`ChainLinearizable` immediately fails, while `parent_nonce` alone cannot
restore it. Conversely `SF_normal.cfg` passes with Byzantine submitters across
43.2M states.

**Conclusion:** on-chain linearizability rests on the strict nonce increment.
`parent_nonce` (the `Option<u64>` added in `record.rs`) is genuine
defense-in-depth against client-side races, but it is not what prevents a fork —
the nonce monotonicity is. This is worth knowing: the nonce check must never be
weakened, whereas `parent_nonce` could be made mandatory (drop the `Option`)
for extra safety without changing the linearizability guarantee.

---

## Finding 3 — identity must be bound to the signer (impersonation)

`CC_impersonation_current.cfg` models the protocol BEFORE the authentication fix
(`AuthBinding = FALSE`): the `agent_id` carried in a move is trusted as-is. With
a forging third node on the open gossip topic, `OnlyAuthenticAccepted` is
**violated** — the adversary publishes a move under an honest peer's identity and
it is accepted as if genuine. Signing alone does not stop this: a valid signature
proves *a* key signed the message, not that the *claimed* identity's key did.

**The fix** (verified by `CC_impersonation_fixed.cfg`): the receiver discards the
claimed `agent_id` and substitutes the verified signer
(`bind_identity` in `src/multiplayer/systems.rs`), and a roster check rejects any
signer that is not one of the game's two registered session keys. With the fix on,
no forged move ever lands.

This finding is what the model assumed away in its first version — the
`Equivocate` action could only act as the peer itself. Making impersonation an
explicit, adversary-driven action turned that hidden assumption into a checked
property, and the code change (`bind_identity` + roster) is what discharges it.

The Solana layer (`SolanaFinality.tla`) is the backstop: even an accepted forged
P2P move cannot settle on-chain, because `record_move` requires the session key
registered in the on-chain `session_delegation` for the game.

---

## Scope and limitations

- **2 participants.** The model uses two agents (the PvP case). The receiver's
  `head_version` is a single slot per game; with exactly one remote sender this
  is sound. A 3+ participant topic (e.g. a shared spectator channel that also
  accepted moves) would make that slot flip between senders — out of scope here
  and worth a separate model if the topology ever changes.
- **No message authentication.** The model assumes a Byzantine peer acts as
  *itself* (cannot forge another peer's `agent_id`). The gossip messages are
  not signed at the causal-chain layer — only Solana transactions are. Modelling
  impersonation would require adding signatures to the protocol first; that gap
  is noted but not modelled.
- **Bounded.** TLC checks finite instances (`MaxSeq`/`MaxNonce` = 2–3). This is
  exhaustive within the bound, not a proof for unbounded games. Phase 5 (TLAPS
  deductive proof) in the plan would lift the bound; it is not done here.

---

## Files

```
CausalChain.tla            P2P causal-chain spec (Phases 1-3)
  CC_honest_safety.cfg       honest peers, safety
  CC_honest_live.cfg         honest peers, liveness/convergence
  CC_byzantine_current.cfg   current code + Byzantine  -> finds the bug
  CC_byzantine_fixed.cfg     fix applied + Byzantine    -> verifies the fix
  CC_byzantine_broken.cfg    guard removed              -> necessity
SolanaFinality.tla         on-chain settlement spec (Phase 4)
  SF_normal.cfg              real checks + Byzantine    -> linearizable
  SF_no_nonce.cfg            nonce check removed        -> necessity
NoFork_counterexample.txt  TLC's saved attack trace for Finding 1
tla2tools.jar              the TLC model checker (v1.8.0 line)
```
