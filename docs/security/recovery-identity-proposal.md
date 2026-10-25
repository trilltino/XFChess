# Recovery identity design (approved; local runtime checks passed)

The owner explicitly approved the schema and client migration in this session.
The implementation refines the proposal below: the two booleans become one
`RecoveryStatus` enum (Waiting, Ready, Delegated, Closed, Recovered), making the
account **90 bytes**, with default Rent **1,517,280 lamports**, asserted by tests.
Query cluster rent again before release. There are no changes to existing Game
or Profile serialized layouts, existing error numbering, or the deployed ID.

Nine v2 instructions preserve existing argument bytes and account prefixes and
append the identity account. Creation binds white; joining binds black exactly
once. Tournament authority creation binds both registered seats. Setup and the
original handler are atomic. Explicit canonical PDA checks precede record writes;
the composite Anchor context must not deserialize the arguments twice.
Records are permanent, never delegated, never reinitialized, and never closed.
An intact active Game may be redelegated with identical bound seats and wager;
closed/recovered records cannot be delegated. V2 creation cannot reuse the ID.

V2 recovery requires a canonical XFChess-owned empty Game and a canonical record
in Delegated status. Recipients must equal its distinct nonzero players before
payments or rent movement. Each receives exactly the stored single-player wager;
insufficient escrow fails atomically. Zero wager pays no escrow. Donations and
remainders stay in escrow. Status, payouts, the existing Game-rent sweep, and the
existing event are atomic; replay fails. Native SOL is supported.

Legacy delegation and legacy empty-Game recovery fail closed. Ordinary finalize,
cancel, expiry and dispute payout instructions retain their discriminators but
require the canonical identity PDA as a writable trailing account. A nonexistent
System-owned PDA supports legacy games. Omission fails, preventing callers from
hiding an existing record while settling. Successful normal payouts mark Closed;
failed transactions roll back both record and money changes.
Legacy creation/join also require a canonical trailing identity account and accept
only an empty System-owned PDA. This rejects reuse of v2 game IDs and prevents a
legacy join from bypassing a v2 seat binding. Missing accounts fail closed.
Nonzero excess escrow must remain rent exempt after the wager refund; a smaller
remainder rejects before all payments and needs a permissionless escrow top-up.
Zero-wager recovery does not require funded recipient accounts.

The original creation fee payer funds record rent; authenticated global/tournament
session vaults fund it for session creation. Rent does not change wager allowance
semantics. Desktop, backend and CU builders, setup co-signing and dispute account
lists were migrated. Deployment requires coordinated clients and IDL validation.

Historical wiped games remain **BLOCKED: MISSING VERIFIED HISTORICAL IDENTITY
SOURCE**. No backfill instruction exists; operator/database/explorer assertions
or admin signatures cannot populate these records. Native forged-recipient and
zero-wager recovery checks, SBF nonzero payout conservation, wallet lifecycle,
global/tournament session funding and sponsored profiles pass. Actual ER
integration remains a release gate. Authority-role recovery fixtures explicitly
skip transaction cryptographic signature verification and access no secret.

## Original proposal presented for approval (superseded by the details above)

The execution prompt requires approval before new recovery accounts or an ABI
migration. The current patch rejects recovery for empty Games and preserves all
escrow and Game rent. No original-player proof is available in the current
instruction accounts. A DisputeRecord identifies only its challenger. Per-game
session delegations exist for some flows, but do not form an immutable universal
pair of original seats. TournamentMatch records cover only tournament games and
their pairings have mutation paths. None is a universal historical proof.

Proposed new program-owned PDA: `[b"recovery_identity", game_id.to_le_bytes()]`.
Proposed fields after the discriminator: `game_id: u64`, `white: Pubkey`,
`black: Pubkey`, `wager_amount: u64`, `finalized: bool`, `recovered: bool`,
`bump: u8` (91 bytes including discriminator). Native SOL only initially;
token escrow would require a separately reviewed design. Exact rent must be
queried and recorded before implementing.

Creation binds white from the authenticated creator in every creation flow;
black is initially the all-zero reserved value. Joining binds black from the
authenticated joiner exactly once, rejects white == black, and finalizes the
record. Tournament game creation binds both authenticated pairing seats and
finalizes atomically. Delegation requires a finalized record matching Game and
the wager amount. No admin may edit identities or reinitialize/close the PDA.
Cancellation/expiry/finalization must mark its escrow disposition so an old
record cannot pay again. Record permanence prevents reuse of a recovered game ID.

Recovery requires an empty canonical Game owned by XFChess and a canonical,
program-owned, finalized, unrecovered identity record; both supplied recipients
must equal its stored distinct nonzero players. Verify everything before money
movement. Pay each exactly the stored single-player wager (checked pot of two
wagers); insufficient escrow rejects atomically. Zero-wager recovery pays no
escrow. Unsolicited extra lamports and any remainder stay in escrow; no arbitrary
split of its current balance. Mark recovered atomically; repeated attempts fail.
Any Game-rent sweep needs the same verification and approved rent policy.

Account additions affect create/join/global/tournament/delegate/recovery clients
and IDLs. Prefer versioned instructions to preserve existing discriminators and
account order, with a reviewed transition that prevents new unbound delegation.
The creation payer funds the record (session vault for the global flow); exact
budget/rent effects must be approved together with the client migration.

Historical wiped games remain blocked. No operator/database/explorer assertion
or administrator signature can backfill identities. A future history-proof
mechanism needs independent design and approval. For intact legacy Games, an
upgrade must separately establish which committed state and signing conditions
are sufficient before allowing record creation; this proposal does not authorize
backfill from arbitrary stale delegated state.

Required tests: forged recipients under real authorized signer; forged/wrong-owner
record; cross-game record; missing/unfinished record; replacement and premature
closure; all creation/join paths; post-delegation mutation; duplicate recipients;
zero wager; odd/excess/insufficient escrow; settled/cancelled record; repeated
recovery; account balances and conservation including rent and failures; exact
client metas; actual ER round-trip. A successful authenticated payout cannot be
claimed until this design is approved, implemented, and runtime-tested.
