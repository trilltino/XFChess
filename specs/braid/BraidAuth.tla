---------------------------- MODULE BraidAuth ----------------------------
EXTENDS Naturals, Sequences

CONSTANTS Agents, Outsiders, MaxSeq, AuthCheck
VARIABLE accepted
vars == <<accepted>>

Init == accepted = <<>>

Put(p, parent) ==
    /\ Len(accepted) < MaxSeq
    /\ (AuthCheck => p \in Agents)
    /\ parent = Len(accepted)
    /\ LET n == Len(accepted) + 1
       IN accepted' = Append(accepted,
           [sender |-> p, version |-> n, parent |-> parent])

Next == \E p \in Agents \cup Outsiders, parent \in 0..MaxSeq : Put(p, parent)
Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ Len(accepted) <= MaxSeq
    /\ \A i \in 1..Len(accepted) :
        accepted[i] \in [sender: Agents \cup Outsiders,
                        version: 1..MaxSeq, parent: 0..MaxSeq]

OnlyParticipantsAccepted ==
    \A i \in 1..Len(accepted) : accepted[i].sender \in Agents

ParentLinked ==
    \A i \in 1..Len(accepted) :
        /\ accepted[i].version = i
        /\ accepted[i].parent = i - 1
=============================================================================
