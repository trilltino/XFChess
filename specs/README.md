# XFChess Formal Specifications (TLA+)

Mechanically-checked models of the XFChess consistency protocol, executed with
the TLC model checker (the deliverable of the TLA causal-chain plan, Phases 1–4).

**Headline result:** TLC found a real, exploitable gap in the live P2P
causal-chain code (the `parent_version == "0"` bypass), produced a concrete
attack trace, and then verified that the proposed fix closes it across 15.2
million states. See [Finding 1](#finding-1--genesis-parent-bypass-real-bug).
The same model → violate → fix → verify method found a second real bug when
the Braid transport was added alongside gossip: PUT authorization wasn't
scoped to the game. See
[Finding 4](#finding-4--braid-put-authorization-was-not-scoped-to-the-game-real-bug).

---

## What is modelled

Three specs, each mapped line-for-line to real code:

| Spec | Models | Source code |
|---|---|---|
| `CausalChain.tla` | P2P gossip move transport: seq continuity, equivocation guard, identity binding | `src/multiplayer/network/online_game_session.rs:127` (send), `src/multiplayer/systems.rs` (`bind_identity` + causal/roster block) |
| `BraidChain.tla` | Braid transport (added 2026-08-02, alongside gossip): backend authorization, cross-transport dedup | `backend/src/signing/routes/game_log.rs` (`put_event`), `src/multiplayer/network/braid_transport.rs` (`drain_braid_messages`), `CausalChainState.applied_versions` (`src/multiplayer/types.rs`) |
| `SolanaFinality.tla` | On-chain settlement: nonce + parent_nonce checks; N replica submitters | `programs/xfchess-game/src/moves_ix/record.rs:64-71` |

`CausalChain.tla` has three switches that select which version of the gossip
protocol is being checked, each mapped to a code change:

| Switch | TRUE models | FALSE models |
|---|---|---|
| `GenesisBypass` | the original `parent_version != "0"` skip (the bug) | the fix (parent must match head once set) |
| `AuthBinding` | `bind_identity` + roster: only authenticated participants' moves land | the pre-fix wire `agent_id`, forgeable by anyone |
| `EnableAdversary` | a forging third node is present on the open topic | no impersonator |

`BraidChain.tla` has two independent switches for its two sub-models (see
[Finding 4](#finding-4--braid-put-authorization-was-not-scoped-to-the-game-real-bug)):

| Switch | TRUE models | FALSE models |
|---|---|---|
| `AuthCheck` | the fix: poster must be in the game's participant roster | the shipped bug: `auth_ok` only checks *some* valid session exists |
| `DedupPresent` | `CausalChainState.applied_versions` present | the set removed, to show it is necessary |

The models are deliberately faithful, not idealised. Where the Rust code skips a
check, the model skips it too — which is exactly why TLC could find the bugs.

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

# P2P causal chain
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_honest_safety.cfg     CausalChain.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_honest_live.cfg       CausalChain.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_byzantine_current.cfg CausalChain.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_byzantine_fixed.cfg   CausalChain.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_byzantine_broken.cfg  CausalChain.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_combined.cfg          CausalChain.tla   # equivocator + impersonator together
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_byzantine_live.cfg    CausalChain.tla   # liveness under Byzantine peer
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_impersonation_current.cfg CausalChain.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config CC_impersonation_fixed.cfg   CausalChain.tla

# Braid transport
java -cp tla2tools.jar tlc2.TLC -deadlock -config BC_auth_current.cfg  BraidChain.tla   # -> finds Finding 4
java -cp tla2tools.jar tlc2.TLC -deadlock -config BC_auth_fixed.cfg    BraidChain.tla   # -> verifies the fix
java -cp tla2tools.jar tlc2.TLC -deadlock -config BC_dedup_present.cfg BraidChain.tla   # -> dedup holds
java -cp tla2tools.jar tlc2.TLC -deadlock -config BC_dedup_absent.cfg  BraidChain.tla   # -> necessity

# Solana finality
java -cp tla2tools.jar tlc2.TLC -deadlock -config SF_normal.cfg        SolanaFinality.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config SF_no_nonce.cfg      SolanaFinality.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config SF_replicas.cfg      SolanaFinality.tla
java -cp tla2tools.jar tlc2.TLC -deadlock -config SF_impersonation.cfg SolanaFinality.tla   # on-chain auth backstop
java -cp tla2tools.jar tlc2.TLC -deadlock -config SF_no_auth.cfg       SolanaFinality.tla   # backstop necessity
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
| `BC_auth_current`         | **shipped `auth_ok`** (any valid session accepted) + non-participant with real credentials | ❌ **`OnlyParticipantsAccepted` VIOLATED** (Finding 4) — 4 states, violated at depth 2 |
| `BC_auth_fixed`           | per-game roster fix applied | ✅ `OnlyParticipantsAccepted` holds — 53,609 states generated, 19,155 distinct |
| `BC_dedup_present`        | `applied_versions` present, move delivered via both gossip + Braid | ✅ `NoDoubleApply` holds — 53,609 states generated, 19,155 distinct |
| `BC_dedup_absent`         | dedup set removed, same redundant delivery | ❌ `NoDoubleApply` violated (necessity) — 123 states generated, 76 distinct |
| `SF_normal`               | Solana checks + Byzantine + reordering mempool | ✅ `ChainLinearizable` holds (43.2M states) |
| `SF_no_nonce`             | nonce check removed | ❌ violated (necessity) |
| `SF_replicas`             | 4 redundant submitters (2 players + 2 replicas), no consensus | ✅ `ChainLinearizable` holds (Gap C) |

---

## Status: fixes are in the code

All findings have been fixed in the live code and tied to the configs above:

- **Finding 1 (genesis bypass)** — fixed in `src/multiplayer/systems.rs` (the
  equivocation guard now keys on whether a head exists, not on `parent != "0"`).
- **Finding 3 (impersonation)** — fixed by `bind_identity` (receiver substitutes
  the verified signer for the claimed `agent_id`), a per-game roster check
  populated from `SessionInfo`, per-sender head lanes, and rejection of unsigned
  messages by default (the `allow-unsigned-p2p` feature re-enables them for dev).
  The multiplayer test suite includes an impersonation regression test
  (`auth_tests::bind_identity_uses_verified_signer_not_claimed_agent_id`).
- **Finding 4 (Braid PUT authorization)** — fixed in
  `backend/src/signing/routes/game_log.rs`: a per-game participant roster,
  built from accepted `ChessMessage::SessionInfo` posts (mirroring
  `CausalChainState.roster`'s exact first-two-seen pattern, server-side), is
  now checked before any `move`/`resign`/draw-offer kind is accepted. Backend
  regression tests: `game_log::tests::non_participant_cannot_put_a_move`,
  `game_log::tests::empty_roster_does_not_block_casual_games`.

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

**`BraidChain.tla`**
- `OnlyParticipantsAccepted` — no move from a non-participant is ever accepted
  into a game's Braid stream, even one with valid platform credentials for a
  *different* game. Violated when `AuthCheck = FALSE` (the shipped bug);
  holds when `AuthCheck = TRUE` (the roster fix).
- `NoDoubleApply` — a move delivered redundantly over both gossip and Braid is
  dispatched to the board at most once. Violated when `DedupPresent = FALSE`
  (proving `CausalChainState.applied_versions` is necessary, not merely
  convenient); holds when `DedupPresent = TRUE`.

**`SolanaFinality.tla`**
- `ChainLinearizable` — the committed on-chain history is a gap-free nonce
  sequence (1, 2, 3, …) with consistent parents, even under a reordering
  mempool and Byzantine submitters.
- `OnlyAuthorizedCommitted` — no transaction from an author without a registered
  session key is ever committed, even when its nonce is otherwise valid. This is
  the on-chain authorization backstop (models the `session_delegation` roster
  checks in `record.rs:26-27`): it is what makes an accepted-but-forged P2P move
  harmless, because it can never settle. Checked against a forging outsider in
  `SF_impersonation`; shown necessary by `SF_no_auth`.

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

## Finding 4 — Braid PUT authorization was not scoped to the game (REAL BUG)

Found the same way Findings 1 and 3 were: not by auditing the new Braid
transport in isolation, but by asking whether the *existing* spec's
properties (specifically `OnlyAuthenticAccepted`'s "an identity can only act
under its own key") still held once a second delivery path was added
alongside gossip. They didn't.

`BC_auth_current.cfg` models the shipped `game_log.rs`/`auth_ok` exactly and
**fails** immediately (violated at search depth 2, 4 states):

1. `auth_ok` (`backend/src/signing/routes/game_log.rs`) checks that
   `player_pubkey` parses as a real Solana pubkey with a matching entry in
   `active_global_sessions` — a **platform-wide** session store, not scoped
   to any particular `game_id`.
2. `put_event`'s only other check is causal continuity: does
   `content_parent` match the stream's current head. An outsider can always
   satisfy this by reading the head via `GET` first.
3. So: a player with a real, valid session for *any* game on the platform —
   call them Mallory — could PUT a forged move into a *different* game's
   Braid stream. The receiving client applies it, because
   `braid_transport::drain_braid_messages` (the Braid consumer) does not run
   the P2P roster check gossip-delivered moves go through — it only gates
   on `applied_versions` (dedup), never on identity.

**Why it matters:** the P2P layer's roster check exists precisely to stop an
unregistered peer from injecting moves into a game (Finding 3). Adding a
second delivery path that skips that check reopens the same class of hole,
just through a different door — display-layer only (Braid moves are
rendered on the victim's board), not funds-affecting: on-chain `record_move`
still independently requires a `session_delegation` scoped to that exact
`game_id`, so this is the same backstop shape as Finding 3, not a new class
of on-chain risk. It's real griefing potential, though — an outsider could
confuse a victim mid-game with fabricated moves.

**The fix** (verified by `BC_auth_fixed.cfg`, 53,609 states generated,
19,155 distinct, no violation): a per-game participant roster, built
server-side the same way the P2P layer already builds one — from the first
two distinct wallet pubkeys seen in accepted `SessionInfo` posts for that
`game_id`. `move`/`resign`/`offer_draw`/`accept_draw`/`decline_draw` kinds
are checked against it once populated (matching the P2P roster's identical
"skip the check while still empty" bootstrap behavior, which is also what
naturally exempts casual/no-wallet games — they never post `SessionInfo` at
all, so their roster for that `game_id` simply never gets populated).

This finding also produced a second, related check in the same file:
`NoDoubleApply` (`BC_dedup_present.cfg`/`BC_dedup_absent.cfg`), formally
verifying `CausalChainState.applied_versions` — the set that stops a move
delivered redundantly over both gossip and Braid from being dispatched to
the board twice. Not a security finding (both transports are, post-fix,
equally trustworthy), but the same "same-session discovery" process that
found Finding 4 surfaced it as a safety property with no prior formal
coverage — only an empirical one
(`src/multiplayer/network/reorder.rs::dual_transport_interleave_duplicate_drop_fuzz`,
which covers gossip-side reordering but not the cross-transport interaction).
`BC_dedup_absent.cfg` confirms the set is load-bearing, not decorative
(violated in 123 states).

---

## Scope and limitations

- **2 participants.** The model uses two agents (the PvP case). The receiver's
  `head_version` is a single slot per game; with exactly one remote sender this
  is sound. A 3+ participant topic (e.g. a shared spectator channel that also
  accepted moves) would make that slot flip between senders — out of scope here
  and worth a separate model if the topology ever changes.
- **Impersonation IS modelled.** The `Adversary` action (`EnableAdversary`)
  models a forging third node injecting moves under another peer's `agent_id`,
  and `AuthBinding` models the `bind_identity` + roster fix. See Finding 3. The
  remaining authentication abstraction is that signatures are modelled as a
  single `authentic` boolean, not as concrete keys/crypto — sound for the
  identity-binding property, but it does not model signature forgery via key
  compromise.
- **Bounded.** TLC checks finite instances (`MaxSeq`/`MaxNonce` = 2–3). This is
  exhaustive within the bound, not a proof for unbounded games. Phase 5 (TLAPS
  deductive proof) in the plan would lift the bound; it is not done here.
- **`BraidChain.tla` covers Braid only.** It does not re-model gossip's own
  seq/equivocation/roster checks (already covered by `CausalChain.tla`) — its
  `HonestPut`/`OutsiderPut` actions model the backend's accept decision as a
  single atomic step (PUT request in, accept-or-reject out), which is
  faithful to Braid's actual architecture (one backend sequencer, not a
  peer-to-peer broadcast mesh with a separate publish/receive phase) rather
  than a simplification of it. The dedup sub-model likewise assumes each
  transport's own per-message validity (gossip's checks, Braid's causal
  check) already happened before a version reaches `gossipNet`/`braidNet` —
  it isolates the cross-transport dedup property rather than re-deriving
  everything both transports already separately guarantee.
- **Braid's roster fix reuses the P2P roster's exact bootstrap assumption**
  (skip the check while empty) without separately verifying that assumption
  is still sound when TWO independent rosters (P2P's `CausalChainState.roster`
  and Braid's `GameLogState.roster`) exist for the same game and could, in
  principle, race to populate differently. Not modelled — both are built
  from the same `SessionInfo` messages by the same first-two-seen rule, so
  in practice they converge, but this convergence itself is an assumption,
  not a proven invariant.

---

## Files

```
CausalChain.tla            P2P gossip causal-chain spec (Phases 1-3)
  CC_honest_safety.cfg       honest peers, safety
  CC_honest_live.cfg         honest peers, liveness/convergence
  CC_byzantine_current.cfg   current code + Byzantine  -> finds the bug
  CC_byzantine_fixed.cfg     fix applied + Byzantine    -> verifies the fix
  CC_byzantine_broken.cfg    guard removed              -> necessity
BraidChain.tla             Braid transport spec (added 2026-08-02)
  BC_auth_current.cfg        shipped auth_ok            -> finds Finding 4
  BC_auth_fixed.cfg          per-game roster fix         -> verifies the fix
  BC_dedup_present.cfg       applied_versions present    -> dedup holds
  BC_dedup_absent.cfg        dedup set removed           -> necessity
SolanaFinality.tla         on-chain settlement spec (Phase 4)
  SF_normal.cfg              real checks + Byzantine    -> linearizable
  SF_no_nonce.cfg            nonce check removed        -> necessity
NoFork_counterexample.txt  TLC's saved attack trace for Finding 1
tla2tools.jar              the TLC model checker (v1.8.0 line)
```
