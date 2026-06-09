//! Cost estimation and formatted reporting in SOL/GBP.
//!
//! Cost model (MagicBlock public nodes):
//!   - Base fee per TX on ER:           0 lamports (free per-TX)
//!   - Session fee (at undelegation):    300_000 lamports (0.0003 SOL)
//!   - Commit fee (commit_move_batch):   100_000 lamports (0.0001 SOL)
//!   - Base-layer TXs:                   5_000 base + 10_000 priority = 15_000 lamports

use crate::cu_logger::CuLogger;
use crate::{BASE_TX_FEE, LAMPORTS_PER_SOL, SOL_GBP_RATE};

/// MagicBlock ER session fee charged at undelegation (0.0003 SOL).
pub const ER_SESSION_FEE_LAMPORTS: u64 = 300_000;

/// MagicBlock ER commit fee per commit_move_batch (0.0001 SOL).
pub const ER_COMMIT_FEE_LAMPORTS: u64 = 100_000;

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
    /// ER commit fees (0.0001 SOL × number of commit_move_batch calls).
    pub er_commit_fees_sol: f64,
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

    // Base-layer instructions charged at 5_000 (base) + 10_000 (priority) lamports per TX.
    let paid_instructions = [
        "init_profile", "create_game", "join_game", "finalize_game",
        "resign",
        "initialize_tournament", "initialize_tournament_shards", "initialize_tournament_escrow",
        "register_player", "start_tournament",
        "record_match_result", "session_create_game", "session_join_game",
        "record_swiss_result", "authorize_session_key",
        "authorize_tournament_session",
    ];

    // ER instructions that carry a MagicBlock network fee (not per-move, but per-session/commit).
    // undelegate_game  -> 0.0003 SOL session fee
    // commit_move_batch -> 0.0001 SOL commit fee
    let mut paid_tx_count = 0u64;
    let mut er_session_count = 0u64;  // undelegate_game calls
    let mut er_commit_count = 0u64;   // commit_move_batch calls
    let mut paid_breakdown_map: std::collections::HashMap<String, (u64, u64)> =
        std::collections::HashMap::new();

    for entry in logger.entries() {
        if paid_instructions.contains(&entry.instruction.as_str()) {
            paid_tx_count += 1;
            let (total, count) = paid_breakdown_map
                .entry(entry.instruction.clone())
                .or_insert((0, 0));
            *total += entry.cu_consumed;
            *count += 1;
        } else if entry.instruction == "undelegate_game" {
            er_session_count += 1;
        } else if entry.instruction == "commit_move_batch" {
            er_commit_count += 1;
        }
    }

    let base_tx_fees_lamports = paid_tx_count * BASE_TX_FEE;
    let priority_fees_lamports = paid_tx_count * 10_000;
    let er_session_fees_lamports = er_session_count * ER_SESSION_FEE_LAMPORTS;
    let er_commit_fees_lamports = er_commit_count * ER_COMMIT_FEE_LAMPORTS;

    let total_lamports = base_tx_fees_lamports
        + priority_fees_lamports
        + er_session_fees_lamports
        + er_commit_fees_lamports;
    let total_sol = total_lamports as f64 / LAMPORTS_PER_SOL as f64;
    let total_gbp = total_sol * SOL_GBP_RATE;

    let mut breakdown: Vec<InstructionCost> = paid_breakdown_map
        .iter()
        .map(|(instruction, (total_cu, count))| {
            let avg_cu = *total_cu as f64 / *count as f64;
            let tx_fees = *count * (BASE_TX_FEE + 10_000);
            let est_sol = tx_fees as f64 / LAMPORTS_PER_SOL as f64;
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
        er_commit_fees_sol: er_commit_fees_lamports as f64 / LAMPORTS_PER_SOL as f64,
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
    println!("   Free ER Transactions:  {}", report.tx_count - report.paid_tx_count);
    println!();
    println!("   Fee Breakdown:");
    println!("     Base TX Fees:        {:.6} SOL  (5000 lam × paid txs)", report.base_tx_fees_sol);
    println!("     Priority Fees:       {:.6} SOL  (10000 lam × paid txs)", report.priority_fees_sol);
    println!("     ER Session Fees:     {:.6} SOL  (0.0003 SOL × undelegations)", report.er_session_fees_sol);
    println!("     ER Commit Fees:      {:.6} SOL  (0.0001 SOL × commits)", report.er_commit_fees_sol);
    println!();
    println!("   ╔══════════════════════════════════════════════════════╗");
    println!("   ║  TOTAL ESTIMATED COST:  {:>12.6} SOL            ║", report.estimated_sol);
    println!("   ║                        ({:>12.2} GBP)            ║", report.estimated_gbp);
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
