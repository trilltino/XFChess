----------------------------- MODULE Delivery -----------------------------
EXTENDS Naturals, Sequences

CONSTANTS MaxVersion, DedupPending, AckOnDelivery
VARIABLES gossip, braid, pending, board, savedBoard, checkpoint, cursor
vars == <<gossip, braid, pending, board, savedBoard, checkpoint, cursor>>

Versions == 1..MaxVersion
Members(s) == {s[i] : i \in 1..Len(s)}
Without(s, v) == SelectSeq(s, LAMBDA x : x # v)

Init ==
    /\ gossip = Versions
    /\ braid = Versions
    /\ pending = <<>>
    /\ board = <<>>
    /\ savedBoard = <<>>
    /\ checkpoint = 0
    /\ cursor = 0

\* Reservation and board application are separate steps.
Deliver(v, isGossip) ==
    /\ v \in (IF isGossip THEN gossip ELSE braid)
    /\ pending' = IF v \in Members(board)
                     \/ (DedupPending /\ v \in Members(pending))
                  THEN pending ELSE Append(pending, v)
    /\ IF isGossip
          THEN /\ gossip' = gossip \ {v}
               /\ UNCHANGED braid
          ELSE /\ braid' = braid \ {v}
               /\ UNCHANGED gossip
    /\ cursor' = IF AckOnDelivery /\ ~isGossip THEN v ELSE cursor
    /\ UNCHANGED <<board, savedBoard, checkpoint>>

Apply(i) ==
    /\ i \in 1..Len(pending)
    /\ pending[i] <= Len(board) + 1
    /\ board' = Append(board, pending[i])
    /\ pending' = SubSeq(pending, 1, i - 1)
                  \o SubSeq(pending, i + 1, Len(pending))
    /\ UNCHANGED <<gossip, braid, savedBoard, checkpoint, cursor>>

Reject(v) ==
    /\ v \in Members(pending)
    /\ pending' = Without(pending, v)
    /\ UNCHANGED <<gossip, braid, board, savedBoard, checkpoint, cursor>>

Persist ==
    /\ board # savedBoard
    /\ savedBoard' = board
    /\ checkpoint' = Len(board)
    /\ cursor' = Len(board)
    /\ UNCHANGED <<gossip, braid, pending, board>>

Replay ==
    /\ gossip' = Versions
    /\ braid' = {v \in Versions : v > cursor}
    /\ UNCHANGED <<pending, board, savedBoard, checkpoint, cursor>>

\* A fresh process does not seed subscription cursors from the local journal.
Restart ==
    /\ board' = <<>>
    /\ pending' = <<>>
    /\ gossip' = Versions
    /\ braid' = Versions
    /\ cursor' = 0
    /\ UNCHANGED <<savedBoard, checkpoint>>

Next ==
    \/ \E v \in gossip : Deliver(v, TRUE)
    \/ \E v \in braid : Deliver(v, FALSE)
    \/ \E i \in 1..Len(pending) : Apply(i)
    \/ \E v \in Members(pending) : Reject(v)
    \/ Persist
    \/ Replay
    \/ Restart

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ gossip \subseteq Versions
    /\ braid \subseteq Versions
    /\ pending \in Seq(Versions)
    /\ board \in Seq(Versions)
    /\ savedBoard \in Seq(Versions)
    /\ checkpoint \in 0..MaxVersion
    /\ cursor \in 0..MaxVersion

NoDoubleApply ==
    \A i, j \in 1..Len(board) : board[i] = board[j] => i = j

BoardOrdered == \A i \in 1..Len(board) : board[i] = i

CheckpointMatchesBoard ==
    /\ checkpoint = Len(savedBoard)
    /\ \A i \in 1..Len(savedBoard) : savedBoard[i] = i

ResumeAfterApplication ==
    /\ cursor <= Len(board)
    /\ cursor <= checkpoint
=============================================================================
