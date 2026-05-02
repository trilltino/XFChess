//! Crash context gathering

use bevy::prelude::*;
use std::panic::PanicInfo;
use std::time::SystemTime;

#[derive(Debug)]
pub struct CrashContext {
    pub timestamp: u64,
    pub panic_message: String,
    pub location: String,
    pub game_state: Option<String>,
    pub recent_logs: Vec<String>,
    pub system_info: SystemInfo,
}

#[derive(Debug)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub game_version: String,
}

pub fn gather_crash_context(info: &PanicInfo) -> CrashContext {
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
    
    CrashContext {
        timestamp,
        panic_message,
        location,
        game_state: None, // Would be fetched from ECS in real impl
        recent_logs: vec![],
        system_info: SystemInfo {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    }
}

impl CrashContext {
    pub fn format_report(&self) -> String {
        format!(
            "PANIC DETECTED [{}]\n\
            ============================================\n\
            Message: {}\n\
            Location: {}\n\
            OS: {}\n\
            Arch: {}\n\
            Version: {}\n\
            ============================================\n",
            self.timestamp,
            self.panic_message,
            self.location,
            self.system_info.os,
            self.system_info.arch,
            self.system_info.game_version
        )
    }
}
