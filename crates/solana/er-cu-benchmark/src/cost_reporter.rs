//! Cost estimation and formatted reporting in SOL/GBP.
//!
//! Cost model (MagicBlock public nodes):
//!   - Base fee per TX on ER:           0 lamports (free per-TX)
//!   - Session fee (at undelegation):    300_000 lamports (0.0003 SOL)
//!   - Base-layer TXs:                   5_000 base + CU-priced priority fee
//!
//! There used to be a "commit fee (commit_move_batch)" line here too, but
//! `commit_move_batch` is not a real instruction in the production program —
//! it never had a handler, and `game_flows.rs`'s own 1v1 flow labels that
//! step "Bypassed in event-based architecture". Removed rather than kept as
//! an always-zero line item that implied a cost path that doesn't exist.

use crate::cu_logger::CuLogger;
use crate::{sol_gbp_rate, BASE_TX_FEE, DEFAULT_CU_PRICE, LAMPORTS_PER_SOL};
use std::collections::HashMap;

/// MagicBlock ER session fee charged at undelegation (0.0003 SOL). Mirrors
/// `ER_SESSION_FEE_LAMPORTS` in `programs/xfchess-game/src/constants.rs`,
/// which is now actually accrued into `Game.fees_advanced` at undelegation
/// (`lifecycle::transitions::mark_undelegated`) — keep the two in sync.
pub const ER_SESSION_FEE_LAMPORTS: u64 = 300_000;

/// Modeled (not measured — see `docs/plans/tournament-production-readiness-plan.md`
/// F9/WS-D.1b) rent-exemption cost per account created by each instruction,
/// from the program's own account layouts via Solana's rent-exempt formula
/// `(128 + data_len) × 6960` lamports. Corroborated by the program's own
/// comment on shard rent ("1 shard, ~0.034 SOL",
/// `programs/xfchess-game/src/tournament_ix/lifecycle/initialize_shards.rs`).
/// Instructions not listed here create no new account (or one negligible
/// enough not to matter for a working-capital estimate) and contribute 0.
/// Kept in sync with `backend/src/signing/routes/admin.rs::tournament_cost_estimate`,
/// which needs the same numbers for the pre-flight affordability gate.
fn modeled_rent_lamports(instruction: &str) -> u64 {
    match instruction {
        "initialize_shards_small" | "initialize_shards_medium" | "initialize_shards" => 34_100_000,
        "initialize_match" => 2_170_000,
        "session_create_game" | "create_game" | "global_create_game" => 3_400_000,
        "initialize_tournament" | "initialize_escrow" => 2_000_000,
        _ => 0,
    }
}

/// Instructions that return previously-paid rent to the payer (a close/finalize
/// step), used to derive `net_rent_sol` = `gross_rent_sol` − refunds. These
/// don't reliably map 1:1 to which specific rent line they refund without
/// reading close-account instructions in detail, so refunds are tracked in
/// aggregate rather than per-account-type.
fn is_rent_refunding_instruction(instruction: &str) -> bool {
    matches!(
        instruction,
        "close_tournament" | "finalize_game" | "session_finalize_game" | "global_finalize_game"
    )
}

/// A cost report for a test scenario.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CostReport {
    pub scenario: String,
    pub total_cu: u64,
    pub tx_count: u64,
    pub paid_tx_count: u64,
    pub estimated_sol: f64,
    pub estimated_gbp: f64,
    pub base_tx_fees_sol: f64,
    pub priority_fees_sol: f64,
    /// ER session fees (0.0003 SOL × number of undelegate_game calls).
    pub er_session_fees_sol: f64,
    /// Measured fee-payer balance movement when snapshots were captured.
    pub measured_outflow_sol: f64,
    pub measured_inflow_sol: f64,
    /// Modeled (not measured) rent-exemption cost for every account-creating
    /// instruction seen — see `modeled_rent_lamports`. This is the dominant
    /// cost for a tournament and was previously omitted entirely (F9.1).
    pub modeled_rent_gross_sol: f64,
    /// `modeled_rent_gross_sol` minus rent recovered by any
    /// rent-refunding instruction observed in the same run (F9.2). This is
    /// still modeled, not measured — a real refund amount depends on the
    /// specific account being closed, which this crate doesn't track per-PDA.
    pub modeled_rent_net_sol: f64,
    /// Per fee-payer breakdown of measured lamport movement (F9's
    /// "per-signer attribution" ask) — key is the fee payer pubkey string,
    /// or `"unknown"` for entries logged without `log_with_balance`.
    pub per_signer_outflow_sol: HashMap<String, f64>,
    pub breakdown: Vec<InstructionCost>,
}

