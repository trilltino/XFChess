//! Lamport-movement helpers — the one place that encodes Solana's account
//! ownership rules for fund flows.
//!
//! Two distinct cases, easy to get wrong (and the source of an earlier bug
//! where a system-owned escrow was debited directly):
//!
//! * **System-owned PDA** (the per-game wager escrow, funded by
//!   `system_program::transfer`): a program may *not* decrement its lamports
//!   directly. Money must leave via a `system_program::transfer` CPI signed
//!   with the escrow's seeds — [`pay_from_game_escrow`].
//! * **Program-owned PDA** (fee vault, tournament escrow, global-session
//!   vault): the owning program *may* decrement lamports directly, as long as
//!   the account stays rent-exempt — [`debit_program_pda`].

use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};

use crate::constants::WAGER_ESCROW_SEED;
use crate::errors::GameErrorCode;

/// The total pot for a wager (`wager × 2`), checked for overflow as defense in
/// depth even though `wager` is capped at `MAX_WAGER_AMOUNT`.
#[inline]
pub fn pot(wager: u64) -> Result<u64> {
    wager
        .checked_mul(2)
        .ok_or_else(|| GameErrorCode::ArithmeticOverflow.into())
}

/// Move `lamports` out of the **system-owned** per-game wager escrow to `to`,
/// signed by the escrow seeds. No-op when `lamports == 0`.
///
/// This is the only correct way to debit the wager escrow: it is owned by the
/// System Program, so a direct lamport decrement would fail the runtime's
/// ownership check.
pub fn pay_from_game_escrow<'info>(
    system_program: &Program<'info, System>,
    escrow: &SystemAccount<'info>,
    to: &AccountInfo<'info>,
    lamports: u64,
    game_id: u64,
    escrow_bump: u8,
) -> Result<()> {
    if lamports == 0 {
        return Ok(());
    }
    let game_id_bytes = game_id.to_le_bytes();
    let signer: &[&[&[u8]]] = &[&[WAGER_ESCROW_SEED, &game_id_bytes, &[escrow_bump]]];
    system_program::transfer(
        CpiContext::new_with_signer(
            system_program.to_account_info(),
            Transfer {
                from: escrow.to_account_info(),
                to: to.clone(),
            },
            signer,
        ),
        lamports,
    )
}

/// Direct-debit `lamports` from a **program-owned** PDA to `to`, keeping the
/// PDA rent-exempt. No-op when `lamports == 0`. Errors if the debit would push
/// the PDA below its rent-exempt minimum.
pub fn debit_program_pda<'info>(
    pda: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    lamports: u64,
) -> Result<()> {
    if lamports == 0 {
        return Ok(());
    }
    let rent_min = Rent::get()?.minimum_balance(pda.data_len());
    require!(
        pda.lamports().saturating_sub(lamports) >= rent_min,
        GameErrorCode::InsufficientFunds
    );
    **pda.try_borrow_mut_lamports()? -= lamports;
    **to.try_borrow_mut_lamports()? += lamports;
    Ok(())
}

/// Returns an error (instead of silently skipping a transfer) when crediting
/// `added` lamports would leave `dest` below the rent-exempt minimum.
pub fn require_rent_exempt_after(dest: &AccountInfo, added: u64) -> Result<()> {
    let rent = Rent::get()?;
    let after = dest.lamports().saturating_add(added);
    require!(
        rent.is_exempt(after, dest.data_len()),
        GameErrorCode::InsufficientFunds
    );
    Ok(())
}
