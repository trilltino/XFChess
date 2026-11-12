//! Persistent UCI worker. All child-process I/O runs outside the game loop.
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use xfchess_rules::Game;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnginePosition {
    pub starting_fen: String,
    pub moves: Vec<String>,
}
impl EnginePosition {
    pub fn from_fen(fen: impl Into<String>) -> Self {
        Self {
            starting_fen: fen.into(),
            moves: Vec::new(),
        }
    }
    pub fn game(&self) -> Result<Game, String> {
        let mut game = Game::from_fen(&self.starting_fen)?;
        for mv in &self.moves {
            game.play_uci(mv)?;
        }
        Ok(game)
    }
    fn command(&self) -> String {
        let mut command = format!("position fen {}", self.starting_fen);
        if !self.moves.is_empty() {
            command.push_str(" moves ");
            command.push_str(&self.moves.join(" "));
        }
        command
    }
}

#[derive(Clone, Debug)]
pub struct SearchRequest {
    pub game_id: u64,
    pub position: EnginePosition,
    pub movetime_ms: u64,
    pub depth: Option<u8>,
    pub strength_elo: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Score {
    Centipawns(i32),
    Mate(i32),
}
impl Score {
    pub fn white_perspective(self, white_to_move: bool) -> Self {
        let sign = if white_to_move { 1 } else { -1 };
        match self {
            Self::Centipawns(cp) => Self::Centipawns(cp.saturating_mul(sign)),
            Self::Mate(m) => Self::Mate(m.saturating_mul(sign)),
        }
    }
    pub fn centipawns(self) -> i32 {
        match self {
            Self::Centipawns(cp) => cp,
            Self::Mate(m) => {
                if m >= 0 {
                    30_000
                } else {
                    -30_000
                }
            }
        }
    }
}
#[derive(Clone, Debug)]
pub struct SearchResult {
    pub best_move: Option<String>,
    pub score: Option<Score>,
    pub depth: u8,
    pub nodes: i64,
    pub elapsed: f32,
}

pub struct SearchTicket {
    rx: Receiver<Result<SearchResult, String>>,
    cancel: Arc<AtomicBool>,
}
impl SearchTicket {
    pub fn try_result(&self) -> Option<Result<SearchResult, String>> {
        match self.rx.try_recv() {
            Ok(result) => Some(result),
            Err(crossbeam_channel::TryRecvError::Empty) => None,
            Err(_) => Some(Err("Stockfish worker stopped".into())),
        }
    }
}
impl Drop for SearchTicket {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

enum Job {
    Warmup,
    Search(
        SearchRequest,
        Sender<Result<SearchResult, String>>,
        Arc<AtomicBool>,
    ),
}
struct Control {
    tx: Sender<Job>,
    shutdown: Arc<AtomicBool>,
}
impl Drop for Control {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
    }
}
#[derive(Clone)]
pub struct EngineWorker(Arc<Control>);

#[derive(Clone)]
struct Config {
    path: PathBuf,
    threads: u8,
    hash_mb: u16,
    startup: Duration,
    grace: Duration,
    args: Vec<String>,
    environment: Vec<(String, String)>,
}
impl EngineWorker {
    pub fn new(path: PathBuf, threads: u8, hash_mb: u16) -> Result<Self, String> {
        Self::with_config(Config {
            path,
            threads,
            hash_mb,
            startup: Duration::from_secs(15),
            grace: Duration::from_secs(5),
            args: Vec::new(),
            environment: Vec::new(),
        })
    }
    fn with_config(config: Config) -> Result<Self, String> {
        let (tx, rx) = unbounded();
        let shutdown = Arc::new(AtomicBool::new(false));
        let stopped = shutdown.clone();
        thread::Builder::new()
            .name("stockfish-worker".into())
            .spawn(move || {
                let mut engine: Option<Process> = None;
                while !stopped.load(Ordering::Acquire) {
                    let job = match rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(job) => job,
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                        Err(_) => break,
                    };
                    match job {
                        Job::Warmup => {
                            if engine.is_none() {
                                engine =
                                    Process::start(&config, &stopped, &AtomicBool::new(false)).ok();
                            }
                        }
                        Job::Search(request, reply, cancel) => {
                            if cancel.load(Ordering::Acquire) {
                                continue;
                            }
                            let start = Instant::now();
                            let result = (|| {
                                let game = request.position.game()?;
                                if game.legal_moves().is_empty() {
                                    return Ok(SearchResult {
                                        best_move: None,
                                        score: None,
                                        depth: 0,
                                        nodes: 0,
                                        elapsed: 0.0,
                                    });
                                }
                                let mut last_error = String::new();
                                for _ in 0..2 {
                                    if stopped.load(Ordering::Acquire)
                                        || cancel.load(Ordering::Acquire)
                                    {
                                        return Err("Search cancelled".into());
                                    }
                                    let attempt = (|| {
                                        if engine.is_none() {
                                            engine =
                                                Some(Process::start(&config, &stopped, &cancel)?);
                                        }
                                        let process = engine
                                            .as_mut()
                                            .ok_or("Stockfish failed to initialize")?;
                                        let mut result =
                                            process.search(&request, &config, &stopped, &cancel)?;
                                        let mv = result.best_move.as_deref().ok_or(
                                            "Stockfish returned no move in a playable position",
                                        )?;
                                        if !game.is_legal_uci(mv) {
                                            return Err(format!(
                                                "Stockfish returned illegal move: {mv}"
                                            ));
                                        }
                                        result.score = result
                                            .score
                                            .map(|s| s.white_perspective(game.white_to_move()));
                                        result.elapsed = start.elapsed().as_secs_f32();
                                        Ok(result)
                                    })();
                                    match attempt {
                                        Ok(result) => return Ok(result),
                                        Err(error) => {
                                            engine = None;
                                            last_error = error;
                                        }
                                    }
                                }
                                Err(last_error)
                            })();
                            let _ = reply.send(result);
                        }
                    }
                }
            })
            .map_err(|error| format!("Cannot start Stockfish worker thread: {error}"))?;
        Ok(Self(Arc::new(Control { tx, shutdown })))
    }
    pub fn warm_up(&self) {
        let _ = self.0.tx.send(Job::Warmup);
    }
    pub fn search(&self, request: SearchRequest) -> SearchTicket {
        let (tx, rx) = unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        let _ = self.0.tx.send(Job::Search(request, tx, cancel.clone()));
        SearchTicket { rx, cancel }
    }
}

