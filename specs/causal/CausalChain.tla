-------------------------------- MODULE CausalChain --------------------------------

EXTENDS Naturals, Sequences, FiniteSets
CONSTANTS Agents, MaxSeq, MaxContent, Byzantine, CheckParent,
          GenesisBypass, AuthBinding, EnableAdversary, MaxInFlight

VARIABLES own_seq, last_version, net, recv_last_seq, head, accepted

vars == <<own_seq, last_version, net, recv_last_seq, head, accepted>>

Genesis == [agent |-> "gen", seq |-> 0, content |-> 0]

Version == [agent: Agents \cup {"gen"}, seq: 0..MaxSeq, content: 0..MaxContent]

Message == [sender: Agents, seq: 1..MaxSeq, parent: Version, version: Version,
            authentic: BOOLEAN]

HonestVersion(p, n) == [agent |-> p, seq |-> n, content |-> n]

Init ==
    /\ own_seq       = [p \in Agents |-> 0]
    /\ last_version  = [p \in Agents |-> Genesis]
    /\ net           = {}
    /\ recv_last_seq = [p \in Agents |-> [q \in Agents |-> 0]]
    /\ head          = [p \in Agents |-> [q \in Agents |-> Genesis]]
    /\ accepted      = [p \in Agents |-> [q \in Agents |-> << >>]]

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

Equivocate(p) ==
    /\ p \in Byzantine
    /\ \E n \in 1..MaxSeq, c \in 1..MaxContent,
          par \in {Genesis, last_version[p]} :
         LET ver == [agent |-> p, seq |-> n, content |-> c]
             msg == [sender |-> p, seq |-> n, parent |-> par, version |-> ver,
                     authentic |-> TRUE]
         IN net' = net \cup {msg}
    /\ UNCHANGED <<own_seq, last_version, recv_last_seq, head, accepted>>

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

ParentOk(p, m) ==
    \/ ~CheckParent
    \/ head[p][m.sender] = Genesis
    \/ (GenesisBypass /\ m.parent = Genesis)
    \/ m.parent = head[p][m.sender]

CanAccept(p, m) ==
    /\ m \in net
    /\ m.sender # p
    /\ (AuthBinding => m.authentic)
    /\ m.seq = recv_last_seq[p][m.sender] + 1
    /\ ParentOk(p, m)

Receive(p, m) ==
    /\ CanAccept(p, m)
    /\ recv_last_seq' = [recv_last_seq EXCEPT ![p][m.sender] = m.seq]
    /\ head'          = [head          EXCEPT ![p][m.sender] = m.version]
    /\ accepted'      = [accepted      EXCEPT ![p][m.sender] = Append(@, m)]
    /\ UNCHANGED <<own_seq, last_version, net>>

Drop(m) ==
    /\ m \in net
    /\ net' = net \ {m}
    /\ UNCHANGED <<own_seq, last_version, recv_last_seq, head, accepted>>

Next ==
    \/ \E p \in Agents : LocalMove(p)
    \/ \E p \in Agents : Equivocate(p)
    \/ Adversary
    \/ \E p \in Agents, m \in net : Receive(p, m)
    \/ \E m \in net : Drop(m)

SafeSpec == Init /\ [][Next]_vars

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

NetBounded == Cardinality(net) <= MaxInFlight

TypeOK ==
    /\ own_seq      \in [Agents -> 0..MaxSeq]
    /\ last_version \in [Agents -> Version]
    /\ net \subseteq Message
    /\ recv_last_seq \in [Agents -> [Agents -> 0..MaxSeq]]
    /\ head \in [Agents -> [Agents -> Version]]
    /\ \A p, q \in Agents :
         /\ Len(accepted[p][q]) <= MaxSeq
         /\ \A i \in 1..Len(accepted[p][q]) : accepted[p][q][i] \in Message

NoFork ==
    \A p, q \in Agents :
      \A i \in 2..Len(accepted[p][q]) :
        accepted[p][q][i].parent = accepted[p][q][i-1].version

SeqMonotonic ==
    \A p, q \in Agents :
      \A i \in 1..Len(accepted[p][q]) : accepted[p][q][i].seq = i

OnlyAuthenticAccepted ==
    \A p, q \in Agents :
      \A i \in 1..Len(accepted[p][q]) : accepted[p][q][i].authentic

Convergence ==
    <>( \A p \in Agents :
          \A q \in Agents \ Byzantine :
            (p # q) => (recv_last_seq[p][q] = MaxSeq) )

============================================================================
