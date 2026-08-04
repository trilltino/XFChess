-------------------------------- MODULE BraidChain --------------------------------
(***************************************************************************)
(* Formal model of the XFChess Braid transport, added alongside P2P gossip *)
(* in the 2026-08-02 "Braid-first-class" migration. Deliberately a         *)
(* SEPARATE module from CausalChain.tla: Braid is backend-sequenced (one   *)
(* shared head per game+stream), not per-agent P2P broadcast, so it does   *)
(* not share CausalChain.tla's structure. Touching that module to bolt     *)
(* this on would risk perturbing its already-verified, already-cited      *)
(* (15.2M / 43.2M state) results for no reason.                            *)
(*                                                                         *)
(* Two independent properties of two real code paths are modelled here:    *)
(*                                                                         *)
(* (1) AUTHORIZATION (backend/src/signing/routes/game_log.rs, put_event):  *)
(*     a PUT is accepted iff content_parent == the stream's current head.  *)
(*     As shipped in the same session that added this file, the auth      *)
(*     check (auth_ok) verifies only that the caller holds *some* valid    *)
(*     platform session — not that they are a registered participant of    *)
(*     THIS game_id. AuthCheck = FALSE models that (the bug); AuthCheck =  *)
(*     TRUE models the fix (a per-game participant roster, built from      *)
(*     ChessMessage::SessionInfo posts, mirroring CausalChainState.roster).*)
(*                                                                         *)
(* (2) CROSS-TRANSPORT DEDUP (src/multiplayer/network/braid_transport.rs's *)
(*     drain_braid_messages + systems.rs's gossip causal-chain block):     *)
(*     the same move is published over BOTH gossip and Braid. Whichever    *)
(*     arrives first must "win"; the second must not double-apply to the   *)
(*     board. appliedVersions models CausalChainState.applied_versions.    *)
(*     DedupPresent = FALSE models it removed, to show it is NECESSARY     *)
(*     (mirrors CC_byzantine_broken.cfg's necessity pattern).              *)
(*                                                                         *)
(* The two sub-models use disjoint variables and don't interact; they are  *)
(* combined in one spec (rather than two files) because they describe two  *)
(* facets of the same new transport, and each config only asserts the      *)
(* invariant relevant to what it's checking.                               *)
(***************************************************************************)
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS
    Agents,          \* the game's two registered participants, e.g. {A, B}
    Outsiders,        \* agents with a VALID platform session but NOT a
                       \* participant of THIS game — the realistic Finding-4
                       \* threat (unlike CausalChain.tla's forging adversary,
                       \* this one has real credentials, just for the wrong game)
    MaxSeq,           \* bound on accepted/dispatched moves (keeps the model finite)
    MaxContent,       \* bound on distinct board-contents at a seq
    AuthCheck,        \* TRUE  = models the FIX: poster must be a participant
                       \* FALSE = models CURRENT auth_ok: any valid session accepted
    DedupPresent      \* TRUE  = models CausalChainState.applied_versions present
                       \* FALSE = models it removed, to show necessity

VARIABLES
    \* ---- (1) authorization sub-model ----
    seq,              \* Nat              count of accepted moves so far
    head,             \* Version          single shared head (one game+stream)
    accepted,         \* Seq(Message)     ordered accept log
    \* ---- (2) cross-transport dedup sub-model ----
    gossipNet,        \* SUBSET Version   in-flight gossip-delivered copies
    braidNet,         \* SUBSET Version   in-flight Braid-delivered copies
    appliedVersions,  \* SUBSET Version   dedup set (CausalChainState.applied_versions)
    dispatched        \* Seq(Version)     board-apply log (what actually got applied)

vars == <<seq, head, accepted, gossipNet, braidNet, appliedVersions, dispatched>>

----------------------------------------------------------------------------
Genesis == [seq |-> 0, content |-> 0]
Version == [seq: 0..MaxSeq, content: 0..MaxContent]
Message == [sender: Agents \cup Outsiders, seq: 1..MaxSeq, parent: Version, version: Version]
Participants == Agents

----------------------------------------------------------------------------
Init ==
    /\ seq             = 0
    /\ head            = Genesis
    /\ accepted        = << >>
    /\ gossipNet       = {}
    /\ braidNet        = {}
    /\ appliedVersions = {}
    /\ dispatched      = << >>

----------------------------------------------------------------------------
(* ================= (1) Authorization sub-model ================= *)

(* put_event()'s actual accept predicate: causal continuity is unconditional; *)
(* participant membership is gated by the AuthCheck switch.                   *)
CanAcceptAuth(p, m) ==
    /\ m.parent = head
    /\ (AuthCheck => p \in Participants)

(* A registered participant PUTs their own next move, correctly chained.    *)
HonestPut(p) ==
    /\ p \in Agents
    /\ seq < MaxSeq
    /\ LET n   == seq + 1
           ver == [seq |-> n, content |-> n]
           msg == [sender |-> p, seq |-> n, parent |-> head, version |-> ver]
       IN /\ CanAcceptAuth(p, msg)
          /\ seq'      = n
          /\ head'     = ver
          /\ accepted' = Append(accepted, msg)
    /\ UNCHANGED <<gossipNet, braidNet, appliedVersions, dispatched>>

(* An outsider — real platform session, WRONG game — PUTs a forged move    *)
(* into this game's stream. Under the current (broken) auth_ok this is     *)
(* accepted whenever it's also causally valid (an outsider can always      *)
(* satisfy that by reading the current head via GET first).                *)
OutsiderPut ==
    /\ seq < MaxSeq
    /\ \E p \in Outsiders, c \in 1..MaxContent :
         LET n   == seq + 1
             ver == [seq |-> n, content |-> c]
             msg == [sender |-> p, seq |-> n, parent |-> head, version |-> ver]
         IN /\ CanAcceptAuth(p, msg)
            /\ seq'      = n
            /\ head'     = ver
            /\ accepted' = Append(accepted, msg)
    /\ UNCHANGED <<gossipNet, braidNet, appliedVersions, dispatched>>

----------------------------------------------------------------------------
(* ================= (2) Cross-transport dedup sub-model ================= *)

(* An honest move is produced once and offered to BOTH transports at once —  *)
(* models `publish_local_move` calling both `network_state.message_sender`   *)
(* (gossip) and `braid_transport::publish_move` (Braid) for the same move.   *)
ProduceMove ==
    /\ Len(dispatched) + Cardinality(gossipNet \cup braidNet) < MaxSeq
    /\ \E c \in 1..MaxContent :
         LET ver == [seq |-> Len(dispatched) + 1, content |-> c]
         IN /\ gossipNet' = gossipNet \cup {ver}
            /\ braidNet'  = braidNet  \cup {ver}
    /\ UNCHANGED <<seq, head, accepted, appliedVersions, dispatched>>

(* Deliver one pending copy from a transport. Gated by the dedup set        *)
(* exactly like systems.rs's causal-chain block / drain_braid_messages:     *)
(* `applied_versions.insert(version)` returning false (already present)     *)
(* skips dispatch to the board.                                             *)
DeliverGossip ==
    /\ \E v \in gossipNet :
         /\ gossipNet' = gossipNet \ {v}
         /\ IF DedupPresent /\ v \in appliedVersions
              THEN UNCHANGED <<appliedVersions, dispatched>>
              ELSE /\ appliedVersions' = appliedVersions \cup {v}
                   /\ dispatched'      = Append(dispatched, v)
    /\ UNCHANGED <<seq, head, accepted, braidNet>>

DeliverBraid ==
    /\ \E v \in braidNet :
         /\ braidNet' = braidNet \ {v}
         /\ IF DedupPresent /\ v \in appliedVersions
              THEN UNCHANGED <<appliedVersions, dispatched>>
              ELSE /\ appliedVersions' = appliedVersions \cup {v}
                   /\ dispatched'      = Append(dispatched, v)
    /\ UNCHANGED <<seq, head, accepted, gossipNet>>

----------------------------------------------------------------------------
Next ==
    \/ \E p \in Agents : HonestPut(p)
    \/ OutsiderPut
    \/ ProduceMove
    \/ DeliverGossip
    \/ DeliverBraid

Spec == Init /\ [][Next]_vars

----------------------------------------------------------------------------
(*                              INVARIANTS                                   *)

TypeOK ==
    /\ seq \in 0..MaxSeq
    /\ head \in Version
    /\ Len(accepted) <= MaxSeq
    /\ \A i \in 1..Len(accepted) : accepted[i] \in Message
    /\ gossipNet \subseteq Version
    /\ braidNet \subseteq Version
    /\ appliedVersions \subseteq Version
    /\ Len(dispatched) <= MaxSeq

(* Finding 4's central claim: no move from a non-participant is ever        *)
(* accepted into a game's Braid stream. Violated when AuthCheck = FALSE     *)
(* (the shipped bug); holds when AuthCheck = TRUE (the roster fix).         *)
OnlyParticipantsAccepted ==
    \A i \in 1..Len(accepted) : accepted[i].sender \in Participants

(* A given version is ever dispatched to the board at most once, regardless *)
(* of whether gossip, Braid, or both delivered it. Violated when            *)
(* DedupPresent = FALSE (proving the dedup set is necessary, not merely     *)
(* convenient); holds when DedupPresent = TRUE.                             *)
NoDoubleApply ==
    \A i, j \in 1..Len(dispatched) :
        (dispatched[i] = dispatched[j]) => (i = j)

============================================================================
