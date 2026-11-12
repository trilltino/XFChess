pub mod config;
pub mod cross_game;
pub mod elo_baseline;
pub mod engine;
pub mod error;
pub mod features;
pub mod ingest;
pub mod metrics;
pub mod report;
pub mod scoring;
pub mod types;

use std::time::{SystemTime, UNIX_EPOCH};

use config::AcConfig;
use engine::stockfish::StockfishHandle;
use error::AcResult;
use features::{accuracy, blur, complexity, timing};
use types::{AcReport, GameRecord, PlyEval, SideAnalysis, SignalValues};

pub async fn analyse_game(game: GameRecord, cfg: &AcConfig) -> AcResult<AcReport> {
    cfg.validate()?;
    let sf_path = cfg.stockfish_path.clone();
    let depth = cfg.analysis_depth;
    let analysis_cfg = cfg.clone();

    let game_id = game.game_id.clone();
    let context = game.context.clone();
    let result = game.result.clone();
    let white_ref = game.white.clone();
    let black_ref = game.black.clone();
    let moves_clone = game.moves.clone();

    // Run the CPU-bound Stockfish work on a blocking thread
    let (white_evals, black_evals) = tokio::task::spawn_blocking(move || {
        let mut sf = StockfishHandle::spawn(&sf_path)?;
        sf.set_multipv(2)?;
        evaluate_all_plies(&mut sf, &game, &analysis_cfg)
    })
    .await
    .map_err(|e| error::AcError::Stockfish(format!("spawn_blocking panicked: {e}")))??;

    let white = build_side_analysis(
        &white_ref.pubkey,
        white_ref.elo,
        &white_evals,
        &moves_clone,
        0,
        cfg,
    );
    let black = build_side_analysis(
        &black_ref.pubkey,
        black_ref.elo,
        &black_evals,
        &moves_clone,
        1,
        cfg,
    );

    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            error::AcError::InvalidInput(format!("system clock unavailable: {error}"))
        })?;
    let now_ms = u64::try_from(elapsed.as_millis())
        .map_err(|_| error::AcError::InvalidInput("analysis timestamp is out of range".into()))?;

    Ok(AcReport {
        game_id,
        context,
        result,
        white,
        black,
        engine_version: format!("stockfish-depth{depth}"),
        analysis_depth: depth,
        analysed_at_ms: now_ms,
    })
}

fn evaluate_all_plies(
    sf: &mut StockfishHandle,
    game: &GameRecord,
    cfg: &AcConfig,
) -> AcResult<(Vec<PlyEval>, Vec<PlyEval>)> {
    let mut white_evals: Vec<PlyEval> = Vec::new();
    let mut black_evals: Vec<PlyEval> = Vec::new();

    let mut fen_before = game.start_fen.as_str();
    for (index, mv) in game.moves.iter().enumerate() {
        if usize::try_from(mv.ply).ok() != Some(index) {
            return Err(error::AcError::InvalidInput(
                "analysis plies must be contiguous and zero-based".into(),
            ));
        }

        if fen_before.is_empty() {
            return Err(error::AcError::InvalidInput(format!(
                "missing position before ply {}",
                mv.ply
            )));
        }

        let pos = sf.analyse(fen_before, cfg.analysis_depth, cfg.movetime_ms)?;

        let played_eval =
            eval_after_played_move(sf, &mv.fen_after, cfg.analysis_depth, cfg.movetime_ms)?;
        let cpl = cpl_from_evals(pos.top1_cp, played_eval);
        let is_t1 = pos.best_move == mv.move_uci;
        let cplx = complexity::classify(pos.top1_cp, pos.top2_cp, cfg);

        let eval = PlyEval {
            ply: mv.ply,
            move_uci: mv.move_uci.clone(),
            top1_cp: pos.top1_cp,
            top2_cp: pos.top2_cp,
            cpl,
            is_t1,
            complexity: cplx,
            latency_ms: mv.latency_ms,
        };

        if mv.ply % 2 == 0 {
            white_evals.push(eval);
        } else {
            black_evals.push(eval);
        }
        fen_before = &mv.fen_after;
    }

    Ok((white_evals, black_evals))
}

fn eval_after_played_move(
    sf: &mut StockfishHandle,
    fen_after: &str,
    depth: u8,
    movetime: u64,
) -> AcResult<i32> {
    if fen_after.is_empty() {
        return Err(error::AcError::InvalidInput(
            "missing position after move".into(),
        ));
    }
    Ok(sf
        .analyse(fen_after, depth, movetime)?
        .top1_cp
        .saturating_neg())
}

fn cpl_from_evals(best_before_cp: i32, played_after_cp: i32) -> i32 {
    best_before_cp.saturating_sub(played_after_cp).max(0)
}

fn build_side_analysis(
    pubkey: &str,
    elo: u32,
    evals: &[PlyEval],
    moves: &[types::MoveRecord],
    parity: usize,
    cfg: &AcConfig,
) -> SideAnalysis {
    let avg_cpl = accuracy::avg_cpl(evals);
    let t1 = accuracy::t1_rate(evals);
    let cpl_signal = accuracy::cpl_signal(elo, evals);
    let timing_source = timing::source_for(moves, parity);
    let timing = timing::timing_anomaly(evals, moves, timing_source, cfg);
    let complex_count = accuracy::complex_ply_count(evals);
    let blur_rate = blur::blur_rate(moves, parity);

    let signals = SignalValues {
        timing_anomaly: timing,
        cpl_vs_elo: cpl_signal,
        t1_rate: t1,
        avg_cpl,
        complex_ply_count: complex_count,
        blur_rate,
        timing_source,
    };

    let (score, verdict) = if complex_count < cfg.min_complex_plies {
        (0.0, types::Verdict::Clean)
    } else {
        scoring::score(&signals, cfg)
    };

    SideAnalysis {
        pubkey: pubkey.to_string(),
        elo,
        signals,
        weighted_score: score,
        verdict,
        ply_evals: evals.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::cpl_from_evals;

    #[test]
    fn cpl_from_evals_zero_when_best_move_played() {
        // Best move before was +50; the reached position also evals +50 — no loss.
        assert_eq!(cpl_from_evals(50, 50), 0);
    }

    #[test]
    fn cpl_from_evals_is_the_difference_not_the_raw_eval() {
        // A +50 best move followed by a +10 position loses 40 centipawns.
        assert_eq!(cpl_from_evals(50, 10), 40);
    }

    #[test]
    fn cpl_from_evals_clamps_negative_to_zero() {
        // Played continuation scored higher than the pre-move "best" line —
        // never report negative loss.
        assert_eq!(cpl_from_evals(50, 60), 0);
    }
}
