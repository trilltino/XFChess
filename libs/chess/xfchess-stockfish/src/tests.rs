use super::*;
use std::{
    fs::{self, OpenOptions},
    sync::atomic::AtomicU64,
};
use xfchess_rules::STARTING_FEN;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    worker: EngineWorker,
    log: PathBuf,
}
impl Fixture {
    fn new(mode: &str) -> Self {
        let log = std::env::temp_dir().join(format!(
            "xfchess-uci-{}-{}.log",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let config = Config {
            path: std::env::current_exe().unwrap(),
            threads: 2,
            hash_mb: 32,
            startup: Duration::from_secs(2),
            grace: Duration::from_millis(150),
            args: vec![
                "--ignored".into(),
                "--nocapture".into(),
                "--exact".into(),
                "tests::fake_engine_child".into(),
            ],
            environment: vec![
                ("XFCHESS_FAKE_MODE".into(), mode.into()),
                (
                    "XFCHESS_FAKE_LOG".into(),
                    log.to_string_lossy().into_owned(),
                ),
            ],
        };
        Self {
            worker: EngineWorker::with_config(config),
            log,
        }
    }
    fn commands(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }
    fn starts(&self) -> usize {
        self.commands()
            .lines()
            .filter(|l| l.starts_with("start "))
            .count()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.log);
    }
}
fn request() -> SearchRequest {
    SearchRequest {
        game_id: 1,
        position: EnginePosition::from_fen(STARTING_FEN),
        movetime_ms: 10,
        depth: None,
        strength_elo: Some(1320),
    }
}
fn wait(ticket: &SearchTicket) -> Result<SearchResult, String> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(result) = ticket.try_result() {
            return result;
        }
        assert!(Instant::now() < deadline, "worker never completed");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
#[ignore = "subprocess fixture, invoked only by protocol tests"]
fn fake_engine_child() {
    let Ok(mode) = std::env::var("XFCHESS_FAKE_MODE") else {
        return;
    };
    let log = PathBuf::from(std::env::var_os("XFCHESS_FAKE_LOG").unwrap());
    let append = |line: &str| {
        writeln!(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log)
                .unwrap(),
            "{line}"
        )
        .unwrap();
    };
    append(&format!("start {}", std::process::id()));
    let first = fs::read_to_string(&log)
        .unwrap()
        .lines()
        .filter(|l| l.starts_with("start "))
        .count()
        == 1;
    let mut game = Game::default();
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        append(&line);
        match line.as_str() {
            "uci" => {
                if mode == "hang-start" {
                    thread::sleep(Duration::from_secs(30));
                }
                println!("id name Fake Stockfish\noption name UCI_Elo type spin default 1600 min 1400 max 2800\nuciok");
            }
            "isready" => println!("readyok"),
            line if line.starts_with("position fen ") => {
                let (fen, moves) = line[13..]
                    .split_once(" moves ")
                    .unwrap_or((&line[13..], ""));
                game = Game::from_fen(fen).unwrap();
                for mv in moves.split_whitespace() {
                    game.play_uci(mv).unwrap();
                }
            }
            line if line.starts_with("go ") => {
                if mode == "crash" || (mode == "crash-once" && first) {
                    std::process::exit(3);
                }
                if mode == "hang-search" || (mode == "hang-once" && first) {
                    thread::sleep(Duration::from_secs(30));
                }
                let mv = if mode == "illegal" {
                    "a1a8".to_string()
                } else {
                    game.legal_moves()[0].clone()
                };
                let score = if mode == "mate" { "mate 3" } else { "cp 35" };
                println!("info depth 8 score {score} nodes 123");
                if mode == "malformed" {
                    println!("bestmove");
                } else {
                    println!("bestmove {mv}");
                }
            }
            "quit" => break,
            _ => {}
        }
        std::io::stdout().flush().unwrap();
    }
    std::process::exit(0);
}

#[test]
fn strength_clamps_and_process_reuses_game_history() {
    let fixture = Fixture::new("normal");
    let first = wait(&fixture.worker.search(request())).unwrap();
    assert!(first.best_move.is_some());
    let mut next = request();
    next.position.moves = vec!["e2e4".into()];
    let second = wait(&fixture.worker.search(next.clone())).unwrap();
    assert_eq!(second.score, Some(Score::Centipawns(-35)));
    next.game_id = 2;
    next.strength_elo = None;
    wait(&fixture.worker.search(next)).unwrap();
    assert_eq!(fixture.starts(), 1);
    let commands = fixture.commands();
    assert_eq!(commands.lines().filter(|l| *l == "ucinewgame").count(), 2);
    assert!(commands.contains("setoption name UCI_Elo value 1400"));
    assert!(commands.contains("setoption name UCI_LimitStrength value false"));
    assert!(commands.contains(&format!("position fen {STARTING_FEN} moves e2e4")));
}

