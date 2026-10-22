use crate::cost::measurements::{ExecutionLayer, Measurement, MeasurementLog};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct CostReport {
    pub scenario: String,
    pub submitted_transactions: usize,
    pub skipped_operations: usize,
    pub measured_transactions: usize,
    pub failed_transactions: usize,
    pub measured_cu: u64,
    pub solana_transaction_fees_lamports: u64,
    pub er_transaction_fees_lamports: u64,
    pub per_payer_net_outflow_lamports: BTreeMap<String, i128>,
    pub warnings: Vec<String>,
    pub transactions: Vec<Measurement>,
}

pub fn generate_cost_report(logger: &MeasurementLog, scenario: &str) -> CostReport {
    let mut report = CostReport {
        scenario: scenario.into(), submitted_transactions: 0, skipped_operations: 0,
        measured_transactions: 0, failed_transactions: 0, measured_cu: logger.total_cu(),
        solana_transaction_fees_lamports: 0, er_transaction_fees_lamports: 0,
        per_payer_net_outflow_lamports: BTreeMap::new(),
        warnings: vec![
            "Fees cover recorded transactions only; wallet funding, reclamation and unrecorded setup are excluded.".into(),
            "Delegation deposit charges, rent and wagers are not transaction fees; use an explicit workload to estimate them.".into(),
            "Payer cashflow includes transfers and storage and must not be interpreted as a game fee.".into(),
        ],
        transactions: logger.entries().to_vec(),
    };
    for entry in logger.entries() {
        if entry.signature.is_none() {
            report.skipped_operations += 1;
            continue;
        }
        report.submitted_transactions += 1;
        if let Some(fee) = entry.fee_lamports {
            report.measured_transactions += 1;
            report.failed_transactions += usize::from(!entry.success);
            match entry.layer {
                ExecutionLayer::Solana => report.solana_transaction_fees_lamports += fee,
                ExecutionLayer::Er => report.er_transaction_fees_lamports += fee,
            }
        }
        if let (Some(payer), Some(pre), Some(post)) = (
            &entry.fee_payer,
            entry.pre_balance_lamports,
            entry.post_balance_lamports,
        ) {
            *report
                .per_payer_net_outflow_lamports
                .entry(payer.clone())
                .or_default() += i128::from(pre) - i128::from(post);
        }
    }
    if report.measured_transactions < report.submitted_transactions {
        report
            .warnings
            .push("Transaction metadata coverage is incomplete; fee totals are partial.".into());
    }
    report
}

pub fn print_cost_report(report: &CostReport) {
    println!(
        "\n{}: {}/{} transactions measured, {} skipped operations",
        report.scenario,
        report.measured_transactions,
        report.submitted_transactions,
        report.skipped_operations
    );
    println!(
        "Measured Solana fees: {} lamports; ER transaction fees: {} lamports",
        report.solana_transaction_fees_lamports, report.er_transaction_fees_lamports
    );
    for warning in &report.warnings {
        println!("  {warning}");
    }
}

pub fn export_json(report: &CostReport) -> serde_json::Result<String> {
    serde_json::to_string_pretty(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;

    #[test]
    fn unknown_metadata_is_partial_and_skips_are_not_paid_transactions() {
        let mut logger = MeasurementLog::new();
        logger.log(ExecutionLayer::Solana, "setup", "profile", true, None);
        logger.log(
            ExecutionLayer::Er,
            "moves",
            "move",
            true,
            Some("signature".into()),
        );
        let report = generate_cost_report(&logger, "test");
        assert_eq!(report.submitted_transactions, 1);
        assert_eq!(report.skipped_operations, 1);
        assert_eq!(report.measured_cu, 0);
        assert_eq!(report.measured_transactions, 0);
        assert!(report.warnings.iter().any(|s| s.contains("partial")));
    }

    #[test]
    fn includes_failed_fees_and_keeps_cashflow_separate() -> anyhow::Result<()> {
        let entries: Vec<Measurement> = serde_json::from_value(serde_json::json!([
            {"group":"setup", "instruction":"failed", "layer":"solana",
             "cu_consumed":123, "cu_limit":200000, "cu_price_micro_lamports":10000,
             "fee_lamports":12000, "success":false, "signature":"base",
             "fee_payer":"alice", "pre_balance_lamports":1000000, "post_balance_lamports":988000},
            {"group":"moves", "instruction":"record", "layer":"er",
             "cu_consumed":40000, "cu_limit":80000, "cu_price_micro_lamports":0,
             "fee_lamports":0, "success":true, "signature":"er",
             "fee_payer":"alice", "pre_balance_lamports":988000, "post_balance_lamports":1008000}
        ]))
        .context(concat!(file!(), ":", line!()))?;
        let log = MeasurementLog::from(entries);
        let report = generate_cost_report(&log, "test");
        assert_eq!(report.measured_transactions, 2);
        assert_eq!(report.failed_transactions, 1);
        assert_eq!(report.solana_transaction_fees_lamports, 12000);
        assert_eq!(report.er_transaction_fees_lamports, 0);
        assert_eq!(report.per_payer_net_outflow_lamports["alice"], -8000);
        assert_eq!(report.measured_cu, 40123);
        Ok(())
    }
}
