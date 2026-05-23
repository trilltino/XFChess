use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::error::{AcError, AcResult};

/// Result of analysing one position.
#[derive(Debug, Clone)]
pub struct PosResult {
    /// Score of the best move in centipawns (from the side to move's perspective).
    pub top1_cp: i32,
    /// Score of the second-best move. 0 if only one legal move.
    pub top2_cp: i32,
    /// UCI string of the best move (e.g. "e2e4").
    pub best_move: String,
}

/// A single Stockfish subprocess handle.
pub struct StockfishHandle {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl StockfishHandle {
    pub fn spawn(path: &str) -> AcResult<Self> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| AcError::Stockfish(format!("failed to spawn stockfish at '{path}': {e}")))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| AcError::Stockfish("no stdin".into()))?;
        let stdout = BufReader::new(
            child.stdout.take()
                .ok_or_else(|| AcError::Stockfish("no stdout".into()))?
        );

        let mut sf = StockfishHandle { _child: child, stdin, stdout };

        // Handshake
        sf.send("uci")?;
        sf.wait_for("uciok", 5_000)?;
        sf.send("isready")?;
        sf.wait_for("readyok", 5_000)?;

        Ok(sf)
    }

    fn send(&mut self, cmd: &str) -> AcResult<()> {
        writeln!(self.stdin, "{cmd}")
            .map_err(|e| AcError::Stockfish(format!("write error: {e}")))
    }

    fn wait_for(&mut self, token: &str, timeout_ms: u64) -> AcResult<()> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let mut line = String::new();
        loop {
            if Instant::now() > deadline {
                return Err(AcError::StockfishTimeout(timeout_ms));
            }
            line.clear();
            self.stdout.read_line(&mut line)
                .map_err(|e| AcError::Stockfish(format!("read error: {e}")))?;
            if line.contains(token) {
                return Ok(());
            }
        }
    }

    /// Analyse a position given as a FEN string.
    /// Returns the top-2 moves by score and the best-move UCI.
    pub fn analyse(&mut self, fen: &str, depth: u8, movetime_ms: u64) -> AcResult<PosResult> {
        self.send("ucinewgame")?;
        self.send(&format!("position fen {fen}"))?;
        self.send(&format!("go depth {depth} movetime {movetime_ms}"))?;

        let mut scores: Vec<(i32, String)> = Vec::new(); // (cp, move_uci)
        let deadline = Instant::now() + Duration::from_millis(movetime_ms * 3);

        loop {
            if Instant::now() > deadline {
                return Err(AcError::StockfishTimeout(movetime_ms * 3));
            }
            let mut line = String::new();
            self.stdout.read_line(&mut line)
                .map_err(|e| AcError::Stockfish(format!("read error: {e}")))?;
            let line = line.trim();

            if line.starts_with("bestmove") {
                break;
            }

            // Parse: "info depth N multipv M score cp X ... pv <move>"
            if line.starts_with("info") && line.contains("multipv") && line.contains(" pv ") {
                if let Some(cp) = parse_cp(line) {
                    if let Some(mv) = parse_pv_move(line) {
                        if let Some(mpv) = parse_multipv(line) {
                            // Keep the latest score for each multipv slot
                            if mpv <= 2 {
                                let idx = mpv - 1;
                                while scores.len() <= idx { scores.push((0, String::new())); }
                                scores[idx] = (cp, mv);
                            }
                        }
                    }
                }
            }
        }

        // Fallback: re-run with multipv 2 if we got nothing useful
        if scores.is_empty() {
            return Err(AcError::UciParse("no scores returned".into()));
        }

        let top1_cp = scores.get(0).map(|(cp, _)| *cp).unwrap_or(0);
        let top2_cp = scores.get(1).map(|(cp, _)| *cp).unwrap_or(top1_cp);
        let best_move = scores.get(0).map(|(_, mv)| mv.clone()).unwrap_or_default();

        debug!("[stockfish] fen={fen} top1={top1_cp} top2={top2_cp} best={best_move}");

        Ok(PosResult { top1_cp, top2_cp, best_move })
    }

    /// Configure multipv (call once after spawn, before analysis loop).
    pub fn set_multipv(&mut self, n: u8) -> AcResult<()> {
        self.send(&format!("setoption name MultiPV value {n}"))
    }
}

// ── UCI line parsers ────────────────────────────────────────────────────────────

fn parse_cp(line: &str) -> Option<i32> {
    let mut parts = line.split_whitespace().peekable();
    while let Some(tok) = parts.next() {
        if tok == "score" {
            match parts.next()? {
                "cp" => return parts.next()?.parse().ok(),
                "mate" => {
                    let n: i32 = parts.next()?.parse().ok()?;
                    // Mate in N: use a large sentinel value
                    return Some(if n > 0 { 30_000 - n } else { -30_000 - n });
                }
                _ => {}
            }
        }
    }
    None
}

fn parse_pv_move(line: &str) -> Option<String> {
    let pv_idx = line.find(" pv ")?;
    let after = line[pv_idx + 4..].trim();
    Some(after.split_whitespace().next()?.to_string())
}

fn parse_multipv(line: &str) -> Option<usize> {
    let mut parts = line.split_whitespace();
    while let Some(tok) = parts.next() {
        if tok == "multipv" {
            return parts.next()?.parse().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cp_normal() {
        let line = "info depth 18 multipv 1 score cp 34 nodes 1234 pv e2e4 e7e5";
        assert_eq!(parse_cp(line), Some(34));
    }

    #[test]
    fn parse_cp_negative() {
        let line = "info depth 18 multipv 2 score cp -15 nodes 999 pv d7d5 c2c4";
        assert_eq!(parse_cp(line), Some(-15));
    }

    #[test]
    fn parse_pv_move_ok() {
        let line = "info depth 18 multipv 1 score cp 34 pv e2e4 e7e5 g1f3";
        assert_eq!(parse_pv_move(line), Some("e2e4".to_string()));
    }

    #[test]
    fn parse_multipv_ok() {
        let line = "info depth 18 multipv 2 score cp 20 pv d2d4";
        assert_eq!(parse_multipv(line), Some(2));
    }
}
