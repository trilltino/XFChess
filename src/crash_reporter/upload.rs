//! Optional crash report upload to backend

use crate::crash_reporter::context::CrashContext;

pub async fn upload_crash_report(ctx: &CrashContext) -> Result<(), String> {
    let client = reqwest::Client::new();
    
    let report = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": ctx.timestamp,
        "error_hash": hash_error(&ctx.panic_message),
        "game_state": ctx.game_state,
        "os": ctx.system_info.os,
    });
    
    client
        .post("http://178.104.55.19:8090/api/crash-reports")
        .json(&report)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

fn hash_error(message: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    message.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