struct Process {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<Result<String, String>>,
    game_id: Option<u64>,
    elo_range: Option<(u16, u16)>,
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Process {
    fn start(config: &Config, shutdown: &AtomicBool, cancel: &AtomicBool) -> Result<Self, String> {
        let deadline = Instant::now() + config.startup;
        let mut command = Command::new(&config.path);
        command
            .envs(config.environment.iter().cloned())
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("Cannot start Stockfish at {}: {e}", config.path.display()))?;
        let stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("No Stockfish stdin".into());
            }
        };
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("No Stockfish stdout".into());
            }
        };
        let (tx, lines) = unbounded();
        thread::Builder::new()
            .name("stockfish-output".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) => {
                            let _ = tx.send(Err("Stockfish exited".into()));
                            break;
                        }
                        Ok(_) => {
                            if tx.send(Ok(line.trim().to_string())).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e.to_string()));
                            break;
                        }
                    }
                }
            })
            .map_err(|e| {
                let _ = child.kill();
                let _ = child.wait();
                e.to_string()
            })?;
        let mut engine = Self {
            child,
            stdin,
            lines,
            game_id: None,
            elo_range: None,
        };
        engine.send("uci")?;
        loop {
            let line = engine.line(deadline, shutdown, cancel)?;
            if line.starts_with("option name UCI_Elo ") {
                let parts: Vec<_> = line.split_whitespace().collect();
                let min = parts
                    .windows(2)
                    .find(|p| p[0] == "min")
                    .and_then(|p| p[1].parse().ok());
                let max = parts
                    .windows(2)
                    .find(|p| p[0] == "max")
                    .and_then(|p| p[1].parse().ok());
                engine.elo_range = min.zip(max).filter(|(min, max)| min <= max);
            }
            if line == "uciok" {
                break;
            }
        }
        engine.send(&format!("setoption name Threads value {}", config.threads))?;
        engine.send(&format!("setoption name Hash value {}", config.hash_mb))?;
        engine.ready(deadline, shutdown, cancel)?;
        Ok(engine)
    }
    fn send(&mut self, command: &str) -> Result<(), String> {
        writeln!(self.stdin, "{command}")
            .and_then(|_| self.stdin.flush())
            .map_err(|e| e.to_string())
    }
    fn line(
        &self,
        deadline: Instant,
        shutdown: &AtomicBool,
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        loop {
            if shutdown.load(Ordering::Acquire) || cancel.load(Ordering::Acquire) {
                return Err("Search cancelled".into());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("Stockfish timed out".into());
            }
            match self
                .lines
                .recv_timeout(remaining.min(Duration::from_millis(25)))
            {
                Ok(line) => return line,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err("Stockfish output disconnected".into()),
            }
        }
    }
    fn ready(
        &mut self,
        deadline: Instant,
        shutdown: &AtomicBool,
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        self.send("isready")?;
        while self.line(deadline, shutdown, cancel)? != "readyok" {}
        Ok(())
    }
    fn search(
        &mut self,
        request: &SearchRequest,
        config: &Config,
        shutdown: &AtomicBool,
        cancel: &AtomicBool,
    ) -> Result<SearchResult, String> {
        let deadline =
            Instant::now() + Duration::from_millis(request.movetime_ms.max(1)) + config.grace;
        if self.game_id != Some(request.game_id) {
            self.send("ucinewgame")?;
            self.game_id = Some(request.game_id);
        }
        match request.strength_elo {
            Some(elo) => {
                let (min, max) = self
                    .elo_range
                    .ok_or("Stockfish did not advertise UCI_Elo limits")?;
                self.send("setoption name UCI_LimitStrength value true")?;
                self.send(&format!(
                    "setoption name UCI_Elo value {}",
                    elo.clamp(min, max)
                ))?;
            }
            None => self.send("setoption name UCI_LimitStrength value false")?,
        }
        self.ready(deadline, shutdown, cancel)?;
        self.send(&request.position.command())?;
        let depth = request
            .depth
            .map(|d| format!(" depth {}", d.max(1)))
            .unwrap_or_default();
        self.send(&format!(
            "go movetime {}{depth}",
            request.movetime_ms.max(1)
        ))?;
        let mut result = SearchResult {
            best_move: None,
            score: None,
            depth: 0,
            nodes: 0,
            elapsed: 0.0,
        };
        loop {
            let line = self.line(deadline, shutdown, cancel)?;
            let parts: Vec<_> = line.split_whitespace().collect();
            if parts.first() == Some(&"bestmove") {
                let mv = parts.get(1).ok_or("Malformed bestmove response")?;
                if *mv != "(none)" && *mv != "0000" {
                    result.best_move = Some((*mv).to_string());
                }
                return Ok(result);
            }
            if parts.first() != Some(&"info") {
                continue;
            }
            if parts.windows(2).any(|p| p[0] == "multipv" && p[1] != "1") {
                continue;
            }
            for pair in parts.windows(2) {
                match pair[0] {
                    "depth" => {
                        result.depth = pair[1].parse().unwrap_or(result.depth);
                    }
                    "nodes" => {
                        result.nodes = pair[1].parse().unwrap_or(result.nodes);
                    }
                    "cp" => {
                        result.score = pair[1].parse().ok().map(Score::Centipawns);
                    }
                    "mate" => {
                        result.score = pair[1].parse().ok().map(Score::Mate);
                    }
                    _ => {}
                }
            }
        }
    }
}
