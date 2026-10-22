use std::fs::OpenOptions;
use std::io::Write;
use std::time::SystemTime;

/// Appends a line to `recovered_errors.log` for errors the fallback handler swallowed.
pub fn record_recovered_error(source: &str, error: &dyn std::fmt::Display) {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Keep session logs in the writable profile folder alongside game.log.
    let logs_dir = crate::multiplayer::network::identity::log_dir();

    let filepath = logs_dir.join("recovered_errors.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&filepath) {
        let _ = writeln!(file, "[{timestamp}] RECOVERED from {source}: {error}");
    }
}
