# Program fix plan: treasury payout, close_tournament drain, authority hygiene

Status: **implemented in-tree (2026-07-02)** — compiles, all tests pass. Not yet deployed; still requires a program upgrade + redeploy to take effect on devnet/mainnet.
Scope: `programs/xfchess-game/`. Program ID `8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU` (localnet + devnet).
Author: audit follow-up (2026-07-02).

## As-built notes (deviations from the original plan)

- **`close_tournament` was never wired into the `#[program]` block.** The dangerous
  handler existed with an accounts struct and mod re-exports, and `distribute.rs`
  references it as the escrow-reclamation step, but there was no entrypoint — so the
  finding-#2 drain was not actually reachable on-chain. Resolution: rewrote the handler
  to the safe finalizer **and wired it in** (entrypoint + `__client_accounts` wrapper +
  crate-root re-export), so the intended finalizer now exists as the safe version.
- **Hardened `CloseTournament.authority` from `UncheckedAccount` → `Signer`.** The
  original struct only compared `authority.key()` against `tournament.authority` without
  requiring a signature, so anyone could pass the authority pubkey. Now it must sign.
- **`close_tournament` fully reclaims the escrow.** Instead of leaving the escrow at
  rent-exempt, the safe handler sweeps the entire remaining balance (leftover bps
  remainder + unallocated shares + the account's own rent) to the treasury once every
  funded place is claimed, and lets the zero-lamport escrow be reclaimed — matching the
  "close_tournament reclaims it" intent in `distribute.rs`.
- **Real keypairs generated** for `treasury_authority`
  (`8e7NzfKVTyeSmsqjuESoXT9WCadkRioyKgJfNeHMG4HM`) and `link_authority`
  (`42fiB5KcC1jEVXxmgPoWqpA3zuKEsZGu77YHmCwNEcrh`); secrets are in gitignored
  `keys/treasury_authority.json` / `keys/link_authority.json`. `treasury_authority` is
  currently a single fresh key — move to multisig and rotate before mainnet (Fix 4).
- **Tests added:** `programs/xfchess-game/tests/treasury_tests.rs` (7 tests: 4 for
  `withdraw_treasury`, 3 for `close_tournament`) — all passing alongside the existing
  ER suites. Registered as a `[[test]]` in `Cargo.toml`.
- **Still pending:** regenerate IDL + add a `withdraw_treasury` client builder in
  `crates/solana-chess-client`; devnet deploy + smoke; key rotation/multisig; mainnet
  upgrade.

This document is the apply-ready spec for four findings from the on-chain money-flow
audit. Each fix section is self-contained: the problem, the exact files to add/change,
the full code, the wiring, and the tests. Do them in the order below — Fix 1 and Fix 2
are the money-critical ones and are independent of each other.

---

## Findings summary

| # | Finding | Severity | Type of change |
|---|---------|----------|----------------|
| 1 | `treasury_vault` has no withdrawal instruction — all PvP platform fees, dispute fees, and forfeited bonds are permanently stranded. | High (you never get paid from PvP) | New instruction |
| 2 | `close_tournament` pays unconstrained `remaining_accounts` positionally, runs in `Active`, and mis-accounts escrow lamports — drains the tournament escrow to arbitrary wallets. | Critical (theft) | Rewrite / restrict |
| 3 | `link_authority` pubkey is all-zeros — `link_external_elo` is unusable/unsafe. | Medium | Constant + key gen |
| 4 | `vps_authority` / `dispute_authority` / `kyc_authority` are single keys with devnet secrets exposed in git history. | High (pre-mainnet) | Key management |

### What is and isn't affected

- Tournament **entry-fee revenue already works** — `start_tournament` sweeps 100% of entry fees to `host_treasury`. No change needed there.
- Tournament **prize payouts already work** via `claim_tournament_prize` (pull) and `distribute_tournament_prizes` (push, winner-constrained). The `close_tournament` bug is a *separate* drain path bolted on top of those; removing its payout logic does not break winner payouts.
- The `PlatformFeeVault` + `claim_fees` path is functional but disconnected (its only deposit route, `collect_fee`, just moves your own SOL in). Fix 1 targets the `treasury_vault` PDA that game/dispute settlement actually feeds — that is the one holding your unspent PvP revenue.

---

## Fix 1 — Add `withdraw_treasury` so PvP fees become claimable

### Problem

`treasury_vault` (seeds `[TREASURY_VAULT_SEED]`, i.e. `b"treasury_vault"`) is a **System-owned PDA**. It is credited in three places:

- `game_ix/finalize.rs:99` — advanced platform fees per settled game
- `game_ix/finalize.rs:105` — ranked-game country/platform fee
- `governance_ix/resolve.rs:94` — flat dispute-resolution fee, plus forfeited dispute bonds

`grep TREASURY_VAULT_SEED` shows **no instruction ever debits it**. Because it is
System-owned, lamports can only leave via a `system_program::transfer` CPI signed by
the vault's seeds — a direct `**vault.lamports.borrow_mut() -= …` would fail the
runtime's ownership check. That CPI does not exist, so the money is locked forever.

> Note: the `TreasuryVault` *data* struct in `state/treasury_vault.rs` (seeds
> `[TREASURY_VAULT_SEED, country_code]`, with an `authority`/`total_collected` field)
> is a **different, unused PDA**. The live vault used by settlement is the bare
> no-country `[TREASURY_VAULT_SEED]` system account with no data. The withdrawal below
> targets the live one. Leave the unused struct alone (or delete it in a later cleanup).

### Design

- New instruction `withdraw_treasury(amount: u64)`.
- Authority gate: **a dedicated treasury authority**, not the operational `vps_authority`, so treasury access is separable from result-signing (ties into Fix 4). For a minimal first cut you may point it at `vps_authority::ID`, but the constant is defined separately so you can harden it without an ABI change.
- Moves `amount` lamports out via a seed-signed `system_program::transfer` CPI — identical mechanism to `escrow::pay_from_game_escrow`.
- Keeps the vault rent-exempt (a 0-data system PDA needs ~0.00089 SOL) so partial withdrawals don't delete the account mid-accumulation.
- Emits an event for off-chain accounting.

### New file: `programs/xfchess-game/src/account_ix/treasury.rs`

```rust
//! Withdraw accumulated platform fees from the system-owned treasury vault.
//!
//! The treasury vault (seeds `[TREASURY_VAULT_SEED]`) accrues PvP platform fees,
//! dispute-resolution fees, and forfeited dispute bonds from game settlement.
//! It is System-owned, so — exactly like the per-game wager escrow — lamports may
//! only leave through a `system_program::transfer` CPI signed with the vault seeds.
//! A direct lamport decrement would fail the runtime's ownership check.

use crate::constants::*;
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};

#[event]
pub struct TreasuryWithdrawn {
    pub authority: Pubkey,
    pub destination: Pubkey,
    pub amount: u64,
    pub remaining: u64,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct WithdrawTreasury<'info> {
    /// System-owned platform treasury vault — the destination of all PvP
    /// platform/dispute fees. Seeded PDA, so it cannot be substituted.
    #[account(mut, seeds = [TREASURY_VAULT_SEED], bump)]
    pub treasury_vault: SystemAccount<'info>,
    /// Only the dedicated treasury authority may withdraw. Kept separate from
    /// `vps_authority` so treasury access can be a multisig without touching the
    /// result-signing key (see Fix 4).
    #[account(
        mut,
        address = crate::constants::treasury_authority::ID @ GameErrorCode::UnauthorizedAccess
    )]
    pub authority: Signer<'info>,
    /// Destination wallet for the withdrawn fees.
    #[account(mut)]
    pub destination: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WithdrawTreasury>, amount: u64) -> Result<()> {
    require!(amount > 0, GameErrorCode::InvalidArgument);

    let vault = &ctx.accounts.treasury_vault;
    // Keep the vault rent-exempt so partial withdrawals don't garbage-collect it
    // while fees are still accumulating between claims.
    let rent_min = Rent::get()?.minimum_balance(vault.data_len());
    let remaining = vault
        .lamports()
        .checked_sub(amount)
        .ok_or(GameErrorCode::InsufficientFunds)?;
    require!(remaining >= rent_min, GameErrorCode::InsufficientFunds);

    let bump = ctx.bumps.treasury_vault;
    let signer: &[&[&[u8]]] = &[&[TREASURY_VAULT_SEED, &[bump]]];
    system_program::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.treasury_vault.to_account_info(),
                to: ctx.accounts.destination.to_account_info(),
            },
            signer,
        ),
        amount,
    )?;

    emit!(TreasuryWithdrawn {
        authority: ctx.accounts.authority.key(),
        destination: ctx.accounts.destination.key(),
        amount,
        remaining,
    });
    Ok(())
}
```

### Wiring

**`constants.rs`** — add a dedicated treasury authority next to the other authority
modules. For the first deploy you may set the bytes equal to `vps_authority`'s; split
it later (Fix 4) without an ABI change.

```rust
/// The treasury-withdrawal authority — the only signer allowed to call
/// `withdraw_treasury`. Deliberately separate from `vps_authority` so platform
/// revenue can sit behind a multisig without also gating result-signing.
/// Replace with a dedicated (ideally multisig / squads) pubkey before mainnet.
pub mod treasury_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array([
        // TODO: paste the treasury authority pubkey bytes here.
        // For an interim single-key deploy, copy vps_authority's byte array.
    ]);
}
```

**`account_ix/mod.rs`** — register the module and re-export:

```rust
pub mod treasury;
pub use treasury::WithdrawTreasury;
```

**`lib.rs`** — add to the `account_ix` re-export list (near the existing
`InitializeFeeVault, CollectFee, ClaimFees` line):

```rust
pub use account_ix::{ /* …existing… */ , WithdrawTreasury};
```

Add the `__client_accounts` wrapper next to the other treasury/fee ones:

```rust
pub mod __client_accounts_withdraw_treasury {
    pub use crate::account_ix::treasury::__client_accounts_withdraw_treasury::*;
}
```

Add the entrypoint inside `#[program] pub mod xfchess_game`:

```rust
/// Withdraw accumulated platform fees from the treasury vault to a destination
/// wallet. Only the treasury authority may call this.
pub fn withdraw_treasury(ctx: Context<WithdrawTreasury>, amount: u64) -> Result<()> {
    crate::account_ix::treasury::handler(ctx, amount)
}
```

### Client / backend follow-up (post-deploy, not blocking)

- Add a `withdraw_treasury` builder in `crates/solana-chess-client` mirroring the existing fee/claim builders.
- Optionally add a backend admin route (behind the same auth as other privileged endpoints) to trigger a withdrawal to your ops wallet, or run it from the `tournament_admin`-style CLI.

### Tests (add to `programs/xfchess-game/tests/`)

1. **Happy path**: seed the treasury PDA with lamports (run a ranked `finalize_game`, or airdrop directly to the derived PDA in the test harness), call `withdraw_treasury(amount)` as `treasury_authority`, assert destination `+= amount` and vault `-= amount`.
2. **Rent floor**: attempt to withdraw `balance - rent_min + 1` → expect `InsufficientFunds`.
3. **Wrong signer**: call as a random keypair → expect `UnauthorizedAccess`.
4. **Zero amount** → expect `InvalidArgument`.
5. **Wrong PDA**: pass a non-seed account as `treasury_vault` → Anchor seeds constraint rejects.

---

## Fix 2 — Restrict / rewrite `close_tournament` to stop the drain

### Problem (`tournament_ix/lifecycle/close_tournament.rs`)

The current handler, when `prize_escrow_pda.lamports() > 0`:

- Iterates `prize_shares` and pays `ctx.remaining_accounts[i]` **positionally, with no check that account `i` is the recorded winner for place `i`** — unlike `distribute_tournament_prizes`, which matches each wallet against `tournament.winner/second_place/…`. A caller can pass their own wallets and receive the pool.
- Is allowed to run in **`Active`** as well as `Completed` (`close_tournament.rs:37-41`) — i.e. before results even exist.
- Credits recipients with `+= prize_amount` but **never debits the escrow per payment**; it just sets `prize_escrow_pda.lamports = 0` at the end (`:91`). Combined with the also-credited `treasury_vault` remainder, the lamport bookkeeping does not balance and either fails the runtime invariant or mispays.
- Touches the entry-fee escrow, which after `start_tournament` holds only rent (fees were already swept to `host_treasury`) — so its mental model of "distribute the entry-fee pool" is already wrong.

Winners are **already** paid correctly by `distribute_tournament_prizes` (push, winner-constrained) and `claim_tournament_prize` (pull). `close_tournament` should not move prize money at all.

### Design

Turn `close_tournament` into a **safe finalizer**:

- Require `status == Completed` (never `Active`).
- Keep the existing authority gate (`tournament.authority` or `vps_authority::ID`).
- **Verify every funded prize place has already been claimed** before closing, so flipping the status to `Closed` (which disables the `Completed`-gated claim/distribute paths) can never strand an unpaid winner.
- Sweep only true **residual dust** (escrow balance above rent-exempt, after all winners are paid — rounding remainder from bps math) to the platform `treasury_vault`. No `remaining_accounts`, no positional payout.
- Drop the entry-fee-escrow payout logic entirely.

### Full replacement handler

Replace the body of `close_tournament.rs` with the following. The accounts struct can
stay, but **remove the `remaining_accounts` usage** and keep `prize_escrow_pda`,
`treasury_vault`, `authority`. (You may also drop `system_program` if unused after the
rewrite.)

```rust
pub fn handler(ctx: Context<CloseTournament>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    require!(tournament.tournament_id == tournament_id, GameErrorCode::UnauthorizedAccess);

    // Close only AFTER completion. Never during Active — results may not exist.
    require!(
        tournament.status == TournamentStatus::Completed,
        GameErrorCode::InvalidTournamentStatus
    );

    // Authority: tournament host or platform admin.
    require!(
        ctx.accounts.authority.key() == tournament.authority
            || ctx.accounts.authority.key() == crate::constants::vps_authority::ID,
        GameErrorCode::UnauthorizedAccess
    );

    // Every funded place must already be paid (via distribute_tournament_prizes or
    // claim_tournament_prize) before we disable those Completed-gated paths.
    let places: [Option<Pubkey>; 10] = [
        tournament.winner,
        tournament.second_place,
        tournament.third_place,
        tournament.fourth_place,
        tournament.fifth_place,
        tournament.sixth_place,
        tournament.seventh_place,
        tournament.eighth_place,
        tournament.ninth_place,
        tournament.tenth_place,
    ];
    for (i, place) in places.iter().enumerate() {
        // A place that exists and carries a nonzero share must have its claim bit set.
        if place.is_some() && tournament.prize_shares[i] > 0 {
            let bit = 1u16 << i;
            require!(
                tournament.prizes_claimed & bit != 0,
                GameErrorCode::PrizeAlreadyClaimed // reuse: "prizes still outstanding" guard
            );
        }
    }

    // Sweep only residual dust (bps rounding remainder) above rent-exempt to the
    // platform treasury. The escrow is program-owned (TournamentEscrow), so a direct
    // lamport debit is the correct mechanism here.
    let escrow_ai = ctx.accounts.prize_escrow_pda.to_account_info();
    let rent_min = Rent::get()?.minimum_balance(escrow_ai.data_len());
    let sweepable = escrow_ai.lamports().saturating_sub(rent_min);
    if sweepable > 0 {
        **escrow_ai.try_borrow_mut_lamports()? -= sweepable;
        **ctx.accounts.treasury_vault.to_account_info().try_borrow_mut_lamports()? += sweepable;
    }

    tournament.status = TournamentStatus::Closed;
    Ok(())
}
```

> Optional nicety: add a dedicated `PrizesOutstanding` variant to `GameErrorCode`
> instead of reusing `PrizeAlreadyClaimed`, so the "cannot close, winners unpaid"
> case has a clear message. Purely cosmetic; adds an error code at the end of the enum.

### Alternative: disable entirely

If you'd rather not maintain the finalizer, the minimum-risk option is to make the
handler a pure state transition with **no fund movement at all** (keep the
`status == Completed` + authority checks + the all-claimed guard, then set `Closed`
and return). Escrow rent is then reclaimed later by a separate, explicitly-guarded
instruction or left as dust. This removes the drain vector without the sweep logic.

### Tests

1. **Drain attempt is rejected**: complete a tournament, then call `close_tournament` passing attacker wallets in `remaining_accounts` → with the rewrite there is no positional payout, so attacker balances are unchanged; residual only goes to `treasury_vault`.
2. **Active-phase close blocked**: call on an `Active` tournament → `InvalidTournamentStatus`.
3. **Unpaid winner blocks close**: complete a tournament, do *not* claim, call `close_tournament` → rejected (guard). Then claim all places, call again → succeeds and sets `Closed`.
4. **Dust sweep**: after all claims, assert escrow is left at exactly rent-exempt and the rounding remainder landed in `treasury_vault`.
5. **Regression**: `distribute_tournament_prizes` and `claim_tournament_prize` still pay the correct shares to recorded winners (unchanged).

---

## Fix 3 — Set a real `link_authority`

### Problem (`constants.rs:77-86`)

`link_authority::ID` is 32 zero bytes. `link_external_elo` gates on
`address = link_authority::ID`, so with the default `Pubkey` (all zeros) the check is
either impossible to satisfy with a real signer or trivially wrong. The instruction is
effectively dead / unsafe.

### Fix

1. Generate a dedicated keypair for the link authority (backend signer):
   ```bash
   solana-keygen new -o keys/link_authority.json
   solana-keygen pubkey keys/link_authority.json
   ```
2. Convert the pubkey to its 32-byte array and paste into `constants.rs`:
   ```rust
   pub mod link_authority {
       use super::*;
       pub const ID: Pubkey = Pubkey::new_from_array([ /* 32 bytes of the real key */ ]);
   }
   ```
   (Or, cleaner: `pub const ID: Pubkey = pubkey!("<base58>");` using
   `anchor_lang::prelude::Pubkey` + the `solana_program::pubkey!` macro if already in scope.)
3. Store the secret **only** in `backend/.env` as `LINK_AUTHORITY_KEY` and in
   `keys/link_authority.json` — both already gitignored. Do **not** commit the secret.
4. If `link_external_elo` is not launching with this deploy, consider gating the
   handler behind a `#[cfg(feature = "…")]` or leaving it unreachable, rather than
   shipping a zero-key gate.

Requires redeploy (constant is compiled in).

---

## Fix 4 — Split and harden the privileged authorities

### Problem

`vps_authority`, `dispute_authority`, `kyc_authority` (and now `treasury_authority`)
are single Ed25519 keys. Per memory (`project_secret_exposure.md`), the devnet secrets
were in **public git history**; they were regenerated and untracked locally, but the
history still contains the old secrets and rotation before mainnet is outstanding.
Compromise of `vps_authority` alone lets an attacker forge every tournament result
(`record_match_result` / `record_swiss_result` are authority-trusted with no on-chain
game verification) and initialize tournaments.

### Fix (staged; only the constant swaps need a redeploy)

1. **Separate duties** — keep four distinct keys, never reuse one across roles:
   - `vps_authority` → result signing + ELO + tournament init (operational, hot).
   - `treasury_authority` → `withdraw_treasury` (revenue, cold/multisig).
   - `dispute_authority` → `resolve_dispute` (cold).
   - `kyc_authority` → `verify_profile`.
2. **Move revenue + dispute keys to multisig** — use Squads (or an equivalent
   multisig) for `treasury_authority` and `dispute_authority`. The `address = …ID`
   gate accepts the multisig's authority PDA/derived signer; validate the exact
   signer type your multisig exposes when you wire it.
3. **Rotate every key before mainnet**, since the old secrets are in history. Generate
   fresh keypairs, update the `constants.rs` byte arrays, redeploy.
4. **Never rely on git-history removal alone** — treat all previously-committed secrets
   as burned. Rotation is the mitigation, not history rewriting.
5. Keep the `keys/*.json` and `backend/.env` entries gitignored (already are); confirm
   with `git check-ignore` after regenerating.

This finding is partly process, not code — but the `constants.rs` byte arrays are the
on-chain enforcement point, so any key change is a compiled-in change requiring redeploy.

---

## Build, upgrade, and deploy procedure

All three code fixes (1, 2, 3) and any key rotation (4) ship in one program upgrade.

1. **Build**:
   ```bash
   scripts\build_program.bat        # or: anchor build
   ```
   Confirm the program size still fits and `opt-level = "z"` is intact.
2. **Test locally**:
   ```bash
   cargo test -p xfchess-game
   cargo test -p xfchess-game --test smoke_tests
   cargo test -p xfchess-game --test security_tests
   ```
   Add the new test cases from Fixes 1 and 2 first.
3. **Regenerate the IDL** and sync it to the clients (`crates/solana-chess-client`,
   `web-solana`) — the new `withdraw_treasury` instruction and any changed account
   structs must be reflected downstream.
4. **Deploy to devnet** and smoke-test end to end:
   ```bash
   anchor deploy            # devnet
   ```
   - Run a ranked PvP game to accrue a treasury fee, then `withdraw_treasury` it out.
   - Run a full tournament (fund → register → start → results → claim → close) and
     confirm `close_tournament` succeeds only after all winners are paid and moves no
     prize money to non-winners.
5. **Mainnet** (only after key rotation in Fix 4 and a clean devnet run):
   ```bash
   solana program deploy target/deploy/xfchess_game.so   # ~6.5 SOL
   ```
   The program is upgradeable, so this is an in-place upgrade at the same program ID —
   existing PDAs (including the accumulated `treasury_vault` balance) are preserved and
   become withdrawable immediately after the upgrade.

### Upgrade safety notes

- Fix 1 is **additive** (new instruction, new constant) — no existing account layout
  changes, no migration. The stranded treasury balance is claimable the moment the
  upgrade lands.
- Fix 2 changes `close_tournament`'s behavior but not the `Tournament` account layout,
  so no data migration. Any tournament mid-flight keeps working; the only behavioral
  change is that `close` now refuses to run early or pay non-winners.
- Fix 3/4 change compiled-in constants only — no layout impact.

---

## Apply checklist

- [ ] `constants.rs`: add `treasury_authority` module (Fix 1); set real `link_authority` bytes (Fix 3); rotate all authority byte arrays before mainnet (Fix 4).
- [ ] Add `account_ix/treasury.rs` with `WithdrawTreasury` + `handler` + `TreasuryWithdrawn` event (Fix 1).
- [ ] `account_ix/mod.rs`: `pub mod treasury; pub use treasury::WithdrawTreasury;`
- [ ] `lib.rs`: re-export `WithdrawTreasury`, add `__client_accounts_withdraw_treasury`, add `withdraw_treasury` entrypoint (Fix 1).
- [ ] Rewrite `close_tournament.rs` handler: require `Completed`, all-claimed guard, dust-only sweep, no `remaining_accounts` payout (Fix 2).
- [ ] (Optional) add `PrizesOutstanding` error variant for the close guard.
- [ ] New tests for `withdraw_treasury` (5 cases) and `close_tournament` (5 cases).
- [ ] `cargo test -p xfchess-game` green.
- [ ] Regenerate IDL; add `withdraw_treasury` builder to `solana-chess-client`.
- [ ] Devnet deploy + end-to-end smoke (treasury withdraw + full tournament close).
- [ ] Rotate keys, move treasury/dispute to multisig, mainnet upgrade.

## Related memory

- `project_secret_exposure.md` — exposed devnet secrets; rotation still outstanding (Fix 4).
- `project_auth_hardening.md` — prior auth pass; session-endpoint auth + JWT revocation backlog.
