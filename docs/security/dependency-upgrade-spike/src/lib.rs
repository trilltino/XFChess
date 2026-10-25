use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

pub fn callback_boundary<'a>(
    game: &AccountInfo<'a>, owner: &Pubkey, buffer: &AccountInfo<'a>,
    payer: &AccountInfo<'a>, system: &AccountInfo<'a>, seeds: Vec<Vec<u8>>,
) {
    let _ = ephemeral_rollups_sdk::cpi::undelegate_account(game, owner, buffer, payer, system, seeds);
}
