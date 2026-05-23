use std::path::Path;
use sqlx::SqlitePool;
use tracing::{info, warn};

use crate::config::AcConfig;
use crate::cross_game;
use crate::report::{json, txt};
use crate::types::{AcReport, Verdict};

pub async fn save_report(pool: &SqlitePool, report: &AcReport, cfg: &AcConfig) -> crate::error::AcResult<()> {
    let report_path = if report.white.verdict != Verdict::Clean
        || report.black.verdict != Verdict::Clean
    {
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
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
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
        report.white.verdict.as_str(), report.white.weighted_score,
        report.black.verdict.as_str(), report.black.weighted_score,
    );

    cross_game::update_stats(pool, &report.white).await;
    cross_game::update_stats(pool, &report.black).await;

    Ok(())
}

fn write_report_files(report: &AcReport, cfg: &AcConfig) -> crate::error::AcResult<String> {
    let dir = Path::new(&cfg.reports_dir);
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }

    let base = dir.join(&report.game_id);
    let txt_path  = base.with_extension("txt");
    let json_path = base.with_extension("json");

    std::fs::write(&txt_path, txt::render(report))?;
    std::fs::write(&json_path, serde_json::to_string_pretty(&json::render(report))?)?;

    info!("[anticheat] report written to {}", txt_path.display());
    Ok(txt_path.to_string_lossy().into_owned())
}
