use crate::types::AcReport;

/// Stable JSON schema — version field lets consumers detect breaking changes.
pub fn render(report: &AcReport) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "game_id": report.game_id,
        "context": report.context,
        "result": report.result,
        "engine": {
            "name": report.engine_version,
            "depth": report.analysis_depth,
        },
        "analysed_at_ms": report.analysed_at_ms,
        "white": side_json(&report.white),
        "black": side_json(&report.black),
    })
}

fn side_json(side: &crate::types::SideAnalysis) -> serde_json::Value {
    serde_json::json!({
        "pubkey": side.pubkey,
        "elo": side.elo,
        "verdict": side.verdict.as_str(),
        "weighted_score": side.weighted_score,
        "signals": {
            "timing_anomaly": side.signals.timing_anomaly,
            "timing_source": side.signals.timing_source,
            "cpl_vs_elo": side.signals.cpl_vs_elo,
            "t1_rate": side.signals.t1_rate,
            "avg_cpl": side.signals.avg_cpl,
            "complex_ply_count": side.signals.complex_ply_count,
            "blur_rate": side.signals.blur_rate,
        },
        "ply_evals": side.ply_evals,
    })
}
