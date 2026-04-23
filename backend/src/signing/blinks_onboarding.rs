//! Blinks onboarding state machine — tracks user sessions through
//! wallet creation → funding → registration action chains.
//!
//! Stub: full implementation deferred until Blinks Inspector testing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Onboarding step in the registration flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnboardingStep {
    Start,
    WalletCreated,
    Funded,
    Registered,
}

/// Per-session onboarding state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingSession {
    pub tournament_id: u64,
    pub wallet: String,
    pub step: OnboardingStep,
}

/// In-memory store of onboarding sessions keyed by wallet.
#[derive(Clone, Default)]
pub struct OnboardingStore {
    inner: Arc<RwLock<HashMap<String, OnboardingSession>>>,
}

impl OnboardingStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get(&self, wallet: &str) -> Option<OnboardingSession> {
        self.inner.read().await.get(wallet).cloned()
    }

    pub async fn advance(&self, wallet: &str, step: OnboardingStep) {
        let mut map = self.inner.write().await;
        if let Some(session) = map.get_mut(wallet) {
            info!("[onboarding] {} → {:?}", wallet, step);
            session.step = step;
        }
    }
}

pub fn onboard_user(_user_id: &str) -> Result<(), String> {
    // Placeholder for onboarding logic
    Ok(())
}
