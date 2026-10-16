use crate::types::{AcReport, Complexity, GameContext, GameResult, SideAnalysis, TimingSource};

pub fn render(report: &AcReport) -> String {
    let mut out = String::new();
    let sep = "═".repeat(70);
    let thin = "─".repeat(70);

    out.push_str(&format!("{sep}\nXFCHESS ANTI-CHEAT REPORT\n{sep}\n"));
    out.push_str(&format!("Game ID   : {}\n", report.game_id));

    let ctx = match &report.context {
        GameContext::Pvp { wager_sol } => format!("PvP (wager: {wager_sol:.3} SOL)"),
        GameContext::Tournament {
            tournament_id,
            round,
        } => format!("Tournament #{tournament_id} — Round {round}"),
    };
    out.push_str(&format!("Context   : {ctx}\n"));

    let result_str = match report.result {
        GameResult::WhiteWin => "1-0",
        GameResult::BlackWin => "0-1",
        GameResult::Draw => "½-½",
    };
    out.push_str(&format!("Result    : {result_str}\n"));
    out.push_str(&format!(
        "Engine    : {}  depth={}\n",
        report.engine_version, report.analysis_depth
    ));
    out.push_str(&format!("{thin}\n"));

    for (label, side) in [("WHITE", &report.white), ("BLACK", &report.black)] {
        render_side(&mut out, label, side, &thin);
    }

    out.push_str(&format!("{sep}\n"));
    out.push_str("NOTE: This is a server-generated statistical analysis.\n");
    out.push_str("      A human reviewer makes all final determinations.\n");
    out.push_str("      No automated action has been taken.\n");
    out.push_str(&format!("{sep}\n"));
    out
}

fn render_side(out: &mut String, label: &str, side: &SideAnalysis, thin: &str) {
    out.push_str(&format!(
        "{label:5} : {}  ELO: {}\n",
        &side.pubkey[..12.min(side.pubkey.len())],
        side.elo
    ));
    out.push_str(&format!(
        "VERDICT   : {}  (score: {:.2})\n",
        side.verdict.as_str(),
        side.weighted_score
    ));
    out.push_str(&format!("{thin}\n"));
    let timing_src = match side.signals.timing_source {
        TimingSource::Client => "client think-time",
        TimingSource::Server => "server timestamps",
        TimingSource::None => "DISABLED (untrusted/batched)",
    };
    out.push_str(&format!(
        "  timing_anomaly : {:.3}  [wt 0.40]  source: {timing_src}\n",
        side.signals.timing_anomaly
    ));
    out.push_str(&format!(
        "  cpl_vs_elo     : {:.3}  [wt 0.35]  avg_cpl={:.1}\n",
        side.signals.cpl_vs_elo, side.signals.avg_cpl
    ));
    out.push_str(&format!(
        "  t1_rate        : {:.3}  [wt 0.25]  ({} complex plies)\n",
        side.signals.t1_rate, side.signals.complex_ply_count
    ));
    out.push_str(&format!(
        "  blur_rate      : {:.3}  [wt 0.15]\n",
        side.signals.blur_rate
    ));
    out.push_str(&format!("{thin}\n"));

    // Per-ply table header
    out.push_str(&format!(
        "{:>4}  {:<6}  {:>8}  {:>7}  {:>5}  {:<3}  {:>8}  {:<10}  {}\n",
        "PLY", "MOVE", "PLAYED_CP", "TOP1_CP", "CPL", "T1?", "TIME(ms)", "COMPLEXITY", "FLAGS"
    ));

    for eval in &side.ply_evals {
        let t1 = if eval.is_t1 { "YES" } else { "NO " };
        let cplx = match eval.complexity {
            Complexity::Forced => "Forced    ",
            Complexity::Simple => "Simple    ",
            Complexity::Complex => "Complex   ",
        };
        let played_cp = eval.top1_cp - eval.cpl;
        let flag = if eval.complexity == Complexity::Complex && eval.latency_ms < 2_000 {
            "*** FAST"
        } else {
            ""
        };
        out.push_str(&format!(
            "{:>4}  {:<6}  {:>8}  {:>7}  {:>5}  {}  {:>8}  {}  {}\n",
            eval.ply,
            eval.move_uci,
            played_cp,
            eval.top1_cp,
            eval.cpl,
            t1,
            eval.latency_ms,
            cplx,
            flag,
        ));
    }
    out.push_str(&format!("{thin}\n"));
}