#[test]
fn a_crash_or_timeout_restarts_once_and_recovers() {
    for mode in ["crash-once", "hang-once"] {
        let fixture = Fixture::new(mode);
        assert!(wait(&fixture.worker.search(request())).is_ok(), "{mode}");
        assert_eq!(fixture.starts(), 2);
    }
}

#[test]
fn persistent_failures_stop_after_two_attempts() {
    for mode in ["crash", "illegal", "malformed", "hang-search", "hang-start"] {
        let fixture = Fixture::new(mode);
        assert!(wait(&fixture.worker.search(request())).is_err(), "{mode}");
        assert_eq!(fixture.starts(), 2, "{mode}");
    }
}

#[test]
fn missing_binary_fails_without_hanging() {
    let worker = EngineWorker::new(PathBuf::from("/no/such/stockfish"), 1, 16);
    assert!(wait(&worker.search(request()))
        .unwrap_err()
        .contains("Cannot start"));
}

#[test]
fn dropping_worker_reaps_the_child_process() {
    let fixture = Fixture::new("normal");
    wait(&fixture.worker.search(request())).unwrap();
    let pid = fixture
        .commands()
        .lines()
        .find_map(|line| line.strip_prefix("start ").map(str::to_owned))
        .unwrap();
    drop(fixture);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        #[cfg(windows)]
        let alive = {
            use std::os::windows::process::CommandExt;
            let output = Command::new("tasklist.exe")
                .creation_flags(0x08000000)
                .args(["/FI", &format!("PID eq {pid}"), "/FO", "csv", "/NH"])
                .output()
                .unwrap();
            String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
        };
        #[cfg(not(windows))]
        let alive = Command::new("kill")
            .args(["-0", &pid])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        if !alive {
            break;
        }
        assert!(Instant::now() < deadline, "orphan Stockfish child {pid}");
        thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn dropping_ticket_cancels_active_work_and_unblocks_next_request() {
    let fixture = Fixture::new("hang-once");
    let ticket = fixture.worker.search(request());
    let deadline = Instant::now() + Duration::from_secs(3);
    while !fixture.commands().contains("go movetime") {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    drop(ticket);
    assert!(wait(&fixture.worker.search(request())).is_ok());
    assert_eq!(fixture.starts(), 2);
}

#[test]
fn terminal_positions_and_mate_scores() {
    let fixture = Fixture::new("mate");
    let mut req = request();
    req.position.moves.push("e2e4".into());
    assert_eq!(
        wait(&fixture.worker.search(req.clone())).unwrap().score,
        Some(Score::Mate(-3))
    );
    req.position = EnginePosition::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1");
    assert_eq!(wait(&fixture.worker.search(req)).unwrap().best_move, None);
    assert_eq!(fixture.starts(), 1);
}

/// Uses the exact binary shipped in this checkout, including every difficulty budget.
#[test]
#[ignore = "requires bundled native Stockfish; cargo test -p xfchess-stockfish bundled_engine -- --ignored"]
fn bundled_engine() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let path = std::env::var_os("STOCKFISH_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            root.join(if cfg!(windows) {
                "assets/bin/stockfish.exe"
            } else {
                "assets/bin/stockfish"
            })
        });
    assert!(path.is_file(), "missing Stockfish at {}", path.display());
    let worker = EngineWorker::new(path, 2, 32);
    for (level, (ms, elo)) in [
        (50, Some(1320)),
        (150, Some(1450)),
        (300, Some(1600)),
        (600, Some(1750)),
        (1000, Some(1900)),
        (1500, Some(2100)),
        (2000, None),
        (3000, None),
    ]
    .into_iter()
    .enumerate()
    {
        for black in [false, true] {
            let mut req = request();
            req.game_id = level as u64 + 1;
            req.movetime_ms = ms;
            req.strength_elo = elo;
            if black {
                req.position.moves.push("e2e4".into());
            }
            let game = req.position.game().unwrap();
            let result = wait(&worker.search(req)).unwrap();
            assert!(game.is_legal_uci(result.best_move.as_deref().unwrap()));
        }
    }
    let mut req = request();
    req.strength_elo = None;
    req.movetime_ms = 100;
    assert!(wait(&worker.search(req)).unwrap().score.is_some());
}
