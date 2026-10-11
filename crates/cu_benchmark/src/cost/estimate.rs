use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};

use crate::cost::fees::transaction_fee;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeeProfile {
    Current,
    Legacy,
}

impl FeeProfile {
    fn deposit_rates(self) -> (u64, u64) {
        match self {
            Self::Current => (3_000_000, 1_000_000),
            Self::Legacy => (300_000, 100_000),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Workload {
    pub fee_profile: FeeProfile,
    pub variants: Vec<GameVariant>,
}

/// All counts describe one game; shared setup is amortized through `games`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GameVariant {
    pub name: String,
    pub games: u32,
    pub transactions: Vec<TransactionBatch>,
    pub er_transactions: u64,
    #[serde(default)]
    pub accounts: Vec<AccountRent>,
    #[serde(default)]
    pub delegations: Vec<Delegation>,
    #[serde(default)]
    pub base_actions: Vec<BaseAction>,
    #[serde(default)]
    pub callbacks: u32,
    #[serde(default)]
    pub topups: u32,
    #[serde(default)]
    pub ephemeral_accounts: Vec<EphemeralAccount>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionBatch {
    pub label: String,
    pub count: u32,
    pub signatures: u32,
    pub cu_limit: u32,
    pub cu_price_micro_lamports: u64,
    #[serde(default)]
    pub shared: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccountRent {
    pub label: String,
    /// Allocated bytes including discriminators, padding and vector capacity.
    pub data_bytes: usize,
    pub count: u32,
    pub rent_lamports: Option<u64>,
    pub closed: bool,
    #[serde(default)]
    pub shared: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Delegation {
    pub label: String,
    pub count: u32,
    /// Balance of delegation record plus metadata immediately before undelegation.
    pub deposit_lamports: u64,
    /// Successful normal commits, excluding the final commit-and-undelegate.
    pub normal_commits: u32,
    pub final_commit: bool,
    pub delegated_fee_payer: bool,
    #[serde(default)]
    pub shared: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaseAction {
    pub count: u32,
    /// Requested CU of each action in a single bundle; rounding happens per bundle.
    pub requested_cu: Vec<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EphemeralAccount {
    pub count: u32,
    pub data_bytes: u32,
    pub closed: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct Estimate {
    pub name: String,
    pub games: u32,
    pub er_transactions: u64,
    pub base_transaction_fees_lamports: u64,
    pub delegation_charges_lamports: u64,
    pub live_commit_fees_lamports: u64,
    pub base_action_fees_lamports: u64,
    pub callback_fees_lamports: u64,
    pub topup_fees_lamports: u64,
    pub rent_funded_lamports: u64,
    pub rent_refunded_lamports: u64,
    pub deposits_funded_lamports: u64,
    pub deposits_refunded_lamports: u64,
    pub ephemeral_storage_funded_lamports: u64,
    pub ephemeral_storage_refunded_lamports: u64,
    pub nonrefundable_fees_lamports: u64,
    pub retained_storage_lamports: u64,
    pub total_funding_lamports: u64,
    pub fees_per_game_lamports: f64,
}

fn add(total: &mut u64, amount: u64, count: u64) -> Result<()> {
    *total = total
        .checked_add(amount.checked_mul(count).context("cost overflow")?)
        .context("cost overflow")?;
    Ok(())
}

fn sum(values: &[u64]) -> Result<u64> {
    values
        .iter()
        .try_fold(0u64, |acc, n| acc.checked_add(*n).context("cost overflow"))
}

pub fn estimate(variant: &GameVariant, profile: FeeProfile) -> Result<Estimate> {
    ensure!(variant.games > 0, "games must be positive");
    let mut result = Estimate {
        name: variant.name.clone(),
        games: variant.games,
        ..Estimate::default()
    };
    add(
        &mut result.er_transactions,
        variant.er_transactions,
        u64::from(variant.games),
    )?;
    let count =
        |n: u32, shared: bool| u64::from(n) * if shared { 1 } else { u64::from(variant.games) };
    for batch in &variant.transactions {
        add(
            &mut result.base_transaction_fees_lamports,
            transaction_fee(
                batch.signatures,
                batch.cu_limit,
                batch.cu_price_micro_lamports,
            )
            .with_context(|| batch.label.clone())?,
            count(batch.count, batch.shared),
        )?;
    }
    for account in &variant.accounts {
        let rent = account.rent_lamports.with_context(|| {
            format!(
                "{}: supply rent_lamports or use --quote-rent",
                account.label
            )
        })?;
        add(
            &mut result.rent_funded_lamports,
            rent,
            count(account.count, account.shared),
        )?;
        if account.closed {
            add(
                &mut result.rent_refunded_lamports,
                rent,
                count(account.count, account.shared),
            )?;
        }
    }
    let (session_fee, commit_fee) = profile.deposit_rates();
    for delegation in &variant.delegations {
        ensure!(
            delegation.delegated_fee_payer || delegation.normal_commits <= 10,
            "{}: normal commit 11 needs a delegated fee payer and magic_fee_vault",
            delegation.label
        );
        let commits = u64::from(delegation.normal_commits) + u64::from(delegation.final_commit);
        let charge = sum(&[session_fee, commits.saturating_sub(1) * commit_fee])?
            .min(delegation.deposit_lamports);
        let n = count(delegation.count, delegation.shared);
        add(&mut result.delegation_charges_lamports, charge, n)?;
        add(
            &mut result.deposits_funded_lamports,
            delegation.deposit_lamports,
            n,
        )?;
        add(
            &mut result.deposits_refunded_lamports,
            delegation.deposit_lamports - charge,
            n,
        )?;
        if delegation.delegated_fee_payer {
            add(
                &mut result.live_commit_fees_lamports,
                commits.saturating_sub(25) * 100_000,
                n,
            )?;
        }
    }
    for action in &variant.base_actions {
        ensure!(
            !action.requested_cu.is_empty(),
            "Base Action bundle is empty"
        );
        ensure!(
            action.requested_cu.iter().all(|cu| *cu <= 1_400_000),
            "Base Action CU exceeds transaction limit"
        );
        let micro_lamports: u128 = action
            .requested_cu
            .iter()
            .map(|cu| u128::from(*cu) * 50_000)
            .sum();
        let fee = u64::try_from(micro_lamports.div_ceil(1_000_000))?;
        add(
            &mut result.base_action_fees_lamports,
            fee,
            count(action.count, false),
        )?;
    }
    add(
        &mut result.callback_fees_lamports,
        5_000,
        count(variant.callbacks, false),
    )?;
    add(
        &mut result.topup_fees_lamports,
        300_000,
        count(variant.topups, false),
    )?;
    for account in &variant.ephemeral_accounts {
        let storage = (u64::from(account.data_bytes) + 60) * 32;
        add(
            &mut result.ephemeral_storage_funded_lamports,
            storage,
            count(account.count, false),
        )?;
        if account.closed {
            add(
                &mut result.ephemeral_storage_refunded_lamports,
                storage,
                count(account.count, false),
            )?;
        }
    }
    result.nonrefundable_fees_lamports = sum(&[
        result.base_transaction_fees_lamports,
        result.delegation_charges_lamports,
        result.live_commit_fees_lamports,
        result.base_action_fees_lamports,
        result.callback_fees_lamports,
        result.topup_fees_lamports,
    ])?;
    result.retained_storage_lamports = sum(&[
        result.rent_funded_lamports - result.rent_refunded_lamports,
        result.ephemeral_storage_funded_lamports - result.ephemeral_storage_refunded_lamports,
    ])?;
    result.total_funding_lamports = sum(&[
        result.nonrefundable_fees_lamports - result.delegation_charges_lamports,
        result.deposits_funded_lamports,
        result.rent_funded_lamports,
        result.ephemeral_storage_funded_lamports,
    ])?;
    result.fees_per_game_lamports =
        result.nonrefundable_fees_lamports as f64 / f64::from(variant.games);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variant() -> GameVariant {
        serde_json::from_value(serde_json::json!({
            "name":"game", "games":1, "transactions":[], "er_transactions":100,
            "delegations":[{"label":"game", "count":1, "deposit_lamports":30_000_000,
                "normal_commits":0, "final_commit":true, "delegated_fee_payer":false}]
        }))
        .unwrap()
    }

    #[test]
    fn caps_charges_at_deposit_and_returns_remainder() {
        let mut v = variant();
        v.delegations[0].deposit_lamports = 2_000_000;
        let cost = estimate(&v, FeeProfile::Current).unwrap();
        assert_eq!(cost.nonrefundable_fees_lamports, 2_000_000);
        assert_eq!(cost.deposits_refunded_lamports, 0);
        assert_eq!(
            estimate(&v, FeeProfile::Legacy)
                .unwrap()
                .deposits_refunded_lamports,
            1_700_000
        );
    }

    #[test]
    fn enforces_commit_limit_and_prices_commit_26_per_account() {
        let mut v = variant();
        v.delegations[0].normal_commits = 11;
        assert!(estimate(&v, FeeProfile::Current).is_err());
        v.delegations[0].normal_commits = 25;
        v.delegations[0].delegated_fee_payer = true;
        v.delegations[0].count = 2;
        let cost = estimate(&v, FeeProfile::Current).unwrap();
        assert_eq!(cost.delegation_charges_lamports, 56_000_000);
        assert_eq!(cost.live_commit_fees_lamports, 200_000);
        assert_eq!(cost.deposits_refunded_lamports, 4_000_000);
    }

    #[test]
    fn amortizes_shared_setup_and_separates_retained_rent() {
        let mut v = variant();
        v.games = 10;
        v.accounts.push(AccountRent {
            label: "profile".into(),
            data_bytes: 100,
            count: 2,
            rent_lamports: Some(1_000_000),
            closed: false,
            shared: true,
        });
        v.transactions.push(TransactionBatch {
            label: "setup".into(),
            count: 2,
            signatures: 2,
            cu_limit: 200_000,
            cu_price_micro_lamports: 10_001,
            shared: true,
        });
        let cost = estimate(&v, FeeProfile::Current).unwrap();
        assert_eq!(cost.rent_funded_lamports, 2_000_000);
        assert_eq!(cost.retained_storage_lamports, 2_000_000);
        assert_eq!(cost.base_transaction_fees_lamports, 24_002);
        assert_eq!(cost.fees_per_game_lamports, 3_002_400.2);
        assert_eq!(
            cost.total_funding_lamports - cost.deposits_refunded_lamports,
            cost.nonrefundable_fees_lamports + cost.retained_storage_lamports
        );
    }

    #[test]
    fn missing_rent_and_overflow_are_errors() {
        let mut v = variant();
        v.accounts.push(AccountRent {
            label: "game".into(),
            data_bytes: 100,
            count: 1,
            rent_lamports: None,
            closed: true,
            shared: false,
        });
        assert!(estimate(&v, FeeProfile::Current).is_err());
        v.accounts[0].rent_lamports = Some(u64::MAX);
        v.games = 2;
        assert!(estimate(&v, FeeProfile::Current).is_err());
        v.games = 0;
        assert!(estimate(&v, FeeProfile::Current).is_err());
    }

    #[test]
    fn actions_round_per_bundle_and_storage_is_refundable() {
        let mut v = variant();
        v.base_actions.push(BaseAction {
            count: 1,
            requested_cu: vec![1, 1],
        });
        v.callbacks = 1;
        v.topups = 1;
        v.ephemeral_accounts.push(EphemeralAccount {
            count: 1,
            data_bytes: 1000,
            closed: true,
        });
        let cost = estimate(&v, FeeProfile::Current).unwrap();
        assert_eq!(cost.base_action_fees_lamports, 1);
        assert_eq!(cost.callback_fees_lamports, 5000);
        assert_eq!(cost.topup_fees_lamports, 300000);
        assert_eq!(cost.ephemeral_storage_funded_lamports, 33920);
        assert_eq!(cost.ephemeral_storage_refunded_lamports, 33920);
        assert_eq!(cost.retained_storage_lamports, 0);
    }

    #[test]
    fn checked_in_matrix_has_consistent_cashflow() {
        let w: Workload = serde_json::from_str(include_str!("../../scenarios/games.json")).unwrap();
        for variant in &w.variants {
            let cost = estimate(variant, w.fee_profile).unwrap();
            assert_eq!(
                cost.total_funding_lamports
                    - cost.rent_refunded_lamports
                    - cost.deposits_refunded_lamports
                    - cost.ephemeral_storage_refunded_lamports,
                cost.nonrefundable_fees_lamports + cost.retained_storage_lamports
            );
        }
    }
}
