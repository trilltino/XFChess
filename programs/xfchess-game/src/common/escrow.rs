use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};

use crate::constants::WAGER_ESCROW_SEED;
use crate::errors::GameErrorCode;

#[inline]
pub fn pot(wager: u64) -> Result<u64> {
    wager
        .checked_mul(2)
        .ok_or_else(|| GameErrorCode::ArithmeticOverflow.into())
}

pub fn pay_from_game_escrow<'info>(
    _system_program: &Program<'info, System>,
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
            System::id(),
            Transfer {
                from: escrow.to_account_info(),
                to: to.clone(),
            },
            signer,
        ),
        lamports,
    )
}

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

pub fn require_rent_exempt_after(dest: &AccountInfo, added: u64) -> Result<()> {
    let rent = Rent::get()?;
    let after = dest.lamports().saturating_add(added);
    require!(
        rent.is_exempt(after, dest.data_len()),
        GameErrorCode::InsufficientFunds
    );
    Ok(())
}
