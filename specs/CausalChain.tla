-------------------------------- MODULE CausalChain --------------------------------
(* Model per-agent sequencing and parent-linked move acceptance under Byzantine equivocation, *)
(* message reordering, loss, and replay. Versions model hashes as injective records. *)
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS
    Agents,        \* set of player identities, e.g. {A, B}
    MaxSeq,        \* bound on moves per agent (keeps the model finite)
    MaxContent,    \* bound on distinct board-contents at a seq (>= MaxSeq)
    Byzantine,     \* subset of Agents allowed to equivocate; {} = all honest
    CheckParent,   \* TRUE  = model the equivocation guard present
                   \* FALSE = guard removed, to show it is NECESSARY
    GenesisBypass, \* TRUE skips parent validation for genesis; FALSE requires the existing head.
    AuthBinding,   \* TRUE binds identity to the signer and roster; FALSE permits impersonation.
    EnableAdversary \* TRUE  = a forging third node is present on the topic.
                    \*         Enabled only in the impersonation configs.

VARIABLES
    own_seq,       \* [Agents -> Nat]      sender: count of own published moves
    last_version,  \* [Agents -> Version]  sender: version of own last move
    net,           \* SUBSET Message       in-flight gossip (set => reorder/dup)
    recv_last_seq, \* [Agents -> [Agents -> Nat]]  receiver: last seq per sender
    head,          \* [Agents -> Version]  receiver: head_version (single slot)
    accepted       \* [Agents -> Seq(Message)]     receiver: ordered accept log

vars == <<own_seq, last_version, net, recv_last_seq, head, accepted>>

----------------------------------------------------------------------------
(* genesis sentinel = the "0" parent_version / empty head in the Rust code *)
Genesis == [agent |-> "gen", seq |-> 0, content |-> 0]

Version == [agent: Agents \cup {"gen"}, seq: 0..MaxSeq, content: 0..MaxContent]

\* authentic binds a message to its identity’s signer; a forging adversary cannot satisfy it.
Message == [sender: Agents, seq: 1..MaxSeq, parent: Version, version: Version,
            authentic: BOOLEAN]

(* Honest content is deterministic for each (agent, sequence) pair. *)
HonestVersion(p, n) == [agent |-> p, seq |-> n, content |-> n]

----------------------------------------------------------------------------
Init ==
    /\ own_seq       = [p \in Agents |-> 0]
    /\ last_version  = [p \in Agents |-> Genesis]
    /\ net           = {}
    /\ recv_last_seq = [p \in Agents |-> [q \in Agents |-> 0]]
    /\ head          = [p \in Agents |-> Genesis]
    /\ accepted      = [p \in Agents |-> << >>]

----------------------------------------------------------------------------
LocalMove(p) ==
    /\ own_seq[p] < MaxSeq
    /\ LET n   == own_seq[p] + 1
           ver == HonestVersion(p, n)
           msg == [sender |-> p, seq |-> n,
                   parent |-> last_version[p], version |-> ver,
                   authentic |-> TRUE]
       IN /\ net'          = net \cup {msg}
          /\ own_seq'       = [own_seq      EXCEPT ![p] = n]
          /\ last_version'  = [last_version EXCEPT ![p] = ver]
    /\ UNCHANGED <<recv_last_seq, head, accepted>>

(* A Byzantine peer may publish conflicting contents or an unrelated parent. *)
Equivocate(p) ==
    /\ p \in Byzantine
    /\ \E n \in 1..MaxSeq, c \in 1..MaxContent,
          par \in {Genesis, last_version[p]} :
         LET ver == [agent |-> p, seq |-> n, content |-> c]
             msg == [sender |-> p, seq |-> n, parent |-> par, version |-> ver,
                     authentic |-> TRUE]
         IN net' = net \cup {msg}
    /\ UNCHANGED <<own_seq, last_version, recv_last_seq, head, accepted>>

(* An adversary can join the gossip topic and claim another identity without its key; *)
(* such messages have authentic=FALSE. *)
Adversary ==
    /\ EnableAdversary
    /\ \E s \in Agents :
         \E n \in 1..MaxSeq, c \in 1..MaxContent,
            par \in {Genesis, last_version[s]} :
            LET ver == [agent |-> s, seq |-> n, content |-> c]
                msg == [sender |-> s, seq |-> n, parent |-> par, version |-> ver,
                        authentic |-> FALSE]
            IN net' = net \cup {msg}
    /\ UNCHANGED <<own_seq, last_version, recv_last_seq, head, accepted>>

