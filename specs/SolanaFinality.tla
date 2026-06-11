------------------------------ MODULE SolanaFinality ------------------------------
(***************************************************************************)
(* Phase 4: the on-chain settlement layer.                                 *)
(*                                                                         *)
(* Models record_move in the Anchor program:                              *)
(*   programs/xfchess-game/src/moves_ix/record.rs                          *)
(*                                                                         *)
(*   if let Some(pn) = parent_nonce {                                      *)
(*       require!(pn == game.nonce, ParentNonceMismatch);                  *)
(*   }                                                                     *)
(*   require!(nonce == game.nonce + 1, InvalidNonce);                      *)
(*   game.nonce = nonce;                                                   *)
(*                                                                         *)
(* The mempool may reorder transactions and Byzantine players may submit   *)
(* conflicting ones. We check that the committed chain is always a single  *)
(* linear, gap-free history (linearizable), and that the parent_nonce      *)
(* check keeps committed parents consistent.                               *)
(*                                                                         *)
(* The model also answers the open question from the plan: is parent_nonce *)
(* REQUIRED for linearizability, or does the strict nonce check carry it?  *)
(* Setting EnforceNonce = FALSE removes the `nonce == game.nonce + 1`      *)
(* check and shows the chain immediately forks -- i.e. the NONCE check is   *)
(* the load-bearing guarantee; parent_nonce is defense-in-depth.           *)
(***************************************************************************)
EXTENDS Naturals, Sequences

CONSTANTS
    Agents,        \* submitting players, e.g. {A, B}
    MaxNonce,      \* bound on committed moves
    MaxContent,    \* bound on distinct contents (allows conflicting moves)
    Byzantine,     \* players that may submit arbitrary transactions
    SubmitCap,     \* bound on outstanding submissions (keeps model finite)
    EnforceNonce   \* TRUE = real code; FALSE = nonce check removed (necessity)

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
        author:    Agents ]

Init ==
    /\ chain_nonce = 0
    /\ chain_log   = << >>
    /\ mempool     = {}
    /\ submits     = 0

----------------------------------------------------------------------------
(* An honest player submits its next legitimate move: target = next nonce,  *)
(* parent_nonce = current chain nonce, deterministic content.               *)
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

(* A Byzantine player submits an arbitrary transaction: any target, any     *)
(* parent, any content -- including conflicting moves and forged parents.   *)
SubmitByz(p) ==
    /\ p \in Byzantine
    /\ submits < SubmitCap
    /\ \E t \in Tx : t.author = p /\ mempool' = mempool \cup {t}
    /\ submits' = submits + 1
    /\ UNCHANGED <<chain_nonce, chain_log>>

(* The runtime applies a transaction. This is record_move's guard set.      *)
ApplyTx(t) ==
    /\ t \in mempool
    /\ (EnforceNonce => t.target = chain_nonce + 1)        \* InvalidNonce check
    /\ (t.parentSet => t.parent = chain_nonce)             \* ParentNonceMismatch
    /\ chain_nonce' = t.target
    /\ chain_log'   = Append(chain_log, t)
    /\ mempool'     = mempool \ {t}
    /\ UNCHANGED submits

Next ==
    \/ \E p \in Agents : SubmitHonest(p)
    \/ \E p \in Agents : SubmitByz(p)
    \/ \E t \in mempool : ApplyTx(t)

Spec == Init /\ [][Next]_vars

----------------------------------------------------------------------------
TypeOK ==
    /\ chain_nonce \in 0..MaxNonce
    /\ mempool \subseteq Tx
    /\ submits \in 0..SubmitCap
    /\ \A i \in 1..Len(chain_log) : chain_log[i] \in Tx

(* Linearizable: the committed history is exactly nonce 1, 2, 3, ... with   *)
(* no gaps and no two moves at the same nonce.                              *)
ChainLinear ==
    \A i \in 1..Len(chain_log) : chain_log[i].target = i

(* Every committed move that supplied a parent_nonce named the immediately   *)
(* preceding committed nonce.                                               *)
ChainParentConsistent ==
    \A i \in 2..Len(chain_log) :
        chain_log[i].parentSet => (chain_log[i].parent = i - 1)

ChainLinearizable == ChainLinear /\ ChainParentConsistent

============================================================================
