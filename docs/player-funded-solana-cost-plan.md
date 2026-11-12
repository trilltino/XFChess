# Player-funded Solana costs and transaction review plan

## Objective

Make XFChess economical at 100,000 players and ensure every wallet-funded action is reviewed before signing. Charges must be classified consistently as an actual network fee, an XFChess service fee, a refundable deposit, a conditional stake/entry/bond, or a future delegated-spend authorization.

This plan targets a fresh `xfchess-game` v2 program ID. Existing devnet test accounts are not migrated, and no mainnet deployment exists to migrate.

## Decisions

1. Ordinary accounts, usernames, friends and casual games remain off-chain and free.
2. Create a player PDA lazily, only when a player first uses a ranked on-chain, wagered or tournament feature.
3. Replace the current profile and username records with one fixed-size player PDA of no more than 64 bytes including its discriminator.
4. The player pays their own profile rent and network fee. The PDA must have an implemented, tested close path before its rent is described as refundable.
5. Stakes never fund network costs or XFChess operating expenses. A two-player game escrow always contains exactly `2 * stake` before settlement.
6. Service fees are explicit transfers, capped on-chain, and shown separately from stakes.
7. Use one transaction-review envelope for every wallet and session authorization flow. Free-form signing labels are not a security boundary.
8. Remove automatic wallet invocation. XFChess must show the review and receive an explicit confirmation before calling Phantom, Solflare or Privy.
9. Tournament infrastructure, KYC verification, external-rating linking, cranks and administrative actions remain operator-funded.
10. Replace the fixed global-session funding amounts with player-selected, fully bounded authorization.

## Current player exposure

| Action | What the player currently funds or risks | Target policy |
|---|---|---|
| Profile creation | 289-byte profile rent, 56-byte username rent and network fee | One <=64-byte PDA, paid by player and closable |
| Username change | Another permanent username PDA | Backend-only username; no on-chain charge |
| Friendship request | Friendship PDA rent and network fee | Backend-only social graph |
| Create wagered game | Stake; sometimes game/recovery/session rent; network fee | Stake + disclosed service fee + exact network fee; temporary deposits itemized and returned |
| Join wagered game | Matching stake and network fee | Stake + disclosed service fee + exact network fee |
| Game settlement | Hard-coded reimbursements, accumulated pseudo-fees and platform/country deductions from the pot | Preserve full stake pot; service fee collected separately |
| Per-game actions | Network fees or funded session-key spending | Quote direct actions; bound delegated actions by the reviewed session budget |
| Global session | PDA rent, fixed 0.1 SOL vault deposit and 0.01 SOL session-key transfer | Exact rent + player-chosen spend/fee budget; close and return remainder |
| Per-game/tournament session | Account rent, deposit and network fee | Unified bounded-session model with close/refund |
| Tournament registration | Entry fee, possible pass burn and network fee | Show prize contribution, organizer/XFChess fee, consumed pass and network fee separately |
| Dispute | Dispute-record rent, 0.01 SOL bond and network fee | Keep bond, rent and network fee separate; close record after resolution |
| Prize claim/leave/revoke/close | Network fee and possibly recipient ATA rent | Quote actual network fee and any new token-account deposit |

The player may also pay ordinary network fees for resign, draw, timeout, cancellation, withdrawal and recovery transactions. These actions should not gain invented protocol surcharges.

## Economics that must be corrected

- `CREATE_GAME_COST`, `JOIN_GAME_COST`, `DELEGATE_COST`, `UNDELEGATE_COST`, `RECORD_RESULT_COST`, `ER_SESSION_FEE_LAMPORTS`, `CLAIM_PRIZE_COST` and `ELO_FEE_LAMPORTS` are fixed constants, not actual Solana fees (`programs/xfchess-game/src/constants.rs`). Remove them from player settlement accounting.
- `lifecycle/settlement.rs` currently deducts a hard-coded 10,000 lamports, accumulated `fees_advanced`, and `country_fee` from wager escrow. Replace this with a full stake payout and separately collected service fees.
- Rename `country_fee` to `service_fee_lamports`. It is a caller-supplied platform charge, not a country-specific network cost.
- Tournament start currently transfers all entry fees to the host treasury and calls the result `platform_fee_pool`. Store and enforce an explicit prize contribution and organizer/XFChess fee split.
- The global session authorizes defaults larger than its actual funding and accounts only for wager spending. Every debit category must be capped and counted.
- `UsernameRecord::LEN` already includes a discriminator but profile creation adds another eight bytes. Eliminate the record in v2 and fix remaining `LEN`/`INIT_SPACE` conventions.
- Session allocation using `size_of::<SessionDelegation>()` must become `SessionDelegation::INIT_SPACE`; Rust layout is not the serialized Anchor layout.

