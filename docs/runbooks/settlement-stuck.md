# Wager settlement stuck

A terminal result without a confirmed payout needs account and transaction
evidence, not a repeated HTTP request.

## Diagnose

1. Identify game ID, players, network, Game PDA, escrow and submitted signatures.
2. Inspect settlement logs, [RPC health](rpc-degraded.md) and fee-payer funding.
3. Read the Game owner/phase/result. Delegated games must commit and undelegate
   before base settlement; follow [the lifecycle](magicblock-lifecycle-devnet.md).
4. Inspect `/api/debug/tx/{signature}` and current balances for unknown outcomes.
5. Confirm the deployed program matches the caller's instruction/account shape.

## Missing Game PDA

`finalize_game` closes the Game account. Missing data alone does not prove payment.
The worker retires an older missing-account session only after its escrow check;
RPC errors do not establish absence.

`game N: Game PDA missing but escrow still holds X lamports - operator recovery required`
identifies funds requiring investigation. Preserve escrow signature history and
find the last finalize/cancel transaction. Escalate contradictory data; do not
transfer escrow manually or fabricate a winner.

## Recover and verify

Restore provider connectivity/funding and allow idempotent worker reconciliation.
Privileged recovery must follow the deployed program's authority/lifecycle
constraints. Confirm payout/refund signatures and balances, no duplicate rating
update and coherent session retirement. See [settlement invariants](game-settlement.md)
and [verification](../development/verification.md).
