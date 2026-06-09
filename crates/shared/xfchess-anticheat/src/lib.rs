pub mod config;
pub mod cross_game;
pub mod elo_baseline;
pub mod engine;
pub mod error;
pub mod features;
pub mod ingest;
pub mod report;
pub mod scoring;
pub mod types;

use std::time::{SystemTime, UNIX_EPOCH};

use tracing::debug;

use config::AcConfig;
use engine::stockfish::StockfishHandle;
use error::AcResult;
use features::{accuracy, complexity, timing};
use types::{
    AcReport, GameRecord, PlyEval, SideAnalysis, SignalValues,
};

/// Analyse a single game with Stockfish.  This is a blocking-ish call that
/// runs a Stockfish subprocess for the duration — call it from a worker task.
pub async fn analyse_game(game: GameRecord, cfg: &AcConfig) -> AcResult<AcReport> {
    // Spawn a Stockfish process for this analysis
    let sf_path = cfg.stockfish_path.clone();
    let depth = cfg.analysis_depth;
    let movetime = cfg.movetime_ms;

    // Extract metadata before moving game into the blocking closure
    let game_id      = game.game_id.clone();
    let context      = game.context.clone();
    let result       = game.result.clone();
    let white_ref    = game.white.clone();
    let black_ref    = game.black.clone();
    let moves_clone  = game.moves.clone();

    // Run the CPU-bound Stockfish work on a blocking thread
    let (white_evals, black_evals) = tokio::task::spawn_blocking(move || {
        let mut sf = StockfishHandle::spawn(&sf_path)?;
        sf.set_multipv(2)?;
        evaluate_all_plies(&mut sf, &game, depth, movetime)
    })
    .await
    .map_err(|e| error::AcError::Stockfish(format!("spawn_blocking panicked: {e}")))?
    ?;

    let white = build_side_analysis(&white_ref.pubkey, white_ref.elo, &white_evals, &moves_clone, cfg);
    let black = build_side_analysis(&black_ref.pubkey, black_ref.elo, &black_evals, &moves_clone, cfg);

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

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

/// Run Stockfish over every ply, returning white_evals and black_evals.
/// White plays even plies (0, 2, 4…), black plays odd plies.
fn evaluate_all_plies(
    sf: &mut StockfishHandle,
    game: &GameRecord,
    depth: u8,
    movetime: u64,
) -> AcResult<(Vec<PlyEval>, Vec<PlyEval>)> {
    let mut white_evals: Vec<PlyEval> = Vec::new();
    let mut black_evals: Vec<PlyEval> = Vec::new();

    for mv in &game.moves {
        // The FEN *before* this move is what we evaluate
        // We use fen_after of the previous move, or start_fen for ply 0
        let fen_before = if mv.ply == 0 {
            game.start_fen.clone()
        } else {
            game.moves
                .iter()
                .find(|m| m.ply + 1 == mv.ply)
                .map(|m| m.fen_after.clone())
                .unwrap_or_else(|| game.start_fen.clone())
        };

        if fen_before.is_empty() {
            debug!("[analyse] skipping ply {} — no FEN", mv.ply);
            continue;
        }

        let pos = sf.analyse(&fen_before, depth, movetime)?;

        let cpl = (pos.top1_cp - (pos.top1_cp - cpl_of_played_move(sf, &mv.fen_after, depth, movetime))).max(0);
        let is_t1 = pos.best_move == mv.move_uci;
        let cplx = complexity::classify(pos.top1_cp, pos.top2_cp, &AcConfig::default());

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
    }

    Ok((white_evals, black_evals))
}

/// Evaluate the position *after* the played move to compute CPL.
/// CPL = best_score_before − score_of_played_position (from side-to-move perspective).
/// We get this by evaluating fen_after and negating (it's opponent's turn).
fn cpl_of_played_move(
    sf: &mut StockfishHandle,
    fen_after: &str,
    depth: u8,
    movetime: u64,
) -> i32 {
    if fen_after.is_empty() { return 0; }
    match sf.analyse(fen_after, depth, movetime) {
        Ok(pos) => -pos.top1_cp, // negate because it's now the opponent's perspective
        Err(_) => 0,
    }
}

fn build_side_analysis(
    pubkey: &str,
    elo: u32,
    evals: &[PlyEval],
    moves: &[types::MoveRecord],
    cfg: &AcConfig,
) -> SideAnalysis {
    let avg_cpl = accuracy::avg_cpl(evals);
    let t1 = accuracy::t1_rate(evals);
    let cpl_signal = accuracy::cpl_signal(elo, evals);
    let timing = timing::timing_anomaly(evals, moves, cfg);
    let complex_count = accuracy::complex_ply_count(evals);

    let signals = SignalValues {
        timing_anomaly: timing,
        cpl_vs_elo: cpl_signal,
        t1_rate: t1,
        avg_cpl,
        complex_ply_count: complex_count,
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