## Target on-chain player state

Replace `programs/xfchess-game/src/state/player_profile.rs` with a versioned, fixed-width structure no larger than 64 bytes total. Retain only fields required for program enforcement, such as:

- version and bump;
- fixed-point rating buckets;
- essential ranked counters;
- last activity or rating period;
- active-obligation counters needed to guard closure.

Do not put country, exact date of birth, KYC state, usernames, Lichess identity, fiat totals, annual winnings, total wagering or analytics on-chain. Store them in the backend identity/compliance stores with appropriate retention and deletion controls.

Replace `f64` ratings with deterministic fixed-point integers. Re-activation after profile closure must require an authorized rating snapshot/attestation so closing cannot reset a rating. `close_profile` is allowed only when all on-chain obligation counters are zero and no live session is supplied; it emits a final snapshot and returns rent to the player.

At current rent rates, a 64-byte account requires about 0.00097536 SOL. At 100,000 profiles this is about 97.536 SOL, funded across the players rather than XFChess.

## Shared transaction-review contract

Add serializable types under `crates/xfchess-platform-api/src/`:

```text
TransactionEnvelope
  transaction bytes
  TransactionReview

TransactionReview
  quote_id
  action
  cluster
  program_id
  payer
  message_digest
  blockhash and last_valid_block_height
  immediate_debit_lamports
  network_fee_lamports
  service_fees[]
  refundable_deposits[]
  conditional_stakes[]
  expected_receipts[]
  future_authorization
  warnings[]
```

Supporting enums/records should include `WalletAction`, `ChargeKind`, `RefundCondition`, `DepositLine`, `ConditionalLine`, `ReceiptLine` and `AuthorizationScope`. Store integer lamports; SOL and indicative fiat formatting belong in the UI.

Required invariants:

- The review is bound to the exact serialized message by its digest.
- All decoded wallet debits reconcile to the displayed maximum.
- The payer, cluster and program IDs are explicit.
- A changed message or refreshed blockhash invalidates the review.
- Every deposit names its refund recipient and closure condition.
- Every session shows the maximum possible future debit, not only the immediate funding transaction.
- No wallet call occurs before an explicit XFChess confirmation.

## Quote construction

Create a shared analyzer in `crates/xfchess-client-solana` and use the same economic model from Bevy and the backend:

1. Fetch a finalized blockhash and last valid block height.
2. Compile the complete legacy or v0 message.
3. Query `getFeeForMessage` for the actual network fee.
4. Query account existence and ownership.
5. Query `getMinimumBalanceForRentExemption` for accounts that will be created.
6. Decode System transfers and every supported XFChess instruction.
7. Simulate the exact transaction.
8. Classify all debits, deposits, conditional funds, receipts and delegated limits.
9. Hash the serialized message and return a `TransactionEnvelope`.

Never hard-code rent or a 5,000-lamport fee. If a quote expires, construct and show a new envelope before signing. Do not silently refresh the profile transaction blockhash after review.

## Tauri review flow

Update:

- `crates/xfchess-platform-api/src/lib.rs`
- `crates/xfchess-client-bevy/src/multiplayer/solana/tauri_signer.rs`
- `apps/tauri/src/wallet/mod.rs`
- `apps/tauri/src/wallet/http.rs`
- `apps/tauri/wallet-ui/src/wallet/TransactionSigner.tsx`
- `apps/tauri/wallet-ui/src/App.tsx`

Replace `{tx, label}` with the verified transaction envelope. Remove the automatic signing effect in `TransactionSigner.tsx`. The signing screen must show:

- action, cluster, program and wallet;
- stake, entry fee or bond;
- XFChess/organizer service fee;
- each refundable account/session deposit;
- actual estimated network fee;
- maximum immediate wallet debit;
- maximum future delegated spend;
- refund/forfeit/closure conditions;
- quote expiry and important warnings.

Buttons are `Cancel` and `Review and sign`. Only the latter invokes the wallet. After signing, verify message integrity, submit and display a receipt using the same categories.

For authentication messages, show the exact domain, purpose, nonce and expiry plus “No SOL can be transferred.” Do not auto-invoke the wallet for messages.

## Wallet-native purpose and readable transaction previews

The XFChess review is authoritative because Solana wallet signing APIs do not provide a universal trusted `purpose` field. Phantom, Solflare and Privy may render the same custom-program transaction differently, and XFChess cannot force a particular native-wallet title. Improve their previews with these additional layers:

