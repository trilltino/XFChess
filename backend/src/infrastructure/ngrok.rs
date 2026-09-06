//! Utilities for starting and inspecting the local ngrok tunnel.

use std::process::Command;
use tracing::{error, info, warn};

/// Starts an ngrok tunnel for the signing server.
pub fn start_ngrok_tunnel(port: u16) -> Result<String, String> {
    info!("[Ngrok] Starting tunnel for port {}", port);

    let check_result = Command::new("ngrok").arg("version").output();

    match check_result {
        Ok(_) => {
            info!("[Ngrok] ngrok is installed");
        }
        Err(_) => {
            let msg = "ngrok not found. Install from https://ngrok.com/download".to_string();
            warn!("[Ngrok] {}", msg);
            return Err(msg);
        }
    }

    let result = Command::new("ngrok")
        .args(["http", "--log=stdout", &port.to_string()])
        .spawn();

    match result {
        Ok(_child) => {
            info!("[Ngrok] Tunnel started successfully");
            info!("[Ngrok] Visit http://localhost:4040 to inspect traffic");
            info!("[Ngrok] Check the ngrok dashboard for the public URL");
            Ok("ngrok tunnel started".to_string())
        }
        Err(e) => {
            let msg = format!("Failed to start ngrok: {}", e);
            error!("[Ngrok] {}", msg);
            Err(msg)
        }
    }
}

/// Returns the current ngrok tunnel URL, when available.
pub fn get_ngrok_url() -> Option<String> {
    None
}
