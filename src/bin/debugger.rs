//! Standalone Transaction Debugger Binary
//!
//! This binary runs as a sidecar process to monitor rollup transactions
//! from the XFChess game client.
//!
//! # Usage
//!
//! ```bash
//! ./xfchess-debugger --game-id 12345 --log-file ./game.log
//! ```

use clap::Parser;
use std::fs::File;
use std::io::{self, BufRead};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Simple transaction log entry
#[derive(Debug, Clone)]
struct LogEntry {
    timestamp: u64,
    game_id: u64,
    event_type: String,
    message: String,
}

/// CLI Arguments
#[derive(Parser, Debug)]
#[command(name = "xfchess-debugger")]
#[command(about = "Transaction debugger for XFChess rollup monitoring")]
struct Args {
    /// Game ID to monitor
    #[arg(long)]
    game_id: u64,

    /// Log file path
    #[arg(long, default_value = "rollup_debug.log")]
    log_file: PathBuf,

    /// Enable pretty colored output
    #[arg(long, default_value = "true")]
    pretty_print: bool,

    /// WebSocket port for remote monitoring (optional)
    #[arg(long)]
    websocket_port: Option<u16>,

    /// Read from stdin instead of file
    #[arg(long)]
    stdin: bool,

    /// Follow mode (keep reading new entries)
    #[arg(short, long)]
    follow: bool,
}

fn main() {
    let args = Args::parse();

    print_banner();
    println!("Game ID: {}", args.game_id);
    println!("Log file: {:?}", args.log_file);
    println!("Pretty print: {}", args.pretty_print);
    if let Some(port) = args.websocket_port {
        println!("WebSocket port: {}", port);
    }
    println!();

    // Create log file
    let log_file = match File::create(&args.log_file) {
        Ok(file) => {
            println!("✓ Log file created: {:?}", args.log_file);
            Some(file)
        }
        Err(e) => {
            eprintln!("✗ Failed to create log file: {}", e);
            None
        }
    };

    // Shared state for WebSocket server
    let entries: Arc<Mutex<Vec<LogEntry>>> = Arc::new(Mutex::new(Vec::new()));

    // Start WebSocket server if port specified
    if let Some(port) = args.websocket_port {
        let entries_clone = Arc::clone(&entries);
        thread::spawn(move || {
            start_websocket_server(port, entries_clone);
        });
    }

    // Main processing loop
    if args.stdin {
        println!("Reading from stdin... (Press Ctrl+C to exit)");
        process_stdin(args.game_id, args.pretty_print, log_file, entries);
    } else {
        println!("Monitoring game {}... (Press Ctrl+C to exit)", args.game_id);
        println!();

        // Simulate monitoring loop
        // In real implementation, this would connect to the game process
        // via shared memory, sockets, or file watching
        monitor_simulation(args.game_id, args.pretty_print, log_file, entries);
    }
}

fn print_banner() {
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║          XFChess Transaction Debugger                  ║");
    println!("║          Rollup Monitor v0.1.0                         ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!();
}

fn process_stdin(
    game_id: u64,
    pretty_print: bool,
    mut log_file: Option<File>,
    entries: Arc<Mutex<Vec<LogEntry>>>,
) {
    let stdin = io::stdin();
    let reader = stdin.lock();

    for line in reader.lines() {
        match line {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }

                // Parse JSON input (simplified)
                let entry = LogEntry {
                    timestamp: current_timestamp(),
                    game_id,
                    event_type: "stdin".to_string(),
                    message: line.clone(),
                };

                // Print to stdout
                if pretty_print {
                    println!("\x1b[36m[→]\x1b[0m {}", line);
                } else {
                    println!("{}", line);
                }

                // Write to log file
                if let Some(ref mut file) = log_file {
                    use std::io::Write;
                    writeln!(file, "{}", line).ok();
                    file.flush().ok();
                }

                // Store in memory
                entries.lock().unwrap().push(entry);
            }
            Err(e) => {
                eprintln!("Error reading stdin: {}", e);
                break;
            }
        }
    }
}

fn monitor_simulation(
    game_id: u64,
    pretty_print: bool,
    mut log_file: Option<File>,
    entries: Arc<Mutex<Vec<LogEntry>>>,
) {
    use std::io::Write;

    let event_types = vec![
        ("BatchProposed", "\x1b[33m[⟳ PROPOSED]\x1b[0m"),
        ("BatchAccepted", "\x1b[32m[✓ ACCEPTED]\x1b[0m"),
        ("SolanaSubmitted", "\x1b[36m[→ SUBMITTED]\x1b[0m"),
        ("SolanaConfirmed", "\x1b[32m[✓ CONFIRMED]\x1b[0m"),
    ];

    let mut event_index = 0;

    loop {
        thread::sleep(Duration::from_secs(2));

        let (event_type, color_code) = &event_types[event_index % event_types.len()];
        event_index += 1;

        let timestamp = current_timestamp();
        let batch_hash = format!("{:08x}", timestamp % 0xFFFFFFFFu64);

        let entry = LogEntry {
            timestamp,
            game_id,
            event_type: event_type.to_string(),
            message: format!("Batch {}", batch_hash),
        };

        // Print to terminal
        if pretty_print {
            println!(
                "{} Game {} | Batch: {}... | Moves: {} | Time: {}",
                color_code,
                game_id,
                &batch_hash,
                10,
                timestamp
            );
        } else {
            println!(
                "{{\"timestamp\":{},\"game_id\":{},\"type\":\"{}\",\"batch\":\"{}\"}}",
                timestamp, game_id, event_type, batch_hash
            );
        }

        // Write to log file
        let json = format!(
            "{{\"timestamp\":{},\"game_id\":{},\"type\":\"{}\",\"batch\":\"{}\"}}",
            timestamp, game_id, event_type, batch_hash
        );

        if let Some(ref mut file) = log_file {
            writeln!(file, "{}", json).ok();
            file.flush().ok();
        }

        // Store in memory
        entries.lock().unwrap().push(entry);

        // Print summary every 5 events
        if event_index % 5 == 0 {
            print_summary(&entries);
        }
    }
}

fn print_summary(entries: &Arc<Mutex<Vec<LogEntry>>>) {
    let entries = entries.lock().unwrap();
    let total = entries.len();

    println!();
    println!("\x1b[36m[Summary]\x1b[0m Total events logged: {}", total);
    println!();
}

fn start_websocket_server(port: u16, entries: Arc<Mutex<Vec<LogEntry>>>) {
    let addr = format!("127.0.0.1:{}", port);

    match TcpListener::bind(&addr) {
        Ok(listener) => {
            println!("✓ WebSocket server listening on ws://{}", addr);

            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        let entries = entries.lock().unwrap();
                        let response = format!(
                            "HTTP/1.1 200 OK\r\n\r\nTotal events: {}\r\n",
                            entries.len()
                        );
                        use std::io::Write;
                        stream.write_all(response.as_bytes()).ok();
                    }
                    Err(e) => {
                        eprintln!("Connection error: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to bind WebSocket server: {}", e);
        }
    }
}

fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