1. Add one short SPL Memo instruction to every user-facing transaction. Use a stable, non-sensitive format such as:

   ```text
   XFChess: Create player profile
   XFChess: Join wagered game
   XFChess: Enter tournament
   XFChess: Open dispute
   ```

   Memos are public. Never include DOB, country, KYC status, email, legal name, private evidence, authentication tokens or other personal data. Keep the memo short because it consumes transaction space and is only a display hint, not an authorization or security control.

2. Keep transactions single-purpose wherever practical. Do not bundle unrelated profile creation, broad session authorization, wallet funding and game creation merely to reduce prompts. If atomicity requires multiple instructions, the memo and review must describe the complete combined effect.

3. Publish the v2 Anchor IDL and keep public instruction names stable so wallets, explorers and security simulators can decode XFChess instructions. Treat any IDL or program-ID change as a review-schema version change.

4. Register the production application and new program ID with wallet metadata systems:
   - verify the XFChess domain in Phantom Portal;
   - submit the XFChess display name, icon, domain, program ID and interaction names;
   - request equivalent program/app recognition from other supported wallets where available;
   - register both desktop callback origins and the public website only through their supported verification processes.

5. Preserve standard System Program, SPL Token and Token-2022 instructions for value movement rather than hiding transfers inside opaque custom instruction data. Wallet simulation can then identify balance changes independently of the XFChess memo.

6. Show the same action name in the Bevy/Tauri review, memo, backend audit record and receipt. Define canonical names in shared platform API data rather than duplicating prose across clients.

The resulting trust sequence is:

```text
XFChess cost review -> explicit confirmation -> native wallet preview -> signature -> XFChess receipt
```

The native wallet may still show raw accounts or instruction data when it does not recognize the program. This is acceptable only because the exact message digest and decoded economics were already reviewed in XFChess and are verified again before broadcast.

## Contract lifecycle changes

### Profiles and social state

- Remove `UsernameRecord` and `LichessUsernameRecord` from the v2 user lifecycle.
- Friendship state is off-chain and the contract no longer has friendship accounts or instructions (done in source; the IDL and ABI snapshot are updated by hand to match, regenerate them with the v2 build). Wallet friendships, blocks and portability are specified in [FRIENDS_ARCHITECTURE.md](FRIENDS_ARCHITECTURE.md).
- Add guarded profile close and rating reactivation.
- Remove production access to the sponsored-profile route.

### Games and wagers

- Collect both stakes into an isolated stake escrow.
- Collect each player's disclosed service-fee share separately.
- Remove `fees_advanced` and fixed pseudo-fee reimbursement.
- Record the original payer for every temporary account.
- Close Game, RecoveryIdentity and session/delegation accounts on settlement, cancellation, timeout and recovery terminal paths.
- Return rent to the recorded payer, not merely the caller who closes the account.
- Keep organizer/backend costs out of player stake escrow.

### Sessions

Unify global, tournament and per-game session policy around:

- short expiry, initially 24 hours;
- allowed-action bitmap;
- maximum wager per game;
- maximum cumulative stakes;
- maximum service fees;
- maximum account deposits;
- maximum network-fee budget;
- maximum games;
- vault funding sufficient for every authorized outflow.

Count every debit, including rent and service fees. Remove the fixed 0.1 SOL deposit and 0.01 SOL session-key funding. On revoke, expiry or exhaustion, close the session PDA, return remaining balance and rent, and delete the local session key.

### Tournaments

- Keep tournament, shard, match, pass-binding and prize-escrow creation operator-funded.
- Create shards lazily instead of preallocating four 4,765-byte accounts.
- Add deterministic close paths for tournament metadata, shards, matches, sessions and pass bindings after terminal settlement.
- Encode entry allocation as prize contribution plus organizer/XFChess service fee, and enforce the displayed total on-chain.
- Treat pass burn as a consumed asset and show it in the review.
- Show any claimant token-account creation as a refundable storage deposit.

### Disputes

- Show the bond, record rent and network fee independently.
- Preserve the current high-level bond policy only after product/legal review: return it when upheld/draw, forfeit it when dismissed.
- Close the dispute record after authority resolution or stale claim, returning rent to the challenger.
- A resolution service fee must be explicit; it must not be silently removed from the wager pot.

## Implementation sequence

### Phase 1: lock the economics specification

1. Define the five charge categories and payer/refund rules.
2. Decide service-fee caps and tournament entry allocation.
3. Decide profile reactivation and rating-restoration policy.
4. Decide dispute bond and abandonment rules with legal review.
5. Write invariants as contract tests before changing handlers.

### Phase 2: build the review envelope

