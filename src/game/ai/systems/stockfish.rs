use bevy::prelude::*;
use std::{env, path::PathBuf};
use xfchess_stockfish::EngineWorker;

#[derive(Resource, Clone)]
pub struct StockfishProcess(pub EngineWorker);

impl StockfishProcess {
    pub fn start() -> Result<Self, String> {
        let path = resolve_stockfish_path()?;
        let (threads, hash) = if cfg!(target_os = "android") {
            (2, 32)
        } else {
            (2, 64)
        };
        Ok(Self(EngineWorker::new(path, threads, hash)))
    }
}

pub fn warm_up_stockfish(
    process: Option<Res<StockfishProcess>>,
    mut commands: Commands,
    mut last_attempt: Local<Option<std::time::Instant>>,
) {
    if process.is_some() {
        return;
    }
    if last_attempt.is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(5)) {
        return;
    }
    *last_attempt = Some(std::time::Instant::now());
    match StockfishProcess::start() {
        Ok(process) => {
            process.0.warm_up();
            commands.insert_resource(process);
        }
        Err(error) => warn!("[AI] Stockfish warm-up: {error}"),
    }
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
