-------------------------------- MODULE BraidChain --------------------------------
(* Model backend-sequenced Braid authorization and gossip/Braid deduplication independently. *)
(* AuthCheck requires game participation; DedupPresent prevents duplicate board application. *)
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS
    Agents,          \* the game's two registered participants, e.g. {A, B}
    Outsiders,        \* Valid platform sessions outside this game’s participant roster.
    MaxSeq,           \* bound on accepted/dispatched moves (keeps the model finite)
    MaxContent,       \* bound on distinct board-contents at a seq
    AuthCheck,        \* TRUE requires game participation; FALSE accepts any valid session.
    DedupPresent      \* TRUE deduplicates applied versions; FALSE demonstrates duplicate application.

VARIABLES
    seq,              \* Nat              count of accepted moves so far
    head,             \* Version          single shared head (one game+stream)
    accepted,         \* Seq(Message)     ordered accept log
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

(* Require causal continuity; AuthCheck additionally requires participant membership. *)
CanAcceptAuth(p, m) ==
    /\ m.parent = head
    /\ (AuthCheck => p \in Participants)

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

(* An outsider can read the current head and forge a causally valid PUT. *)
(* AuthCheck must reject it despite valid platform credentials. *)
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

(* Publish the same move over both gossip and Braid. *)
ProduceMove ==
    /\ Len(dispatched) + Cardinality(gossipNet \cup braidNet) < MaxSeq
    /\ \E c \in 1..MaxContent :
         LET ver == [seq |-> Len(dispatched) + 1, content |-> c]
         IN /\ gossipNet' = gossipNet \cup {ver}
            /\ braidNet'  = braidNet  \cup {ver}
    /\ UNCHANGED <<seq, head, accepted, appliedVersions, dispatched>>

(* Apply each version once across transports when DedupPresent is enabled. *)
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

TypeOK ==
    /\ seq \in 0..MaxSeq
    /\ head \in Version
    /\ Len(accepted) <= MaxSeq
    /\ \A i \in 1..Len(accepted) : accepted[i] \in Message
    /\ gossipNet \subseteq Version
    /\ braidNet \subseteq Version
    /\ appliedVersions \subseteq Version
    /\ Len(dispatched) <= MaxSeq

(* Only game participants may append; requires AuthCheck. *)
OnlyParticipantsAccepted ==
    \A i \in 1..Len(accepted) : accepted[i].sender \in Participants

(* A version is applied at most once across both transports; requires DedupPresent. *)
NoDoubleApply ==
    \A i, j \in 1..Len(dispatched) :
        (dispatched[i] = dispatched[j]) => (i = j)

============================================================================