1. Add platform API types.
2. Implement transaction decoding, RPC quoting and message binding.
3. Add canonical wallet-action names and sanitized SPL memos.
4. Extend the Tauri pending-request bridge.
5. Render the review and remove automatic signing.
6. Migrate profile creation first as the reference implementation.
7. Publish the development IDL and validate Phantom/Solflare/Privy preview behavior without treating it as guaranteed.

### Phase 3: introduce v2 account layouts

1. Implement the compact player PDA and backend-only identity fields.
2. Correct all `INIT_SPACE` and discriminator sizing.
3. Implement original-payer tracking and deterministic close/refund paths.
4. Replace pseudo-fees with explicit service-fee transfers.
5. Correct stake and tournament accounting.

### Phase 4: redesign sessions

1. Implement bounded session authorization and complete debit accounting.
2. Build user-selected funding UI.
3. Unify global/tournament/per-game session behavior.
4. Implement revoke/expiry/exhaustion closure and local-key deletion.

### Phase 5: migrate every signing surface

Migrate in risk order:

1. global-session authorization;
2. wager create/join;
3. tournament registration/leave;
4. dispute creation;
5. prize claims;
6. cancel, withdraw, resign, draw and timeout actions;
7. ~~social actions that remain on-chain during transition~~ (none remain: friends are backend-only);
8. authentication messages.

Replace guessed balance buffers with envelope totals.

### Phase 6: deploy cleanly

1. Generate a new devnet program ID and update IDLs/configuration only after tests pass.
2. Update backend, client, Tauri, website and tournament-pass registrar references.
3. Publish the final v2 IDL and register the program/application metadata with supported wallets.
4. Verify the production XFChess domain through each supported wallet's official process.
5. Freeze writes to the old devnet program.
6. Do not migrate test profiles/usernames.
7. Exercise all financial flows on devnet and reconcile wallet deltas.
8. Use v2 as the first mainnet deployment after security, compliance and legal review.

## Critical files

- `programs/xfchess-game/src/state/player_profile.rs`
- `programs/xfchess-game/src/state/game.rs`
- `programs/xfchess-game/src/state/global_session.rs`
- `programs/xfchess-game/src/account_ix/profile.rs`
- `programs/xfchess-game/src/account_ix/global_session_ix.rs`
- `programs/xfchess-game/src/game_ix/common.rs`
- `programs/xfchess-game/src/lifecycle/settlement.rs`
- `programs/xfchess-game/src/recovery_ix.rs`
- `programs/xfchess-game/src/governance_ix/`
- `programs/xfchess-game/src/tournament_ix/`
- `crates/xfchess-platform-api/src/lib.rs`
- `crates/xfchess-client-solana/src/instructions.rs`
- `crates/xfchess-client-bevy/src/multiplayer/solana/`
- `apps/tauri/src/wallet/`
- `apps/tauri/wallet-ui/src/wallet/TransactionSigner.tsx`
- `services/backend/src/signing/`

## Verification

### Contract tests

- Player account is <=64 bytes and contains no PII.
- Stake escrow always equals `2 * stake`; service fees cannot reduce it.
- Fee caps and tournament allocations are enforced on-chain.
- Every temporary PDA closes on every terminal path and refunds the recorded payer.
- Dispute bond and dispute rent follow independent rules.
- Session caps cannot exceed funding and every debit category is counted.
- Repeated close, cancellation and recovery calls are safe.

### Quote and bridge tests

- Decoded transaction totals equal the review.
- Existing accounts remove rent lines.
- Fee/rent changes produce a new quote.
- Mutation, payer/cluster mismatch, blockhash refresh and expiry invalidate the quote.
- Legacy and v0 messages produce equivalent classifications.
- The bridge rejects unknown or unclassified value transfers.
- Every user-facing transaction contains exactly one expected sanitized memo.
- Memo text and shared `WalletAction` resolve to the same canonical purpose.
- Memo contents reject personal data and remain within the configured byte limit.

### Tauri tests

- No wallet API is called before explicit confirmation.
- All charge categories and future session exposure render correctly.
- Authentication messages display zero financial authority.
- Cancellation and quote expiry do not sign or broadcast.
- Signed transaction integrity checks remain intact.
- Phantom, Solflare and Privy integration tests tolerate wallet-specific rendering while preserving the XFChess review and message digest.
- Single-purpose flows do not contain unrelated value-moving instructions.

### End-to-end tests

- Profile create, close and rating-preserving reactivation.
- Wager create/join/settle, cancellation and timeout.
- Tournament register/leave/start/cancel/prize claim.
- Dispute uphold/dismiss/stale claim.
- Session authorize/spend/revoke/expire/close.
- Final wallet deltas reconcile with the reviewed envelope.

Do not build, deploy, sign or send transactions while implementing the plan unless separately authorized.
