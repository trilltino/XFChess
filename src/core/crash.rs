//! Enhanced crash reporter for XFChess client

use std::panic;
use std::path::Path;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::SystemTime;

/// Set up enhanced panic hook
pub fn setup_enhanced_panic_hook() {
    panic::set_hook(Box::new(|info| {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let panic_message = info.payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "Unknown panic".to_string());
        
        let location = info.location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "Unknown".to_string());
        
        let report = format!(
            "PANIC DETECTED [{}]\n\
            ============================================\n\
            Message: {}\n\
            Location: {}\n\
            OS: {}\n\
            Arch: {}\n\
            Version: {}\n\
            ============================================\n",
            timestamp,
            panic_message,
            location,
            std::env::consts::OS,
            std::env::consts::ARCH,
            env!("CARGO_PKG_VERSION")
        );
        
        // Write to log file
        let logs_dir = Path::new("logs");
        if !logs_dir.exists() {
            let _ = fs::create_dir_all(logs_dir);
        }
        
        let filename = format!("crash_{}.log", timestamp);
        let filepath = logs_dir.join(&filename);
        
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&filepath) 
        {
            let _ = writeln!(file, "{}", report);
        }
        
        // Print user instructions
        eprintln!("\n");
        eprintln!("========================================");
        eprintln!("XFChess has encountered an error.");
        eprintln!("Please check logs/{}", filename);
        eprintln!("========================================");
    }));
}
