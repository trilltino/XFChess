# Game Settlement Runbook

Use this flow when investigating a stuck or disputed game payout.

1. Confirm the `Game` account has the expected phase and result.
2. If the game is delegated, undelegate before base-layer settlement.
3. For resign or timeout, verify the instruction only set `GameStatus::Finished` and `GameResult`.
4. Run final settlement from the base layer so wager escrow payout, fee reimbursement, and profile updates use one path.
5. For disputes, confirm the resolver selected either draw or one of the game players.
6. Check the wager escrow balance against `common::escrow::pot(game.wager_amount)`.

Never patch a payout by adding a one-off lamport transfer to an instruction handler.