/// Cost breakdown per instruction type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstructionCost {
    pub instruction: String,
    pub count: u64,
    pub total_cu: u64,
    pub avg_cu: f64,
    pub estimated_sol: f64,
}

/// Generate a cost report from a CU logger.
pub fn generate_cost_report(logger: &CuLogger, scenario: &str) -> CostReport {
    let total_cu = logger.total_cu();
    let tx_count = logger.entries().len() as u64;

    // Everything is paid by default on the base layer. Only transactions known to
    // execute on ER are excluded, so adding a new base-layer instruction cannot
    // silently make the report undercount.
    let er_free_instructions = ["record_move", "undelegate_game"];

    let mut paid_tx_count = 0u64;
    let mut er_session_count = 0u64; // undelegate_game calls
    let mut paid_breakdown_map: std::collections::HashMap<String, (u64, u64)> =
        std::collections::HashMap::new();
    let mut priority_fees_lamports = 0u64;
    let mut measured_outflow_lamports = 0u64;
    let mut measured_inflow_lamports = 0u64;
    let mut modeled_rent_gross_lamports = 0u64;
    let mut rent_refund_events = 0u64;
    let mut per_signer_outflow_lamports: HashMap<String, i128> = HashMap::new();

    for entry in logger.entries() {
        if let (Some(pre), Some(post)) = (entry.pre_balance_lamports, entry.post_balance_lamports) {
            if pre > post {
                measured_outflow_lamports += pre - post;
            } else {
                measured_inflow_lamports += post - pre;
            }
            let signer = entry
                .fee_payer
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            *per_signer_outflow_lamports.entry(signer).or_insert(0) += pre as i128 - post as i128;
        }
        modeled_rent_gross_lamports += modeled_rent_lamports(&entry.instruction);
        if is_rent_refunding_instruction(&entry.instruction) {
            rent_refund_events += 1;
        }
        if !er_free_instructions.contains(&entry.instruction.as_str()) {
            paid_tx_count += 1;
            let (total, count) = paid_breakdown_map
                .entry(entry.instruction.clone())
                .or_insert((0, 0));
            *total += entry.cu_consumed;
            *count += 1;
            priority_fees_lamports +=
                DEFAULT_CU_PRICE.saturating_mul(entry.cu_consumed) / 1_000_000;
        } else if entry.instruction == "undelegate_game" {
            er_session_count += 1;
        }
    }

    let base_tx_fees_lamports = paid_tx_count * BASE_TX_FEE;
    let er_session_fees_lamports = er_session_count * ER_SESSION_FEE_LAMPORTS;

    // Rough net-rent model: assume each refund event returns the average
    // per-account rent observed this run. This is a coarse approximation
    // (real refunds vary by which specific account closed) — flagged
    // "modeled" throughout rather than presented as measured.
    let distinct_rent_accounts = paid_breakdown_map
        .keys()
        .filter(|i| modeled_rent_lamports(i) > 0)
        .count()
        .max(1) as u64;
    let avg_rent_per_account = modeled_rent_gross_lamports / distinct_rent_accounts;
    let modeled_rent_refund_lamports = rent_refund_events.saturating_mul(avg_rent_per_account);
    let modeled_rent_net_lamports =
        modeled_rent_gross_lamports.saturating_sub(modeled_rent_refund_lamports);

    let per_signer_outflow_sol: HashMap<String, f64> = per_signer_outflow_lamports
        .into_iter()
        .map(|(signer, lamports)| (signer, lamports as f64 / LAMPORTS_PER_SOL as f64))
        .collect();

    // estimated_sol/estimated_gbp now include modeled net rent (F9.1) — this
    // was the dominant cost previously missing from the total entirely.
    let total_lamports = base_tx_fees_lamports
        + priority_fees_lamports
        + er_session_fees_lamports
        + modeled_rent_net_lamports;
    let total_sol = total_lamports as f64 / LAMPORTS_PER_SOL as f64;
    let total_gbp = total_sol * sol_gbp_rate();

    let mut breakdown: Vec<InstructionCost> = paid_breakdown_map
        .iter()
        .map(|(instruction, (total_cu, count))| {
            let avg_cu = *total_cu as f64 / *count as f64;
            let base_fees = *count * BASE_TX_FEE;
            let priority_fees = DEFAULT_CU_PRICE.saturating_mul(*total_cu) / 1_000_000;
            let est_sol = (base_fees + priority_fees) as f64 / LAMPORTS_PER_SOL as f64;
            InstructionCost {
                instruction: instruction.clone(),
                count: *count,
                total_cu: *total_cu,
                avg_cu,
                estimated_sol: est_sol,
            }
        })
        .collect();

    breakdown.sort_by(|a, b| b.estimated_sol.partial_cmp(&a.estimated_sol).unwrap());

    CostReport {
        scenario: scenario.to_string(),
        total_cu,
        tx_count,
        paid_tx_count,
        estimated_sol: total_sol,
        estimated_gbp: total_gbp,
        base_tx_fees_sol: base_tx_fees_lamports as f64 / LAMPORTS_PER_SOL as f64,
        priority_fees_sol: priority_fees_lamports as f64 / LAMPORTS_PER_SOL as f64,
        er_session_fees_sol: er_session_fees_lamports as f64 / LAMPORTS_PER_SOL as f64,
        measured_outflow_sol: measured_outflow_lamports as f64 / LAMPORTS_PER_SOL as f64,
        measured_inflow_sol: measured_inflow_lamports as f64 / LAMPORTS_PER_SOL as f64,
        modeled_rent_gross_sol: modeled_rent_gross_lamports as f64 / LAMPORTS_PER_SOL as f64,
        modeled_rent_net_sol: modeled_rent_net_lamports as f64 / LAMPORTS_PER_SOL as f64,
        per_signer_outflow_sol,
        breakdown,
    }
}

