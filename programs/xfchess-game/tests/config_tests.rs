mod common;

use common::*;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signature::Signer,
};
use xfchess_game::account_ix::{InitializeConfigArgs, UpdateConfigArgs};
use xfchess_game::state::ProgramConfig;
use anchor_lang::{InstructionData, ToAccountMetas, AccountDeserialize, system_program};

pub fn config_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"config"], &xfchess_game::ID)
}

#[tokio::test]
async fn test_initialize_and_update_config() {
    let mut ctx = start(vec![]).await;
    let (config_key, _bump) = config_pda();
    let authority = ctx.payer.pubkey();

    let vps_auth = Pubkey::new_unique();
    let treasury_auth = Pubkey::new_unique();
    let dispute_auth = Pubkey::new_unique();
    let link_auth = Pubkey::new_unique();
    let kyc_auth = Pubkey::new_unique();

    let init_args = InitializeConfigArgs {
        treasury_authority: treasury_auth,
        vps_authority: vps_auth,
        dispute_authority: dispute_auth,
        link_authority: link_auth,
        kyc_authority: kyc_auth,
        max_wager_amount: Some(5_000_000_000),
        min_wager_lamports: Some(2_000_000),
        max_platform_fee_lamports: None,
        er_session_fee_lamports: None,
        dispute_bond_lamports: None,
        dispute_ttl_secs: None,
        crank_max_slot_delay: None,
        crank_max_seconds_early: None,
    };

    let init_ix = Instruction {
        program_id: xfchess_game::ID,
        accounts: xfchess_game::accounts::InitializeConfig {
            config: config_key,
            authority,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::InitializeConfig { args: init_args }.data(),
    };

    send(&mut ctx, init_ix, &[]).await.expect("initialize_config must succeed");

    // Fetch and verify initialized config
    let account = ctx.banks_client.get_account(config_key).await.unwrap().expect("config account exists");
    let mut data_slice: &[u8] = &account.data;
    let config = ProgramConfig::try_deserialize(&mut data_slice).expect("deserialize ProgramConfig");

    assert_eq!(config.authority, authority);
    assert_eq!(config.vps_authority, vps_auth);
    assert_eq!(config.treasury_authority, treasury_auth);
    assert_eq!(config.max_wager_amount, 5_000_000_000);
    assert_eq!(config.min_wager_lamports, 2_000_000);

    // Test update_config by authorized authority
    let new_vps = Pubkey::new_unique();
    let update_args = UpdateConfigArgs {
        new_vps_authority: Some(new_vps),
        new_max_wager_amount: Some(20_000_000_000),
        ..Default::default()
    };

    let update_ix = Instruction {
        program_id: xfchess_game::ID,
        accounts: xfchess_game::accounts::UpdateConfig {
            config: config_key,
            authority,
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::UpdateConfig { args: update_args }.data(),
    };

    send(&mut ctx, update_ix, &[]).await.expect("update_config must succeed");

    let account_updated = ctx.banks_client.get_account(config_key).await.unwrap().expect("config account exists");
    let mut data_slice_updated: &[u8] = &account_updated.data;
    let config_updated = ProgramConfig::try_deserialize(&mut data_slice_updated).expect("deserialize ProgramConfig");

    assert_eq!(config_updated.vps_authority, new_vps);
    assert_eq!(config_updated.max_wager_amount, 20_000_000_000);
    assert_eq!(config_updated.min_wager_lamports, 2_000_000); // untouched
}
