use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CuEntry {
    pub instruction: String,
    pub cu_consumed: u64,
    pub cu_requested: u64,
    pub success: bool,
    pub signature: Option<String>,
    #[serde(default)]
    pub fee_payer: Option<String>,
    #[serde(default)]
    pub pre_balance_lamports: Option<u64>,
    #[serde(default)]
    pub post_balance_lamports: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct CuStats {
    pub total_cu: u64,
    pub min_cu: u64,
    pub max_cu: u64,
    pub avg_cu: f64,
    pub count: u64,
    pub success_count: u64,
    pub failure_count: u64,
}

#[derive(Debug, Clone, Default)]
pub struct CuLogger {
    entries: Vec<CuEntry>,
    groups: HashMap<String, Vec<CuEntry>>,
}

impl CuLogger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn log(
        &mut self,
        group: &str,
        instruction: &str,
        cu_consumed: u64,
        cu_requested: u64,
        success: bool,
        signature: Option<String>,
    ) {
        self.log_with_balance(
            group,
            instruction,
            cu_consumed,
            cu_requested,
            success,
            signature,
            None,
            None,
            None,
        );
    }

    pub fn log_with_balance(
        &mut self,
        group: &str,
        instruction: &str,
        cu_consumed: u64,
        cu_requested: u64,
        success: bool,
        signature: Option<String>,
        fee_payer: Option<String>,
        pre_balance_lamports: Option<u64>,
        post_balance_lamports: Option<u64>,
    ) {
        let entry = CuEntry {
            instruction: instruction.to_string(),
            cu_consumed,
            cu_requested,
            success,
            signature,
            fee_payer,
            pre_balance_lamports,
            post_balance_lamports,
        };
        self.entries.push(entry.clone());
        self.groups
            .entry(group.to_string())
            .or_default()
            .push(entry);
    }

    pub fn group_stats(&self, group: &str) -> Option<CuStats> {
        let entries = self.groups.get(group)?;
        if entries.is_empty() {
            return None;
        }
        let total: u64 = entries.iter().map(|e| e.cu_consumed).sum();
        let count = entries.len() as u64;
        Some(CuStats {
            total_cu: total,
            min_cu: entries.iter().map(|e| e.cu_consumed).min().unwrap_or(0),
            max_cu: entries.iter().map(|e| e.cu_consumed).max().unwrap_or(0),
            avg_cu: total as f64 / count as f64,
            count,
            success_count: entries.iter().filter(|e| e.success).count() as u64,
            failure_count: entries.iter().filter(|e| !e.success).count() as u64,
        })
    }

    pub fn total_cu(&self) -> u64 {
        self.entries.iter().map(|e| e.cu_consumed).sum()
    }

    pub fn success_count(&self) -> usize {
        self.entries.iter().filter(|e| e.success).count()
    }

    pub fn failure_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.success).count()
    }

    pub fn print_summary(&self) {
        println!("\n╔══════════════════════════════════════════════════════════╗");
        println!("║           CU CONSUMPTION SUMMARY                       ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        println!();
        let mut groups: Vec<_> = self.groups.keys().collect();
        groups.sort();
        println!(
            "{:<30} {:>10} {:>10} {:>10} {:>10}",
            "Group", "Total CU", "Count", "Avg CU", "Success %"
        );
        println!("{}", "─".repeat(80));
        for group in groups {
            if let Some(stats) = self.group_stats(group) {
                let success_pct = if stats.count > 0 {
                    (stats.success_count as f64 / stats.count as f64) * 100.0
                } else {
                    0.0
                };
                println!(
                    "{:<30} {:>10} {:>10} {:>10.0} {:>9.1}%",
                    group, stats.total_cu, stats.count, stats.avg_cu, success_pct
                );
            }
        }
        println!("{}", "─".repeat(80));
        println!(
            "   Grand Total: {} CU | Success: {} | Failures: {}",
            self.total_cu(),
            self.success_count(),
            self.failure_count()
        );
        println!();
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.entries).unwrap_or_default()
    }

    pub fn entries(&self) -> &[CuEntry] {
        &self.entries
    }
}
