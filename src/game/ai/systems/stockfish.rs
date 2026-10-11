use super::*;

// Stockfish process code remains buildable on Android but is not offered by
// the engine picker. A missing executable resolves to Err.

/// A running Stockfish process that has completed the UCI handshake.
pub(super) struct StockfishInner {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    reader: std::io::BufReader<std::process::ChildStdout>,
}

impl StockfishInner {
    /// Starts Stockfish and waits until it is ready. This takes seconds (it loads
    /// its network first, and far longer from a cold disk), so it runs on a task.
    fn start() -> Result<Self, String> {
        let stockfish_path = resolve_stockfish_path()?;

        let mut cmd = Command::new(&stockfish_path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let mut child = cmd.spawn().map_err(|e| {
            format!(
                "Failed to spawn Stockfish at '{}': {}",
                stockfish_path.display(),
                e
            )
        })?;

        let stdin = child.stdin.take().ok_or("Failed to get Stockfish stdin")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to get Stockfish stdout")?;
        let mut engine = Self {
            child,
            stdin,
            reader: std::io::BufReader::new(stdout),
        };

        engine.send("uci")?;
        engine.read_until(|line| line == "uciok")?;
        #[cfg(target_os = "android")]
        {
            engine.send("setoption name Threads value 2")?;
            engine.send("setoption name Hash value 32")?;
        }
        engine.send("isready")?;
        engine.read_until(|line| line == "readyok")?;
        Ok(engine)
    }

    fn send(&mut self, command: &str) -> Result<(), String> {
        writeln!(self.stdin, "{command}").map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())
    }

    /// Reads lines until `done` matches one, passing every line to `on_line`.
    fn read_lines(
        &mut self,
        mut on_line: impl FnMut(&str),
        done: impl Fn(&str) -> bool,
    ) -> Result<String, String> {
        loop {
            let mut line = String::new();
            if self
                .reader
                .read_line(&mut line)
                .map_err(|e| e.to_string())?
                == 0
            {
                return Err("Stockfish exited unexpectedly".to_string());
            }
            let line = line.trim();
            on_line(line);
            if done(line) {
                return Ok(line.to_string());
            }
        }
    }

    fn read_until(&mut self, done: impl Fn(&str) -> bool) -> Result<String, String> {
        self.read_lines(|_| {}, done)
    }

    /// Searches `fen` and returns the best move with the last reported stats.
    fn search(
        &mut self,
        fen: &str,
        depth: u8,
        movetime_ms: u64,
        strength_elo: Option<u16>,
    ) -> Result<(String, i32, u8, i64), String> {
        // Apply the level policy on every search so a reused process cannot
        // retain the previous game's strength setting.
        for command in stockfish_strength_commands(strength_elo) {
            self.send(&command)?;
        }
        self.send("ucinewgame")?;
        self.send("isready")?;
        self.read_until(|line| line == "readyok")?;

        self.send(&format!("position fen {fen}"))?;
        self.send(&stockfish_search_command(depth, movetime_ms))?;

        let mut score = 0i32;
        let mut search_depth = 0u8;
        let mut nodes = 0i64;
        let bestmove = self.read_lines(
            |line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.first() != Some(&"info") {
                    return;
                }
                for pair in parts.windows(2) {
                    match pair[0] {
                        "depth" => search_depth = pair[1].parse().unwrap_or(search_depth),
                        "cp" => score = pair[1].parse().unwrap_or(score),
                        "nodes" => nodes = pair[1].parse().unwrap_or(nodes),
                        _ => {}
                    }
                }
            },
            |line| line.starts_with("bestmove"),
        )?;

        match bestmove.split_whitespace().nth(1) {
            Some(mv) if mv != "(none)" => Ok((mv.to_string(), score, search_depth, nodes)),
            _ => Err("Stockfish did not return a valid move".to_string()),
        }
    }
}