/// Print a formatted cost report.
pub fn print_cost_report(report: &CostReport) {
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║           COST ESTIMATION REPORT                       ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("   Scenario: {}", report.scenario);
    println!();
    println!("   Total CU Consumed:     {}", report.total_cu);
    println!("   Total Transactions:    {}", report.tx_count);
    println!("   Paid Transactions:     {}", report.paid_tx_count);
    println!(
        "   Free ER Transactions:  {}",
        report.tx_count - report.paid_tx_count
    );
    println!();
    println!("   Fee Breakdown:");
    println!(
        "     Base TX Fees:        {:.6} SOL  (5000 lam × paid txs)",
        report.base_tx_fees_sol
    );
    println!(
        "     Priority Fees:       {:.6} SOL  (cu_price × cu_used ÷ 1e6, per tx)",
        report.priority_fees_sol
    );
    println!(
        "     ER Session Fees:     {:.6} SOL  (0.0003 SOL × undelegations)",
        report.er_session_fees_sol
    );
    println!(
        "     Rent (modeled, gross): {:.6} SOL  — dominant cost, see F9 in tournament-production-readiness-plan.md",
        report.modeled_rent_gross_sol
    );
    println!(
        "     Rent (modeled, net):   {:.6} SOL  (after modeled refunds)",
        report.modeled_rent_net_sol
    );
    println!(
        "     Measured Outflow:    {:.6} SOL  (balance snapshots, fee+rent combined)",
        report.measured_outflow_sol
    );
    if !report.per_signer_outflow_sol.is_empty() {
        println!("   Per-Signer Outflow (measured):");
        let mut signers: Vec<_> = report.per_signer_outflow_sol.iter().collect();
        signers.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
        for (signer, sol) in signers {
            println!("     {:<44} {:>12.6} SOL", signer, sol);
        }
    }
    println!();
    println!("   ╔══════════════════════════════════════════════════════╗");
    println!(
        "   ║  TOTAL ESTIMATED COST:  {:>12.6} SOL            ║",
        report.estimated_sol
    );
    println!(
        "   ║                        ({:>12.2} GBP)            ║",
        report.estimated_gbp
    );
    println!("   ╚══════════════════════════════════════════════════════╝");
    println!();

    if !report.breakdown.is_empty() {
        println!("   Per-Instruction Breakdown (paid only):");
        println!(
            "     {:<30} {:>6} {:>12} {:>12} {:>12}",
            "Instruction", "Count", "Total CU", "Avg CU", "Est SOL"
        );
        println!("     {}", "─".repeat(80));
        for item in &report.breakdown {
            println!(
                "     {:<30} {:>6} {:>12} {:>12.0} {:>12.6}",
                item.instruction, item.count, item.total_cu, item.avg_cu, item.estimated_sol
            );
        }
        println!();
    }
}

