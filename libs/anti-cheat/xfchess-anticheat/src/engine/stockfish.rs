use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{sync_channel, Receiver};
use std::time::{Duration, Instant};
use tracing::debug;

use crate::error::{AcError, AcResult};

#[derive(Debug, Clone)]
pub struct PosResult {
    pub top1_cp: i32,
    pub top2_cp: i32,
    pub best_move: String,
}

pub struct StockfishHandle {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<AcResult<String>>,
}

impl StockfishHandle {
    pub fn spawn(path: &str) -> AcResult<Self> {
        Self::spawn_command(Command::new(path), 5_000)
    }

    fn spawn_command(mut command: Command, timeout_ms: u64) -> AcResult<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| AcError::Stockfish(format!("failed to spawn stockfish: {e}")))?;

        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AcError::Stockfish(
                "missing child input or output pipe".into(),
            ));
        };
        let (tx, lines) = sync_channel(256);
        let reader = std::thread::Builder::new()
            .name("anticheat-stockfish-output".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    let mut line = String::new();
                    let result = match reader.by_ref().take(65_537).read_line(&mut line) {
                        Ok(0) => Err(AcError::Stockfish("stockfish exited".into())),
                        Ok(_) if line.len() > 65_536 => {
                            Err(AcError::UciParse("engine line exceeds 64 KiB".into()))
                        }
                        Ok(_) => Ok(line),
                        Err(error) => Err(AcError::Stockfish(format!("read error: {error}"))),
                    };
                    let failed = result.is_err();
                    if tx.send(result).is_err() || failed {
                        break;
                    }
                }
            });
        if let Err(error) = reader {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AcError::Stockfish(format!(
                "cannot start engine output reader: {error}"
            )));
        }

        let mut sf = StockfishHandle {
            child,
            stdin,
            lines,
        };

        sf.send("uci")?;
        sf.wait_for("uciok", timeout_ms)?;
        sf.send("isready")?;
        sf.wait_for("readyok", timeout_ms)?;

        Ok(sf)
    }

    fn send(&mut self, cmd: &str) -> AcResult<()> {
        writeln!(self.stdin, "{cmd}").map_err(|e| AcError::Stockfish(format!("write error: {e}")))
    }

    fn read_line(&self, deadline: Instant, timeout_ms: u64) -> AcResult<String> {
        self.lines
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|error| match error {
                std::sync::mpsc::RecvTimeoutError::Timeout => AcError::StockfishTimeout(timeout_ms),
                std::sync::mpsc::RecvTimeoutError::Disconnected => {
                    AcError::Stockfish("engine output reader stopped".into())
                }
            })?
    }

    fn wait_for(&mut self, token: &str, timeout_ms: u64) -> AcResult<()> {
        let deadline = deadline(timeout_ms)?;
        loop {
            if Instant::now() > deadline {
                return Err(AcError::StockfishTimeout(timeout_ms));
            }
            if self.read_line(deadline, timeout_ms)?.trim() == token {
                return Ok(());
            }
        }
    }

    pub fn analyse(&mut self, fen: &str, depth: u8, movetime_ms: u64) -> AcResult<PosResult> {
        if depth == 0 || movetime_ms == 0 || fen.contains(['\r', '\n']) {
            return Err(AcError::InvalidInput(
                "invalid engine search request".into(),
            ));
        }
        let game = xfchess_rules::Game::from_fen(fen).map_err(AcError::InvalidInput)?;
        if game.is_checkmate() || game.is_stalemate() || game.is_insufficient_material() {
            return Ok(PosResult {
                top1_cp: if game.is_checkmate() { -30_000 } else { 0 },
                top2_cp: 0,
                best_move: String::new(),
            });
        }
        let timeout_ms = movetime_ms
            .checked_mul(3)
            .ok_or_else(|| AcError::InvalidInput("search duration is out of range".into()))?;
        let deadline = deadline(timeout_ms)?;
        self.send("ucinewgame")?;
        self.send(&format!("position fen {fen}"))?;
        self.send(&format!("go depth {depth} movetime {movetime_ms}"))?;

        let mut scores: [Option<(i32, String)>; 2] = [None, None];

        loop {
            if Instant::now() > deadline {
                return Err(AcError::StockfishTimeout(timeout_ms));
            }
            let buf = self.read_line(deadline, timeout_ms)?;
            let line = buf.trim();

            if line.starts_with("bestmove") {
                break;
            }

            // Parse: "info depth N multipv M score cp X ... pv <move>"
            if line.starts_with("info") && line.contains("multipv") && line.contains(" pv ") {
                if let Some(cp) = parse_cp(line) {
                    if let Some(mv) = parse_pv_move(line) {
                        if let Some(mpv) = parse_multipv(line) {
                            let index = mpv
                                .checked_sub(1)
                                .filter(|index| *index < scores.len())
                                .ok_or_else(|| {
                                    AcError::UciParse("MultiPV index must be one or two".into())
                                })?;
                            if !game.is_legal_uci(&mv) {
                                return Err(AcError::UciParse(
                                    "illegal principal variation move".into(),
                                ));
                            }
                            scores[index] = Some((cp, mv));
                        }
                    }
                }
            }
        }

        let (top1_cp, best_move) = scores[0]
            .take()
            .ok_or_else(|| AcError::UciParse("no principal variation score returned".into()))?;
        let top2_cp = scores[1].as_ref().map(|(cp, _)| *cp).unwrap_or(top1_cp);

        debug!("[stockfish] fen={fen} top1={top1_cp} top2={top2_cp} best={best_move}");

        Ok(PosResult {
            top1_cp,
            top2_cp,
            best_move,
        })
    }

    pub fn set_multipv(&mut self, n: u8) -> AcResult<()> {
        if !(1..=2).contains(&n) {
            return Err(AcError::InvalidInput("MultiPV must be one or two".into()));
        }
        self.send(&format!("setoption name MultiPV value {n}"))
    }
}