impl Drop for StockfishInner {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The shared Stockfish process, reused across games. It starts on first use
/// (or on [`warm_up_stockfish`]) on a task, never on the main thread, and a
/// failed search drops it so the next search starts a fresh one.
#[derive(Resource, Clone, Default)]
pub struct StockfishProcess(std::sync::Arc<std::sync::Mutex<Option<StockfishInner>>>);

/// The running engine in `slot`, starting it first if needed.
fn running(slot: &mut Option<StockfishInner>) -> Result<&mut StockfishInner, String> {
    if slot.is_none() {
        let started = Instant::now();
        *slot = Some(StockfishInner::start()?);
        info!(
            "[AI] Stockfish ready in {:.1}s",
            started.elapsed().as_secs_f32()
        );
    }
    Ok(slot.as_mut().expect("started above"))
}

/// Starts Stockfish in the background if no process exists yet, so the first
/// AI move doesn't wait for it to load.
pub fn warm_up_stockfish(process: Option<Res<StockfishProcess>>, mut commands: Commands) {
    if process.is_some() {
        return;
    }
    let process = StockfishProcess::default();
    let slot = process.0.clone();
    AsyncComputeTaskPool::get()
        .spawn(async move {
            let Ok(mut slot) = slot.lock() else {
                return;
            };
            if let Err(e) = running(&mut slot) {
                warn!("[AI] Stockfish warm-up failed: {e}");
            }
        })
        .detach();
    commands.insert_resource(process);
}

pub(super) fn resolve_stockfish_path() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("STOCKFISH_PATH") {
        let override_path = PathBuf::from(path.trim());
        if override_path.is_file() {
            return Ok(override_path);
        }

        return Err(format!(
            "STOCKFISH_PATH is set but the file does not exist: {}",
            override_path.display()
        ));
    }

    #[cfg(target_os = "android")]
    {
        let path = crate::android::platform::native_library_dir()?.join("libstockfish.so");
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "Stockfish library not found in Android nativeLibraryDir: {}",
            path.display()
        ));
    }

    let binary = if cfg!(target_os = "windows") {
        "stockfish.exe"
    } else {
        "stockfish"
    };
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join("assets").join("bin").join(binary));
        candidates.push(current_dir.join(binary));
    }

    if let Ok(exe_path) = env::current_exe() {
        for ancestor in exe_path
            .parent()
            .into_iter()
            .flat_map(|path| path.ancestors())
        {
            candidates.push(ancestor.join("assets").join("bin").join(binary));
            candidates.push(ancestor.join(binary));
            candidates.push(ancestor.join("resources").join(binary));
        }
    }

    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            format!(
                "Stockfish executable not found. Set STOCKFISH_PATH or place {binary} in assets/bin/ or next to the app."
            )
        })
}

/// Searches on `process`, waiting for it to finish starting if a warm-up is
/// still running.
pub(super) fn spawn_stockfish_task_persistent(
    fen: String,
    depth: u8,
    movetime_ms: u64,
    strength_elo: Option<u16>,
    process: StockfishProcess,
) -> Task<Result<AIMove, String>> {
    AsyncComputeTaskPool::get().spawn(async move {
        let start_time = Instant::now();

        let mut slot = process
            .0
            .lock()
            .map_err(|e| format!("Stockfish mutex poisoned: {}", e))?;
        let searched = running(&mut slot)
            .and_then(|engine| engine.search(&fen, depth, movetime_ms, strength_elo));
        let (best_move, score, search_depth, nodes) = match searched {
            Ok(found) => found,
            Err(e) => {
                // Its state is unknown now; the next search starts a new one.
                *slot = None;
                return Err(e);
            }
        };
        drop(slot);

        let thinking_time = start_time.elapsed().as_secs_f32();
        info!(
            "[AI] Stockfish: {} depth={} score={} time={:.2}s",
            best_move, search_depth, score, thinking_time
        );

        if best_move.len() >= 4 {
            let from_coords = ChessEngine::uci_to_coords(&best_move[0..2])
                .ok_or_else(|| format!("Invalid from square: {}", &best_move[0..2]))?;
            let to_coords = ChessEngine::uci_to_coords(&best_move[2..4])
                .ok_or_else(|| format!("Invalid to square: {}", &best_move[2..4]))?;
            return Ok(AIMove {
                from: from_coords,
                to: to_coords,
                uci: best_move,
                score,
                depth: search_depth,
                thinking_time,
                nodes,
            });
        }

        Err(format!("Invalid move format from Stockfish: {}", best_move))
    })
}

pub(super) fn stockfish_strength_commands(strength_elo: Option<u16>) -> Vec<String> {
    match strength_elo {
        Some(elo) => vec![
            "setoption name UCI_LimitStrength value true".to_string(),
            format!("setoption name UCI_Elo value {elo}"),
        ],
        None => vec!["setoption name UCI_LimitStrength value false".to_string()],
    }
}

pub(super) fn stockfish_search_command(depth: u8, movetime_ms: u64) -> String {
    if movetime_ms > 0 {
        format!("go movetime {movetime_ms}")
    } else {
        format!("go depth {depth}")
    }
}
