//! Discover games independently of the session-key database. Delegation changes
//! ownership but preserves Game data; always verify the discriminator and PDA.
use super::{game_account, DELEGATION_PROGRAM_ID, GAME_SEED};
use sha2::{Digest, Sha256};
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig, UiAccountEncoding},
    rpc_filter::{Memcmp, RpcFilterType},
};
use solana_sdk::{account::Account, pubkey::Pubkey};
use std::collections::BTreeSet;

pub fn checked_game(
    program: &Pubkey,
    address: &Pubkey,
    account: &Account,
) -> Option<game_account::GameAccount> {
    let delegated: Pubkey = DELEGATION_PROGRAM_ID.parse().ok()?;
    if account.executable
        || (account.owner != *program && account.owner != delegated)
        || account.data.get(..8)? != &Sha256::digest(b"account:Game")[..8]
    {
        return None;
    }
    let game = game_account::parse(&account.data)?;
    let expected =
        Pubkey::find_program_address(&[GAME_SEED, &game.game_id.to_le_bytes()], program).0;
    (*address == expected).then_some(game)
}

pub fn discover_game_ids(rpc: &RpcClient, program: &Pubkey) -> Result<BTreeSet<u64>, String> {
    let delegation: Pubkey = DELEGATION_PROGRAM_ID
        .parse()
        .map_err(|e| format!("Bad delegation program: {e}"))?;
    let discriminator = Sha256::digest(b"account:Game")[..8].to_vec();
    let mut ids = BTreeSet::new();
    for owner in [*program, delegation] {
        let accounts = rpc
            .get_program_ui_accounts_with_config(
                &owner,
                RpcProgramAccountsConfig {
                    filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
                        0,
                        discriminator.clone(),
                    ))]),
                    account_config: RpcAccountInfoConfig {
                        encoding: Some(UiAccountEncoding::Base64),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .map_err(|e| format!("Game discovery failed for {owner}: {e}"))?;
        for (address, ui_account) in accounts {
            let Some(data) = ui_account.data.decode() else {
                continue;
            };
            let Ok(owner) = ui_account.owner.parse() else {
                continue;
            };
            let account = Account {
                lamports: ui_account.lamports,
                data,
                owner,
                executable: ui_account.executable,
                rent_epoch: ui_account.rent_epoch,
            };
            let Some(game) = checked_game(program, &address, &account) else {
                continue;
            };
            if game.wager_amount > 0 && !game.is_terminal() {
                ids.insert(game.game_id);
            }
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context as _;

    #[test]
    fn accepts_only_game_data_at_its_canonical_pda_with_a_known_owner() -> anyhow::Result<()> {
        let program = Pubkey::new_unique();
        let id = 42u64;
        let pda = Pubkey::find_program_address(&[GAME_SEED, &id.to_le_bytes()], &program).0;
        let mut account = Account {
            lamports: 1,
            data: vec![0; 365],
            owner: program,
            executable: false,
            rent_epoch: 0,
        };
        account.data[..8].copy_from_slice(&Sha256::digest(b"account:Game")[..8]);
        account.data[8..16].copy_from_slice(&id.to_le_bytes());
        assert_eq!(
            checked_game(&program, &pda, &account)
                .context("test setup failed")?
                .game_id,
            id
        );
        account.owner = DELEGATION_PROGRAM_ID.parse().context("test setup failed")?;
        assert!(checked_game(&program, &pda, &account).is_some());
        assert!(checked_game(&program, &Pubkey::new_unique(), &account).is_none());
        account.owner = Pubkey::new_unique();
        assert!(checked_game(&program, &pda, &account).is_none());
        account.owner = program;
        account.data[0] ^= 1;
        assert!(checked_game(&program, &pda, &account).is_none());
        account.data.truncate(7);
        assert!(checked_game(&program, &pda, &account).is_none());
        Ok(())
    }
}
