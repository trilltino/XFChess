# Game costs and live measurements

```powershell
cargo run -p cu_benchmark -- --estimate tools/cu_benchmark/scenarios/games.json
cargo run -p cu_benchmark -- --estimate workload.json --quote-rent --export costs.json
cargo run -p cu_benchmark --bin inspect -- <account-public-key>
```

`--estimate` runs without keys or transactions. `--quote-rent` reads
`getMinimumBalanceForRentExemption` for each allocated account size. Set
`SOLANA_RPC_URL` for the target cluster and `ER_RPC_URL` for its MagicBlock endpoint.
No endpoint is required for an entirely offline estimate.

For Triton's account-specific priority price, add `--priority-percentile 5000
--writable-account <pubkey>` (repeat for each writable account). This uses Triton's
percentile extension, then takes the median across returned slots and applies that
price to every modeled transaction. Use separate workloads for unrelated write
sets and an endpoint that implements Triton's percentile extension. The JSON
records the samples and final inputs; RPC errors or empty samples fail without
substituting a price.

## Workloads

Each variant supplies a game count, transaction batches, allocated account sizes,
and delegation deposits/commit counts. Counts apply per game unless `shared` is
true; shared setup is charged once across the variant. Accounts already present
are omitted. Account closures refund only that account's specified rent.

The example compares settlement-only commits, checkpoints every ten plies, and
shared shard allocation. Its game (365), session (92), and shard (4765) byte sizes
come from the current program layouts, including the discriminator. Rent inputs
illustrate Solana's default rent schedule; the 6,000,000-lamport delegation
deposit is a scenario assumption, not a queried deposit. Transaction budgets are explicit
assumptions, not simulated CU measurements. Existing profiles are assumed; escrow,
new profiles, session funding transfers and other setup must be added for a full
lifecycle budget. Use allocated capacity, not serialized used bytes, for rent.

Outputs distinguish nonrefundable fees, retained storage, deposits/refunds, and
funding before refunds. Funding is cumulative, not peak simultaneous capital.
Money uses integer lamports; only amortized per-game presentation uses decimals.
Normal ER transaction fees are currently zero. More moves change ER load;
checkpoint frequency changes per-account commit charges. Include validator
auto-commits in the successful normal-commit count; moves are not commits. Model each delegated
account separately when commit histories differ. Base Action CU lists describe
one bundle; the sum is rounded once. Include Solana fees for app-paid top-ups,
but exclude validator-paid commits to avoid charging operator costs to the app.
Wagers, prizes and RPC subscriptions are separate cashflow/operating costs.

## Fee provenance

Reviewed 2026-10-10 against [Solana fees](https://solana.com/docs/core/fees),
[Triton priority quotes](https://docs.triton.one/chains/solana/improved-priority-fees-api),
and [MagicBlock fee economics](https://docs.magicblock.gg/pages/ephemeral-rollups-ers/introduction/fees-and-commit-economics).

- `current`: session 3,000,000; subsequent commits 1,000,000 lamports. Source:
  [delegation-program 3a6fbe2](https://github.com/magicblock-labs/delegation-program/blob/3a6fbe210530ab912257aa485626f387e294748a/dlp-api/src/consts.rs).
- `legacy`: session 300,000; subsequent commits 100,000 lamports. Source:
  [6898ef4](https://github.com/magicblock-labs/delegation-program/blob/6898ef4b82ba1f2b6fbb5d91eca578729edbbeb8/dlp-api/src/consts.rs).
- Runtime assumptions: [magicblock-validator b38979d](https://github.com/magicblock-labs/magicblock-validator/tree/b38979db27715fdff7346b4bcf0829d5158f96d9):
  ten normal commits without a delegated payer; 100,000 lamports per account from
  commit 26 with one; Base Actions 50,000 micro-lamports/CU; callbacks 5,000;
  ephemeral storage `(bytes + 60) * 32`. Top-up setup is separately 300,000.

MagicBlock's docs link an older delegation revision despite describing current
prices. Select the profile matching your deployment; these profiles do not detect
cluster versions. Legacy changes deposit rates only and retains the reviewed
runtime assumptions. Deposit charges are capped at the record + metadata balance.
The final commit-and-undelegate counts as a commit. The application's 300,000
reimbursement constant is contract accounting, not evidence of provider pricing.

## Live measurements

```powershell
cargo run -p cu_benchmark -- --init
cargo run -p cu_benchmark -- --live --mode 1v1 --export measured
cargo run -p cu_benchmark -- --live --mode swiss --size 16
```

Live modes sign and send transactions using the existing `keys/` configuration.
Reports fetch actual `meta.fee`, CU, requested budget and payer balances from each
recorded signature. Missing metadata remains unknown and coverage is reported;
failed landed transactions retain their fees. Payer balance deltas include wagers,
rent and transfers, so they are not a game-fee total. Wallet provisioning,
reclamation and unrecorded failures are outside report coverage.

Swiss and elimination flows exercise synthetic result reporting, not full ER
chess games for every pairing. Use workload variants to price those games too.
See [RPC probes](rpc.md) for infrastructure measurements.
