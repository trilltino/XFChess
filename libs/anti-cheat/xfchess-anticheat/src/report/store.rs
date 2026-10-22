use sqlx::{SqliteConnection, SqlitePool};
use std::path::Path;
use tracing::{info, warn};

use crate::config::AcConfig;
use crate::cross_game;
use crate::report::{json, txt};
use crate::types::{AcReport, SideAnalysis, Verdict};

const CROSS_GAME_Z_ALERT: f64 = 2.0;

pub async fn save_report(
    pool: &SqlitePool,
    report: &AcReport,
    cfg: &AcConfig,
) -> crate::error::AcResult<()> {
    let report_path =
        if report.white.verdict != Verdict::Clean || report.black.verdict != Verdict::Clean {
            Some(write_report_files(report, cfg).await?)
        } else {
            None
        };

    let path_str = report_path.as_deref().unwrap_or("");
    let white_signals = serde_json::to_string(&report.white.signals)?;
    let black_signals = serde_json::to_string(&report.black.signals)?;
    let mut transaction = pool.begin().await?;
    let existing: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM anticheat_verdicts WHERE game_id = ?")
            .bind(&report.game_id)
            .fetch_one(&mut *transaction)
            .await?;

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
    .execute(&mut *transaction)
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
    if existing == 0 {
        log_cross_game_evidence(&mut transaction, &report.white).await?;
        log_cross_game_evidence(&mut transaction, &report.black).await?;
        cross_game::update_stats_on(&mut transaction, &report.white).await?;
        cross_game::update_stats_on(&mut transaction, &report.black).await?;
    }
    transaction.commit().await?;

    Ok(())
}

async fn log_cross_game_evidence(
    connection: &mut SqliteConnection,
    side: &SideAnalysis,
) -> crate::error::AcResult<()> {
    let stats = cross_game::load_stats_on(connection, &side.pubkey).await?;
    if !stats.has_sufficient_history() {
        return Ok(());
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
    Ok(())
}

async fn write_report_files(report: &AcReport, cfg: &AcConfig) -> crate::error::AcResult<String> {
    if report.game_id.is_empty()
        || !report
            .game_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(crate::error::AcError::InvalidInput(
            "game ID is not a report filename".into(),
        ));
    }
    let dir = Path::new(&cfg.reports_dir);
    tokio::fs::create_dir_all(dir).await?;

    let base = dir.join(&report.game_id);
    let txt_path = base.with_extension("txt");
    let json_path = base.with_extension("json");

    tokio::fs::write(&txt_path, txt::render(report)).await?;
    tokio::fs::write(
        &json_path,
        serde_json::to_string_pretty(&json::render(report))?,
    )
    .await?;

    info!("[anticheat] report written to {}", txt_path.display());
    Ok(txt_path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GameContext, GameResult, SignalValues, TimingSource};

    fn side(player: &str) -> SideAnalysis {
        SideAnalysis {
            pubkey: player.into(),
            elo: 1_500,
            signals: SignalValues {
                timing_anomaly: 0.0,
                cpl_vs_elo: 0.0,
                t1_rate: 0.5,
                avg_cpl: 20.0,
                complex_ply_count: 10,
                blur_rate: 0.0,
                timing_source: TimingSource::Server,
            },
            weighted_score: 0.0,
            verdict: Verdict::Clean,
            ply_evals: vec![],
        }
    }

    #[tokio::test]
    async fn retries_do_not_double_count_and_stats_failure_rolls_back_verdict() -> anyhow::Result<()>
    {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        // Fresh-test schema is checked-in SQL; CREATEs include upgrade columns.
        let schema = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../backend/migrations/session/006_anticheat.sql"
        ))
        .lines()
        .filter(|line| !line.trim_start().starts_with("ALTER TABLE "))
        .collect::<Vec<_>>()
        .join("\n");
        sqlx::raw_sql(sqlx::AssertSqlSafe(schema.as_str()))
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO anticheat_queue (game_id) VALUES ('42')")
            .execute(&pool)
            .await?;
        let report = AcReport {
            game_id: "42".into(),
            context: GameContext::Pvp { wager_sol: 0.0 },
            result: GameResult::Draw,
            white: side("white"),
            black: side("black"),
            engine_version: "fake".into(),
            analysis_depth: 1,
            analysed_at_ms: 0,
        };
        crate::engine::job_queue::save_and_complete(&pool, &report, &AcConfig::default()).await?;
        crate::engine::job_queue::save_and_complete(&pool, &report, &AcConfig::default()).await?;
        let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM anticheat_queue")
            .fetch_one(&pool)
            .await?;
        assert_eq!(queued, 0);
        assert_eq!(
            cross_game::load_stats(&pool, "white").await?.games_analysed,
            1
        );
        sqlx::query("DROP TABLE player_anticheat_stats")
            .execute(&pool)
            .await?;
        let mut next = report;
        next.game_id = "43".into();
        sqlx::query("INSERT INTO anticheat_queue (game_id) VALUES ('43')")
            .execute(&pool)
            .await?;
        assert!(
            crate::engine::job_queue::save_and_complete(&pool, &next, &AcConfig::default())
                .await
                .is_err()
        );
        let queued: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM anticheat_queue WHERE game_id = '43'")
                .fetch_one(&pool)
                .await?;
        assert_eq!(queued, 1);
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM anticheat_verdicts WHERE game_id = '43'")
                .fetch_one(&pool)
                .await?;
        assert_eq!(count, 0);
        Ok(())
    }
}
