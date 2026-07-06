-------------------------------- MODULE CausalChain --------------------------------
(***************************************************************************)
(* Formal model of the XFChess online causal-chain move protocol.          *)
(*                                                                         *)
(* This models the EXACT logic of two code paths in the game client:       *)
(*                                                                         *)
(*   publish_local_move      (src/multiplayer/network/online_game_session.rs:127) *)
(*   handle_network_events   (src/multiplayer/systems.rs:341-398)          *)
(*                                                                         *)
(* Sender side (publish_local_move):                                       *)
(*   - own_seq[p] counts a peer's OWN moves; it starts at 0 and is         *)
(*     incremented once per own publish (next_move_number).                *)
(*   - parent_version = the peer's own last published version              *)
(*     (session.last_version), genesis ("0") for the first move.           *)
(*   - the new version = hash(next_fen, move_number); we model the hash    *)
(*     as an injective record [agent, seq, content].                       *)
(*                                                                         *)
(* Receiver side (handle_network_events causal block):                     *)
(*   - per-agent seq continuity:   seq == last_seq[(game,agent)] + 1       *)
(*   - equivocation guard: if parent != "0" AND our head != ""             *)
(*                         then require parent == our head                 *)
(*   - on accept: last_seq[(game,agent)] = seq ; head[game] = version      *)
(*                                                                         *)
(* The model checks that these two checks together prevent any fork: the   *)
(* accepted move log is always a single linear parent-linked chain, even   *)
(* when a Byzantine peer equivocates and the network reorders / drops /    *)
(* replays messages.                                                       *)
(***************************************************************************)
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS
    Agents,        \* set of player identities, e.g. {A, B}
    MaxSeq,        \* bound on moves per agent (keeps the model finite)
    MaxContent,    \* bound on distinct board-contents at a seq (>= MaxSeq)
    Byzantine,     \* subset of Agents allowed to equivocate; {} = all honest
    CheckParent,   \* TRUE  = model the equivocation guard present
                   \* FALSE = guard removed, to show it is NECESSARY
    GenesisBypass, \* TRUE  = models the CURRENT code: the parent check is
                   \*         skipped whenever parent_version == "0".
                   \* FALSE = models the FIX: once head is set, parent must
                   \*         equal head even if the sender claims "0".
    AuthBinding,   \* TRUE  = models bind_identity + roster (Gap A): a peer can
                   \*         only act under its OWN authenticated identity.
                   \* FALSE = models the pre-fix wire `agent_id`: a Byzantine
                   \*         peer can forge another peer's identity (impersonate).
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

\* `authentic` models whether the message is bound to the signer who owns the
\* claimed identity. Honest/equivocating PLAYERS sign with their own key
\* (authentic = TRUE). A forging adversary cannot (authentic = FALSE).
Message == [sender: Agents, seq: 1..MaxSeq, parent: Version, version: Version,
            authentic: BOOLEAN]

(* an honest move's content is a deterministic function of (agent, seq):    *)
(* the board after a peer's n-th move is fixed by the game history.          *)
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
(* An honest local move: mirrors publish_local_move exactly.                *)
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

(* A Byzantine peer equivocates: it forks its OWN chain by publishing a     *)
(* message whose parent is its current head but whose content (hence        *)
(* version) differs, and/or a message with an out-of-line parent.           *)
Equivocate(p) ==
    /\ p \in Byzantine
    /\ \E n \in 1..MaxSeq, c \in 1..MaxContent,
          par \in {Genesis, last_version[p]} :
         LET ver == [agent |-> p, seq |-> n, content |-> c]
             msg == [sender |-> p, seq |-> n, parent |-> par, version |-> ver,
                     authentic |-> TRUE]
         IN net' = net \cup {msg}
    /\ UNCHANGED <<own_seq, last_version, recv_last_seq, head, accepted>>

(* A forging adversary (a third node on the open gossip topic) injects a       *)
(* message under SOMEONE ELSE'S identity. It does not hold that identity's     *)
(* key, so the message is `authentic |-> FALSE`. This is the impersonation     *)
(* the bind_identity + roster fix (Gap A) is meant to stop. Always enabled:    *)
(* anyone can join an iroh gossip topic.                                       *)
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

(* The receiver's accept predicate = the negation of every "continue"       *)
(* (reject) branch in the Rust causal block.                                *)
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

(* The network drops an in-flight message (partition / loss).               *)
Drop(m) ==
    /\ m \in net
    /\ net' = net \ {m}
    /\ UNCHANGED <<own_seq, last_version, recv_last_seq, head, accepted>>

----------------------------------------------------------------------------
(* Full next-state relation (safety configs): includes loss.                *)
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
(*                              INVARIANTS                                   *)

(* State-space bound used (via CONSTRAINT) only by the adversary configs: a    *)
(* forging adversary can flood `net` with messages that are never acceptable   *)
(* under AuthBinding, so we cap the number of simultaneous in-flight messages  *)
(* to keep the model finite. This bounds breadth, not the invariant.           *)
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

(* The central safety theorem.  The accepted log of every receiver is a      *)
(* single linear parent-linked chain: each accepted move (after the first)   *)
(* names the immediately preceding accepted move as its parent.  No fork     *)
(* is ever admitted.                                                         *)
NoFork ==
    \A p \in Agents :
      \A i \in 2..Len(accepted[p]) :
        accepted[p][i].parent = accepted[p][i-1].version

(* Sequence numbers accepted from the (single, in 2-party play) remote        *)
(* sender are exactly 1, 2, 3, ... with no gaps or repeats.                  *)
SeqMonotonic ==
    \A p \in Agents :
      \A i \in 1..Len(accepted[p]) : accepted[p][i].seq = i

(* No two DIFFERENT moves are ever accepted at the same sequence number      *)
(* (direct statement of "equivocation never lands").                        *)
NoEquivocationAccepted ==
    \A p \in Agents :
      \A i, j \in 1..Len(accepted[p]) :
        (accepted[p][i].seq = accepted[p][j].seq)
          => (accepted[p][i] = accepted[p][j])

(* Gap A safety: no forged (non-authentic) move is ever accepted. A peer can  *)
(* only advance the chain under an identity it actually holds the key for.    *)
(* Violated when AuthBinding = FALSE (the adversary's impersonation lands);    *)
(* holds when AuthBinding = TRUE (bind_identity + roster reject it).           *)
OnlyAuthenticAccepted ==
    \A p \in Agents :
      \A i \in 1..Len(accepted[p]) : accepted[p][i].authentic

----------------------------------------------------------------------------
(*                              LIVENESS                                     *)

(* With no permanent message loss and weak fairness, every honest peer's     *)
(* full move stream is eventually delivered to the other honest peer.        *)
Convergence ==
    <>( \A p \in Agents :
          \A q \in Agents \ Byzantine :
            (p # q) => (recv_last_seq[p][q] = MaxSeq) )

============================================================================