/// Export the cost report as JSON.
pub fn export_json(report: &CostReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cu_logger::CuLogger;

    #[test]
    fn charges_unknown_base_instruction_and_cu_priced_priority_fee() {
        let mut logger = CuLogger::new();
        logger.log("test", "new_instruction", 200_000, 1_400_000, true, None);
        logger.log("test", "record_move", 1_000_000, 1_400_000, true, None);
        logger.log("test", "undelegate_game", 50_000, 1_400_000, true, None);

        let report = generate_cost_report(&logger, "cost-regression");

        assert_eq!(report.tx_count, 3);
        assert_eq!(report.paid_tx_count, 1);
        assert_eq!(report.base_tx_fees_sol, 5_000.0 / LAMPORTS_PER_SOL as f64);
        assert_eq!(report.priority_fees_sol, 2_000.0 / LAMPORTS_PER_SOL as f64);
        assert_eq!(
            report.er_session_fees_sol,
            ER_SESSION_FEE_LAMPORTS as f64 / LAMPORTS_PER_SOL as f64
        );
        assert_eq!(report.estimated_sol, 307_000.0 / LAMPORTS_PER_SOL as f64);
    }

    #[test]
    fn rent_and_refunds_are_modeled_and_netted() {
        let mut logger = CuLogger::new();
        logger.log("test", "initialize_match", 50_000, 200_000, true, None);
        logger.log("test", "close_tournament", 50_000, 200_000, true, None);

        let report = generate_cost_report(&logger, "rent-model");

        assert_eq!(
            report.modeled_rent_gross_sol,
            2_170_000.0 / LAMPORTS_PER_SOL as f64
        );
        // One refund event against one distinct rent-bearing instruction ->
        // net rent is fully offset.
        assert_eq!(report.modeled_rent_net_sol, 0.0);
        // estimated_sol includes net rent, not gross.
        assert!(
            report.estimated_sol
                < report.modeled_rent_gross_sol
                    + report.base_tx_fees_sol
                    + report.priority_fees_sol
        );
    }

    #[test]
    fn per_signer_outflow_is_attributed_by_fee_payer() {
        let mut logger = CuLogger::new();
        logger.log_with_balance(
            "test",
            "initialize_match",
            50_000,
            200_000,
            true,
            None,
            Some("alice".to_string()),
            Some(1_000_000_000),
            Some(997_830_000),
        );
        logger.log_with_balance(
            "test",
            "initialize_match",
            50_000,
            200_000,
            true,
            None,
            Some("bob".to_string()),
            Some(2_000_000_000),
            Some(1_999_995_000),
        );

        let report = generate_cost_report(&logger, "per-signer");

        assert_eq!(report.per_signer_outflow_sol.len(), 2);
        assert!((report.per_signer_outflow_sol["alice"] - 0.00217).abs() < 1e-9);
        assert!((report.per_signer_outflow_sol["bob"] - 0.000005).abs() < 1e-9);
    }
}