(* Mirror every rejection branch in the Rust causal-chain validator. *)
ParentOk(p, m) ==
    \/ ~CheckParent                          \* guard removed entirely
    \/ head[p] = Genesis                      \* code: our_head is empty -> skip
    \/ (GenesisBypass /\ m.parent = Genesis)  \* code: parent_version == "0" -> skip
    \/ m.parent = head[p]                     \* code: parent_version == our_head

CanAccept(p, m) ==
    /\ m \in net
    /\ m.sender # p                              \* no loopback (own moves)
    /\ (AuthBinding => m.authentic)               \* bind_identity + roster (Gap A)
    /\ m.seq = recv_last_seq[p][m.sender] + 1     \* seq continuity
    /\ ParentOk(p, m)                             \* equivocation guard

Receive(p, m) ==
    /\ CanAccept(p, m)
    /\ recv_last_seq' = [recv_last_seq EXCEPT ![p][m.sender] = m.seq]
    /\ head'          = [head          EXCEPT ![p] = m.version]
    /\ accepted'      = [accepted      EXCEPT ![p] = Append(@, m)]
    /\ UNCHANGED <<own_seq, last_version, net>>

Drop(m) ==
    /\ m \in net
    /\ net' = net \ {m}
    /\ UNCHANGED <<own_seq, last_version, recv_last_seq, head, accepted>>

----------------------------------------------------------------------------
Next ==
    \/ \E p \in Agents : LocalMove(p)
    \/ \E p \in Agents : Equivocate(p)
    \/ Adversary
    \/ \E p \in Agents, m \in net : Receive(p, m)
    \/ \E m \in net : Drop(m)

SafeSpec == Init /\ [][Next]_vars

(* Liveness configs: no Drop, plus weak fairness so progress must happen.    *)
LiveNext ==
    \/ \E p \in Agents : LocalMove(p)
    \/ \E p \in Agents : Equivocate(p)
    \/ Adversary
    \/ \E p \in Agents, m \in net : Receive(p, m)

LiveSpec ==
    /\ Init
    /\ [][LiveNext]_vars
    /\ \A p \in Agents : WF_vars(LocalMove(p))
    /\ \A p \in Agents : WF_vars(\E m \in net : Receive(p, m))

----------------------------------------------------------------------------

(* Cap in-flight adversarial messages to keep the state space finite; *)
(* this bounds breadth without weakening the invariant. *)
NetBounded == Cardinality(net) <= 3

TypeOK ==
    /\ own_seq      \in [Agents -> 0..MaxSeq]
    /\ last_version \in [Agents -> Version]
    /\ net \subseteq Message
    /\ recv_last_seq \in [Agents -> [Agents -> 0..MaxSeq]]
    /\ head \in [Agents -> Version]
    /\ \A p \in Agents :
         /\ Len(accepted[p]) <= MaxSeq
         /\ \A i \in 1..Len(accepted[p]) : accepted[p][i] \in Message

(* Each accepted move after the first must name the preceding accepted version as parent. *)
NoFork ==
    \A p \in Agents :
      \A i \in 2..Len(accepted[p]) :
        accepted[p][i].parent = accepted[p][i-1].version

(* Accepted remote sequences are contiguous and unique, starting at 1. *)
SeqMonotonic ==
    \A p \in Agents :
      \A i \in 1..Len(accepted[p]) : accepted[p][i].seq = i

NoEquivocationAccepted ==
    \A p \in Agents :
      \A i, j \in 1..Len(accepted[p]) :
        (accepted[p][i].seq = accepted[p][j].seq)
          => (accepted[p][i] = accepted[p][j])

(* Only verified identities may advance the chain; requires AuthBinding. *)
OnlyAuthenticAccepted ==
    \A p \in Agents :
      \A i \in 1..Len(accepted[p]) : accepted[p][i].authentic

----------------------------------------------------------------------------

(* Without permanent loss and with weak fairness, all honest move streams are delivered. *)
Convergence ==
    <>( \A p \in Agents :
          \A q \in Agents \ Byzantine :
            (p # q) => (recv_last_seq[p][q] = MaxSeq) )

============================================================================
