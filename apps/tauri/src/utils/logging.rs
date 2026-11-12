use tracing::{info, Level};
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::EnvFilter;

pub fn log_dir() -> std::path::PathBuf {
  dirs::data_local_dir()
    .unwrap_or_else(|| std::path::PathBuf::from("."))
    .join("xfchess")
    .join("logs")
}

pub fn init_logging() {
  let env_filter =
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("xfchess_tauri=info"));

  let dir = log_dir();
  let file_appender = match tracing_appender::rolling::Builder::new()
    .rotation(tracing_appender::rolling::Rotation::DAILY)
    .filename_prefix("wallet-bridge.log")
    .build(&dir)
  {
    Ok(writer) => writer,
    Err(error) => {
      eprintln!("Cannot open wallet log at {}: {error}", dir.display());
      if let Err(error) = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .try_init()
      {
        eprintln!("Cannot install console logger: {error}");
      }
      return;
    }
  };
  // The tracing writer guard must live until exit; dropping it stops background flushing.
  let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
  Box::leak(Box::new(guard));

  tracing_subscriber::fmt()
    .with_max_level(Level::TRACE)
    .with_target(false)
    .with_thread_ids(false)
    .with_file(false)
    .with_line_number(false)
    .with_env_filter(env_filter)
    .with_ansi(false)
    // Still write to stdout too — free in dev (visible in a terminal) and
    // harmless in release (goes nowhere, same as before this change).
    .with_writer(std::io::stdout.and(non_blocking))
    .compact()
    .try_init()
    .unwrap_or_else(|error| eprintln!("Wallet logging unavailable: {error}"));

  info!(
    "[Logging] Writing to {}",
    dir.join("wallet-bridge.log.<date>").display()
  );
}
