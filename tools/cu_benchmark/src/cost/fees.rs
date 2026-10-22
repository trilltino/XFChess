use anyhow::{ensure, Result};

pub fn priority_fee(cu_limit: u32, micro_lamports: u64) -> Result<u64> {
    let fee = (u128::from(cu_limit) * u128::from(micro_lamports)).div_ceil(1_000_000);
    Ok(u64::try_from(fee)?)
}

pub fn transaction_fee(signatures: u32, cu_limit: u32, micro_lamports: u64) -> Result<u64> {
    ensure!(
        signatures > 0,
        "a paid transaction needs at least one signature"
    );
    ensure!(
        (1..=1_400_000).contains(&cu_limit),
        "CU limit must be 1..=1400000"
    );
    (u64::from(signatures) * 5_000)
        .checked_add(priority_fee(cu_limit, micro_lamports)?)
        .ok_or_else(|| anyhow::anyhow!("transaction fee overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;

    #[test]
    fn prices_requested_budget_and_rounds_up() -> anyhow::Result<()> {
        assert_eq!(
            transaction_fee(2, 200_000, 10_001).context(concat!(file!(), ":", line!()))?,
            12_001
        );
        assert_eq!(
            priority_fee(1, 1).context(concat!(file!(), ":", line!()))?,
            1
        );
        assert!(transaction_fee(0, 200_000, 0).is_err());
        assert!(transaction_fee(1, 1_400_001, 0).is_err());
        Ok(())
    }

    #[test]
    fn rejects_overflow_instead_of_saturating() {
        assert!(priority_fee(1_400_000, u64::MAX).is_err());
    }
}
