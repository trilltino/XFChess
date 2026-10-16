use super::super::common::*;
use anyhow::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_system_interface::program as system_program;

// Close only after funded prizes are claimed. Sweep residual escrow to
// treasury; this instruction does not pay remaining-account recipients.
pub fn close_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let prize_escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let treasury_vault = Pubkey::find_program_address(&[TREASURY_VAULT_SEED], &program_id).0;

    let mut data = anchor_discriminator("close_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(prize_escrow_pda, false),
            AccountMeta::new(treasury_vault, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(authority, true),
        ],
        data,
    })
}

// Fund a guaranteed prize before any registration or existing prize funding.
pub fn fund_sol_prize_ix(
    program_id: Pubkey,
    operator: Pubkey,
    tournament_id: u64,
    amount_lamports: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("fund_sol_prize").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&amount_lamports.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// Distribute recorded SOL shares to writable winner accounts in any order;
// skip places whose wallet is absent.
pub fn distribute_tournament_prizes_ix(
    program_id: Pubkey,
    cranker: Pubkey,
    tournament_id: u64,
    winners: &[Pubkey],
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("distribute_tournament_prizes").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    let mut accounts = vec![
        AccountMeta::new(tournament_pda, false),
        AccountMeta::new(escrow_pda, false),
        AccountMeta::new_readonly(cranker, true),
    ];
    accounts.extend(winners.iter().map(|w| AccountMeta::new(*w, false)));

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}
