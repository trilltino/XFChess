//! Enhanced crash reporter for XFChess client

pub mod context;
pub mod upload;

use std::panic;
use std::path::Path;
use std::fs::{self, OpenOptions};
use std::io::Write;

use context::CrashContext;

/// Set up enhanced panic hook
pub fn setup_enhanced_panic_hook() {
    panic::set_hook(Box::new(|info| {
        let ctx = context::gather_crash_context(info);
        
        // Write to log file
        if let Err(e) = write_crash_report(&ctx) {
            eprintln!("[CRASH] Failed to write crash report: {}", e);
        }
        
        // Print user instructions
        eprintln!("\n");
        eprintln!("========================================");
        eprintln!("XFChess has encountered an error.");
        eprintln!("Please check logs/crash_{}.log", ctx.timestamp);
        eprintln!("========================================");
    }));
}

fn write_crash_report(ctx: &CrashContext) -> Result<(), Box<dyn std::error::Error>> {
    let logs_dir = Path::new("logs");
    if !logs_dir.exists() {
        fs::create_dir_all(logs_dir)?;
    }
    
    let filename = format!("crash_{}.log", ctx.timestamp);
    let filepath = logs_dir.join(&filename);
    
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&filepath)?;
    
    writeln!(file, "{}", ctx.format_report())?;
    
    Ok(())
}
