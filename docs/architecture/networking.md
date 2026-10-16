# Multiplayer synchronization

Iroh provides authenticated peer transport. Braid carries versioned resources
over peer connections and HTTP subscriptions, including the VPS fallback log.
The chess engine decides whether a delivered move can be applied.

| State | Authority |
| --- | --- |
| Casual participants and recovery history | Accepted identities and durable backend log; clients validate legal application |
| Wagered position during delegation | Delegated Game account on the ER |
| Wagered result/money after commit and undelegation | Base Game state and program settlement |
| Peer messages and HTTP game events | Delivery/replay evidence, never independent settlement authority |

The backend persists game events in SQLite. Version/Parents and per-sender causal
lanes detect duplicates, missing parents and conflicting writes. Resume uses the
acknowledged application checkpoint: receipt must not advance past a position
the engine has not accepted. Pending writes remain recoverable until acknowledged.

A restarted wager client restores from authoritative account state and a legally
replayed log. It refuses incompatible identities, ended games or insufficient
history. Board, causal lane, nonce and rollup baseline must agree. Seat leases
allow device replacement while blocking writes from the superseded device.
Spectators have separate recovery and delay rules.

## Failure handling

- A gap triggers catch-up; a duplicate does not apply twice.
- Rejected application keeps the checkpoint behind that event.
- Backend restart preserves accepted participants, relay room state and history.
- A transaction timeout is unknown until signature/account reconciliation.
- Delegated terminal games commit and undelegate before base settlement.
- Payout requires chain evidence; HTTP success or a peer result is insufficient.

Implementation: [transport](../../src/multiplayer/network/braid_transport.rs),
[journal](../../libs/zarathustra_net/braid_chess/src/journal.rs),
[game log](../../backend/src/signing/routes/game_log.rs),
[resume](../../src/multiplayer/resume.rs),
[board application](../../src/game/systems/network_move.rs).
[Protocol models](../../specs/) cover bounded invariants;
[verification](../development/verification.md) covers live failure drills.
