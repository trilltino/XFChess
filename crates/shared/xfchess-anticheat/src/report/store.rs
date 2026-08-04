use sqlx::SqlitePool;
use std::path::Path;
use tracing::{info, warn};

use crate::config::AcConfig;
use crate::cross_game;
use crate::report::{json, txt};
use crate::types::{AcReport, SideAnalysis, Verdict};

/// Z-score threshold (standard deviations below a player's own rolling CPL
/// average) above which a game is logged as cross-game advisory evidence.
/// ~2σ is the point a single-game result stops looking like normal variance
/// against that player's own history — a common significance threshold, not
/// tuned against real data yet (see `elo_baseline.rs` for how the ELO-based
/// signal *was* fitted). Advisory only: never adjusts `weighted_score` or
/// `verdict`.
const CROSS_GAME_Z_ALERT: f64 = 2.0;

pub async fn save_report(
    pool: &SqlitePool,
    report: &AcReport,
    cfg: &AcConfig,
) -> crate::error::AcResult<()> {
    let report_path =
        if report.white.verdict != Verdict::Clean || report.black.verdict != Verdict::Clean {
            Some(write_report_files(report, cfg)?)
        } else {
            None
        };

    let path_str = report_path.as_deref().unwrap_or("");
    let white_signals = serde_json::to_string(&report.white.signals)?;
    let black_signals = serde_json::to_string(&report.black.signals)?;

    sqlx::query(
        r#"INSERT OR REPLACE INTO anticheat_verdicts
           (game_id, engine_version, analysis_depth,
            white_pubkey, black_pubkey,
            white_verdict, black_verdict,
            white_score, black_score,
            white_signals, black_signals, report_path)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&report.game_id)
    .bind(&report.engine_version)
    .bind(report.analysis_depth as i64)
    .bind(&report.white.pubkey)
    .bind(&report.black.pubkey)
    .bind(report.white.verdict.as_str())
    .bind(report.black.verdict.as_str())
    .bind(report.white.weighted_score)
    .bind(report.black.weighted_score)
    .bind(&white_signals)
    .bind(&black_signals)
    .bind(path_str)
    .execute(pool)
    .await?;

    info!(
        "[anticheat] game {} — white: {} ({:.2}), black: {} ({:.2})",
        report.game_id,
        report.white.verdict.as_str(),
        report.white.weighted_score,
        report.black.verdict.as_str(),
        report.black.weighted_score,
    );

    // Compare against each player's own rolling baseline *before* this game
    // gets folded into it below.
    log_cross_game_evidence(pool, &report.white).await;
    log_cross_game_evidence(pool, &report.black).await;

    cross_game::update_stats(pool, &report.white).await;
    cross_game::update_stats(pool, &report.black).await;

    Ok(())
}

/// Weighs this game's CPL against the player's own rolling history
/// (`cross_game::load_stats`/`game_z_score`) and logs it when it stands out —
/// advisory evidence for the dispute flow, same as the rest of this crate's
/// output. Deliberately does not touch `weighted_score`/`verdict`: this is
/// the read side of data `update_stats` has been persisting every game, so
/// it's now actually used instead of only ever written.
async fn log_cross_game_evidence(pool: &SqlitePool, side: &SideAnalysis) {
    let stats = cross_game::load_stats(pool, &side.pubkey).await;
    if !stats.has_sufficient_history() {
        return;
    }
    let z = stats.game_z_score(side.signals.avg_cpl);
    if z >= CROSS_GAME_Z_ALERT {
        warn!(
            "[anticheat] {} — this game's avg CPL ({:.1}) is {:.1}σ below their own \
             {}-game rolling average ({:.1}); unusually strong relative to their own \
             history, independent of the ELO-baseline signal",
            side.pubkey,
            side.signals.avg_cpl,
            z,
            stats.last_30_cpls.len(),
            stats.rolling_avg_cpl(),
        );
    }
}

fn write_report_files(report: &AcReport, cfg: &AcConfig) -> crate::error::AcResult<String> {
    let dir = Path::new(&cfg.reports_dir);
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }

    let base = dir.join(&report.game_id);
    let txt_path = base.with_extension("txt");
    let json_path = base.with_extension("json");

    std::fs::write(&txt_path, txt::render(report))?;
    std::fs::write(
        &json_path,
        serde_json::to_string_pretty(&json::render(report))?,
    )?;

    info!("[anticheat] report written to {}", txt_path.display());
    Ok(txt_path.to_string_lossy().into_owned())
}
