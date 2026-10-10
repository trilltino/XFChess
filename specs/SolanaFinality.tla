------------------------------ MODULE SolanaFinality ------------------------------
(* Model on-chain record_move under mempool reordering and Byzantine submissions. *)
(* Strict nonce checks guarantee a gap-free log; parent_nonce checks preserve parent consistency. *)
EXTENDS Naturals, Sequences

CONSTANTS
    Agents,        \* submitting players, e.g. {A, B}
    Outsiders,     \* authors with NO registered session key (forging nodes);
                   \* {} in configs where impersonation is not exercised
    Authorized,    \* authors whose session key is registered for this game.
                   \* Models the session_delegation roster record_move enforces.
    MaxNonce,      \* bound on committed moves
    MaxContent,    \* bound on distinct contents (allows conflicting moves)
    Byzantine,     \* players that may submit arbitrary transactions
    SubmitCap,     \* bound on outstanding submissions (keeps model finite)
    EnforceNonce,  \* TRUE = real code; FALSE = nonce check removed (necessity)
    EnforceAuth    \* TRUE requires an enabled registered session key; FALSE disables authorization.

Authors == Agents \cup Outsiders

VARIABLES
    chain_nonce,   \* Nat        on-chain game.nonce
    chain_log,     \* Seq(Tx)    committed move history
    mempool,       \* SUBSET Tx  submitted-but-not-yet-applied transactions
    submits        \* Nat        number of submissions so far (bound)

vars == <<chain_nonce, chain_log, mempool, submits>>

----------------------------------------------------------------------------
NONE == 0   \* sentinel: parentSet = FALSE means parent_nonce == None

Tx == [ target:    1..MaxNonce,
        parentSet: BOOLEAN,
        parent:    0..MaxNonce,
        content:   1..MaxContent,
        author:    Authors ]

Init ==
    /\ chain_nonce = 0
    /\ chain_log   = << >>
    /\ mempool     = {}
    /\ submits     = 0

----------------------------------------------------------------------------
(* Honest submissions use the next nonce, current parent, and deterministic content. *)
SubmitHonest(p) ==
    /\ submits < SubmitCap
    /\ LET t == [ target |-> chain_nonce + 1, parentSet |-> TRUE,
                  parent |-> chain_nonce,
                  content |-> ((chain_nonce + 1) % MaxContent) + 1,
                  author |-> p ]
       IN /\ chain_nonce + 1 <= MaxNonce
          /\ mempool' = mempool \cup {t}
    /\ submits' = submits + 1
    /\ UNCHANGED <<chain_nonce, chain_log>>

(* Byzantine players may submit arbitrary nonces, parents, and content. *)
SubmitByz(p) ==
    /\ p \in Byzantine
    /\ submits < SubmitCap
    /\ \E t \in Tx : t.author = p /\ mempool' = mempool \cup {t}
    /\ submits' = submits + 1
    /\ UNCHANGED <<chain_nonce, chain_log>>

(* Outsiders lack registered session keys and must fail roster authorization. *)
SubmitOutsider(p) ==
    /\ p \in Outsiders
    /\ submits < SubmitCap
    /\ \E t \in Tx : t.author = p /\ mempool' = mempool \cup {t}
    /\ submits' = submits + 1
    /\ UNCHANGED <<chain_nonce, chain_log>>

ApplyTx(t) ==
    /\ t \in mempool
    /\ (EnforceAuth  => t.author \in Authorized)           \* InvalidSessionKey / roster
    /\ (EnforceNonce => t.target = chain_nonce + 1)        \* InvalidNonce check
    /\ (t.parentSet => t.parent = chain_nonce)             \* ParentNonceMismatch
    /\ chain_nonce' = t.target
    /\ chain_log'   = Append(chain_log, t)
    /\ mempool'     = mempool \ {t}
    /\ UNCHANGED submits

Next ==
    \/ \E p \in Agents : SubmitHonest(p)
    \/ \E p \in Agents : SubmitByz(p)
    \/ \E p \in Outsiders : SubmitOutsider(p)
    \/ \E t \in mempool : ApplyTx(t)

Spec == Init /\ [][Next]_vars

----------------------------------------------------------------------------
TypeOK ==
    /\ chain_nonce \in 0..MaxNonce
    /\ mempool \subseteq Tx
    /\ submits \in 0..SubmitCap
    /\ \A i \in 1..Len(chain_log) : chain_log[i] \in Tx

(* Committed nonces are contiguous and unique, starting at 1. *)
ChainLinear ==
    \A i \in 1..Len(chain_log) : chain_log[i].target = i

(* A supplied parent must equal the immediately preceding committed nonce. *)
ChainParentConsistent ==
    \A i \in 2..Len(chain_log) :
        chain_log[i].parentSet => (chain_log[i].parent = i - 1)

ChainLinearizable == ChainLinear /\ ChainParentConsistent

(* Only authors with registered session keys may commit moves; requires EnforceAuth. *)
OnlyAuthorizedCommitted ==
    \A i \in 1..Len(chain_log) : chain_log[i].author \in Authorized

============================================================================
