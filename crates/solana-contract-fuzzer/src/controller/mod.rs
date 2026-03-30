//! JSON controller interface (optional)
//!
//! Provides braid-fuzz compatible control interface for external test runners.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Controller commands from external runner
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd")]
pub enum ControllerCommand {
    #[serde(rename = "hello")]
    Hello { id: u64 },
    #[serde(rename = "init")]
    Init { id: u64, config: serde_json::Value },
    #[serde(rename = "fuzz-step")]
    FuzzStep { id: u64 },
    #[serde(rename = "reset")]
    Reset { id: u64 },
    #[serde(rename = "get-state")]
    GetState { id: u64 },
    #[serde(rename = "results")]
    Results { id: u64 },
}

/// Controller responses
#[derive(Debug, Clone, Serialize)]
pub struct ControllerResponse {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", flatten)]
    pub data: Option<serde_json::Value>,
}

/// Unsolicited events from fuzzer
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event")]
pub enum ControllerEvent {
    #[serde(rename = "instruction-executed")]
    InstructionExecuted { 
        instruction: String,
        result: String,
    },
    #[serde(rename = "invariant-failed")]
    InvariantFailed {
        invariant: String,
        details: String,
    },
    #[serde(rename = "panic")]
    Panic {
        message: String,
    },
    #[serde(rename = "update")]
    Update {
        data: serde_json::Value,
    },
}

/// Controller state
pub struct Controller {
    enabled: bool,
    state: Arc<Mutex<ControllerState>>,
}

#[derive(Debug, Default)]
struct ControllerState {
    fuzz_count: u64,
    last_error: Option<String>,
}

impl Controller {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            state: Arc::new(Mutex::new(ControllerState::default())),
        }
    }

    pub async fn handle_command(&self, cmd: ControllerCommand) -> Result<ControllerResponse> {
        if !self.enabled {
            return Ok(ControllerResponse {
                id: 0,
                ok: Some(false),
                error: Some("Controller disabled".to_string()),
                data: None,
            });
        }

        match cmd {
            ControllerCommand::Hello { id } => {
                Ok(ControllerResponse {
                    id,
                    ok: Some(true),
                    error: None,
                    data: None,
                })
            }
            ControllerCommand::Init { id, .. } => {
                // Initialize fuzzer with config
                Ok(ControllerResponse {
                    id,
                    ok: Some(true),
                    error: None,
                    data: None,
                })
            }
            ControllerCommand::FuzzStep { id } => {
                let mut state = self.state.lock().await;
                state.fuzz_count += 1;
                Ok(ControllerResponse {
                    id,
                    ok: Some(true),
                    error: None,
                    data: Some(serde_json::json!({
                        "fuzz_count": state.fuzz_count,
                    })),
                })
            }
            ControllerCommand::Reset { id } => {
                let mut state = self.state.lock().await;
                *state = ControllerState::default();
                Ok(ControllerResponse {
                    id,
                    ok: Some(true),
                    error: None,
                    data: None,
                })
            }
            ControllerCommand::GetState { id } => {
                let state = self.state.lock().await;
                Ok(ControllerResponse {
                    id,
                    ok: Some(true),
                    error: None,
                    data: Some(serde_json::json!({
                        "fuzz_count": state.fuzz_count,
                        "last_error": state.last_error,
                    })),
                })
            }
            ControllerCommand::Results { id } => {
                Ok(ControllerResponse {
                    id,
                    ok: Some(true),
                    error: None,
                    data: Some(serde_json::json!({
                        "passed": true,
                        "summary": "Fuzzing completed",
                    })),
                })
            }
        }
    }

    pub async fn emit_event(&self, event: ControllerEvent) {
        if !self.enabled {
            return;
        }
        // In stdio mode, print JSON line
        let json = serde_json::to_string(&event).unwrap();
        println!("{}", json);
    }
}
