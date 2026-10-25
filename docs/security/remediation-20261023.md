# Contract remediation record — 23 October 2026

## Starting state and scope

Source of truth: `trilltino/XFChess`, branch `main`, HEAD
`70a25c5c4e2d2fef18bb30f54f4260a74e27fadb`. The working tree is dirty,
including existing changes to contract code and tests; these must be preserved.
Extraction main is `25c1b31b1e8d2692189b795e694caac8a3959658`; the review
branch currently points to `ceb3f540cdc856ff028b3b108683f315c12f448a`.
This work patches the source of truth, not the extraction or a deployed binary.
At the initial audit stage, deployment and authority changes were not authorized.
The owner subsequently authorized devnet deployment, a contracts main push and
an XFChess PR; no mainnet deployment or authority rotation was authorized.

## Pre-edit failure hypotheses

1. **Profile substitution (confirmed in source):** an ordinary player signs
   `init_profile`, supplying another writable XFChess-owned Game as the profile.
   The nonempty branch checks ownership but not its address; the profile loader
   defaults on discriminator mismatch, allowing resize/overwrite. Add the canonical
   wallet-derived profile constraint before any initialization or write.
2. **Recovery redirection (confirmed, privileged):** the real dispute authority
   signs recovery for an empty program-owned Game PDA, supplying substitute
   system-owned recipients. Current checks establish neither original player;
   all escrow can be split to these destinations, followed by rent movement.
   Ordinary attackers cannot satisfy the authority constraint. Without verified
   identities, reject before all payments, rent movement, and success events.
3. **Callback scope (confirmed missing app guard):** a callback caller supplies
   non-Game seeds and matching PDA. The SDK authenticates its buffer and effective
   PDA signature, but the application does not restrict the restored account class.
   Accept exactly `game` plus an eight-byte game ID and the matching program PDA.
   A complete exploit still requires a valid delegation-program-signed buffer;
   do not claim arbitrary unsigned reconstruction.
4. **Callback accounts:** Anchor does not pin the System Program or canonical
   buffer. SDK checks already mitigate buffer spoofing; add equivalent early
   constraints, preserve payer semantics, and test failure atomicity.
5. **Wager expiry:** `created_at + 86400` can overflow for damaged/committed state.
   Normal creation uses Clock; no unprivileged writer of extreme timestamps is
   established. Return ArithmeticOverflow before escrow/status mutation, retaining
   the strict `now > expiry` boundary.
6. **Global-session reset is policy, not a defect:** wallet-signed reauthorization
   deliberately resets spending. Test that active authorizations cannot reset,
   wallet signatures are mandatory, and rotated keys cannot spend.

## Binding owner decisions

> Emergency recovery must pay only the original verified players.

> Global-session spending caps are per wallet-signed authorization.

No new account schema, historical backfill, ABI migration, key rotation, or
network deployment will be implemented without the approval required by the
execution prompt. Failing closed on empty legacy Game data requires none of
those extensions.

## Follow-up hypothesis before changes

**Mixed-pool batch distribution (23 October):** an ordinary cranker can invoke
`distribute_tournament_prizes` for a Completed tournament with both SOL and USDC
funded. That handler transfers only SOL, then sets the shared `prizes_claimed`
place bit. `claim_tournament_prize` checks the same bit before either pool's
payout, so the legitimate winner can no longer claim their USDC. No privileged
signature or forged recipient is needed; the cranker supplies the real winners.
Reproduce with the actual SBF handler before changing it. The minimal fix is to
reject this SOL-only batch route when a USDC pool exists; individual claims
already pay both pools atomically. This needs no account/schema/fee change.