impl Drop for StockfishHandle {
    fn drop(&mut self) {
        // Dropping a `Child` neither stops nor reaps it: without this, every
        // analysed game would leave an engine (or a zombie) behind.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn parse_cp(line: &str) -> Option<i32> {
    let mut parts = line.split_whitespace().peekable();
    while let Some(tok) = parts.next() {
        if tok == "score" {
            match parts.next()? {
                "cp" => return parts.next()?.parse().ok(),
                "mate" => {
                    let n: i32 = parts.next()?.parse().ok()?;
                    // Mate in N: use a large sentinel value
                    return Some(if n > 0 {
                        30_000_i32.saturating_sub(n).max(1)
                    } else {
                        (-30_000_i32).saturating_sub(n).min(-1)
                    });
                }
                _ => {}
            }
        }
    }
    None
}

fn deadline(timeout_ms: u64) -> AcResult<Instant> {
    Instant::now()
        .checked_add(Duration::from_millis(timeout_ms))
        .ok_or_else(|| AcError::InvalidInput("engine deadline is out of range".into()))
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

    fn fake_command(mode: &str) -> std::io::Result<Command> {
        let mut command = Command::new(std::env::current_exe()?);
        command
            .args([
                "--exact",
                "engine::stockfish::tests::fake_engine_child",
                "--ignored",
                "--nocapture",
            ])
            .env("XFCHESS_AC_FAKE_MODE", mode);
        Ok(command)
    }

    #[test]
    #[ignore = "child process fixture"]
    fn fake_engine_child() -> anyhow::Result<()> {
        use anyhow::Context as _;
        let mode = std::env::var("XFCHESS_AC_FAKE_MODE")?;
        if mode == "silent" {
            std::thread::sleep(Duration::from_secs(30));
            return Ok(());
        }
        println!();
        std::io::stdout().flush()?;
        for line in std::io::stdin().lock().lines() {
            match line?.as_str() {
                "uci" => println!("uciok"),
                "isready" => println!("readyok"),
                command if command.starts_with("go ") => {
                    if mode == "search-silent" {
                        std::thread::sleep(Duration::from_secs(30));
                    } else {
                        println!(
                            "info depth 1 multipv {} score cp 20 pv e2e4",
                            if mode == "zero-index" { 0 } else { 1 }
                        );
                        println!("bestmove e2e4");
                    }
                }
                _ => {}
            }
            std::io::stdout()
                .flush()
                .context("flush fake engine response")?;
        }
        Ok(())
    }

    #[test]
    fn silent_engine_handshake_and_search_respect_deadlines() -> anyhow::Result<()> {
        let started = Instant::now();
        assert!(matches!(
            StockfishHandle::spawn_command(fake_command("silent")?, 150),
            Err(AcError::StockfishTimeout(150))
        ));
        assert!(started.elapsed() < Duration::from_secs(3));
        let mut engine = StockfishHandle::spawn_command(fake_command("search-silent")?, 2_000)?;
        let started = Instant::now();
        assert!(matches!(
            engine.analyse(xfchess_rules::STARTING_FEN, 1, 50),
            Err(AcError::StockfishTimeout(150))
        ));
        drop(engine);
        assert!(started.elapsed() < Duration::from_secs(3));
        Ok(())
    }

    #[test]
    fn malformed_multipv_and_extreme_scores_cannot_panic() -> anyhow::Result<()> {
        let mut engine = StockfishHandle::spawn_command(fake_command("zero-index")?, 2_000)?;
        assert!(matches!(
            engine.analyse(xfchess_rules::STARTING_FEN, 1, 50),
            Err(AcError::UciParse(_))
        ));
        assert_eq!(parse_cp("score mate -2147483648"), Some(-1));
        assert!(engine
            .analyse(xfchess_rules::STARTING_FEN, 1, u64::MAX)
            .is_err());
        Ok(())
    }
}
