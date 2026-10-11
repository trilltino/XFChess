------------------------------ MODULE SolanaFinality ------------------------------

EXTENDS Naturals, Sequences

CONSTANTS Agents, Outsiders, Authorized, Submitters, MaxNonce, MaxContent,
          Byzantine, SubmitCap, EnforceNonce, EnforceAuth

Authors == Agents \cup Outsiders

VARIABLES chain_nonce, chain_log, mempool, submits

vars == <<chain_nonce, chain_log, mempool, submits>>

Tx == [ target:    1..MaxNonce,
        parentSet: BOOLEAN,
        parent:    0..MaxNonce,
        content:   1..MaxContent,
        author:    Authors,
        submitter: Submitters ]

Init ==
    /\ chain_nonce = 0
    /\ chain_log   = << >>
    /\ mempool     = {}
    /\ submits     = 0

SubmitHonest(s, p) ==
    /\ submits < SubmitCap
    /\ LET t == [ target |-> chain_nonce + 1, parentSet |-> TRUE,
                  parent |-> chain_nonce,
                  content |-> ((chain_nonce + 1) % MaxContent) + 1,
                  author |-> p, submitter |-> s ]
       IN /\ chain_nonce + 1 <= MaxNonce
          /\ mempool' = mempool \cup {t}
    /\ submits' = submits + 1
    /\ UNCHANGED <<chain_nonce, chain_log>>

SubmitByz(p) ==
    /\ p \in Byzantine
    /\ submits < SubmitCap
    /\ \E t \in Tx : t.author = p /\ mempool' = mempool \cup {t}
    /\ submits' = submits + 1
    /\ UNCHANGED <<chain_nonce, chain_log>>

SubmitOutsider(p) ==
    /\ p \in Outsiders
    /\ submits < SubmitCap
    /\ \E t \in Tx : t.author = p /\ mempool' = mempool \cup {t}
    /\ submits' = submits + 1
    /\ UNCHANGED <<chain_nonce, chain_log>>

ApplyTx(t) ==
    /\ t \in mempool
    /\ (EnforceAuth  => t.author \in Authorized)
    /\ (EnforceNonce => t.target = chain_nonce + 1)
    /\ (t.parentSet => t.parent = chain_nonce)
    /\ chain_nonce' = t.target
    /\ chain_log'   = Append(chain_log, t)
    /\ mempool'     = mempool \ {t}
    /\ UNCHANGED submits

Next ==
    \/ \E s \in Submitters, p \in Agents : SubmitHonest(s, p)
    \/ \E p \in Agents : SubmitByz(p)
    \/ \E p \in Outsiders : SubmitOutsider(p)
    \/ \E t \in mempool : ApplyTx(t)

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ chain_nonce \in 0..MaxNonce
    /\ mempool \subseteq Tx
    /\ submits \in 0..SubmitCap
    /\ \A i \in 1..Len(chain_log) : chain_log[i] \in Tx

ChainLinear ==
    \A i \in 1..Len(chain_log) : chain_log[i].target = i

ChainParentConsistent ==
    \A i \in 1..Len(chain_log) :
        chain_log[i].parentSet => (chain_log[i].parent = i - 1)

ChainLinearizable == ChainLinear /\ ChainParentConsistent

OnlyAuthorizedCommitted ==
    \A i \in 1..Len(chain_log) : chain_log[i].author \in Authorized

============================================================================
