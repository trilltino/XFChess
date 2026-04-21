//! Smart onboarding state machine for Solana Blinks.
//!
//! This module manages the onboarding state for users without wallets or SOL,
//! guiding them through wallet creation, funding, and registration.

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

use super::chains::{ActionChain, complete_step, create_onboarding_chain};
use super::core::BalanceResult;

/// Onboarding state for a user.
#[derive(Serialize, Deserialize, Clone)]
pub enum OnboardingState {
    /// User has no wallet
    NoWallet,
    /// User has wallet but insufficient funds
    InsufficientFunds { balance_lamports: u64, required_lamports: u64 },
    /// User is ready to register (has wallet and sufficient funds)
    ReadyToRegister,
    /// User has completed registration
    Registered,
    /// Validation failed with error
    ValidationFailed { error: String },
}

/// Onboarding session tracking.
#[derive(Clone)]
pub struct OnboardingSession {
    /// User wallet address (if known)
    pub wallet: Option<String>,
    /// Current onboarding state
    pub state: OnboardingState,
    /// Action chain for this user
    pub chain: Option<ActionChain>,
    /// Tournament ID
    pub tournament_id: u64,
    /// Session creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Smart onboarding state machine.
pub struct OnboardingStateMachine {
    /// Active onboarding sessions (keyed by wallet or session ID)
    sessions: RwLock<HashMap<String, OnboardingSession>>,
}

impl OnboardingStateMachine {
    /// Creates a new onboarding state machine.
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Starts a new onboarding session.
    pub async fn start_session(
        &self,
        session_id: String,
        tournament_id: u64,
        wallet: Option<String>,
        required_sol: f64,
    ) -> OnboardingSession {
        let state = if wallet.is_none() {
            OnboardingState::NoWallet
        } else {
            OnboardingState::InsufficientFunds {
                balance_lamports: 0,
                required_lamports: (required_sol * 1_000_000_000.0) as u64,
            }
        };

        let chain = create_onboarding_chain(tournament_id, wallet.clone(), required_sol);

        let session = OnboardingSession {
            wallet,
            state,
            chain: Some(chain),
            tournament_id,
            created_at: chrono::Utc::now(),
        };

        self.sessions.write().await.insert(session_id.clone(), session.clone());
        session
    }

    /// Updates onboarding state based on wallet balance check.
    pub async fn update_with_balance(
        &self,
        session_id: &str,
        balance: &BalanceResult,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id).ok_or("Session not found")?;

        session.state = if balance.sufficient {
            OnboardingState::ReadyToRegister
        } else {
            OnboardingState::InsufficientFunds {
                balance_lamports: balance.balance_lamports,
                required_lamports: balance.required_lamports,
            }
        };

        Ok(())
    }

    /// Marks user as registered.
    pub async fn mark_registered(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id).ok_or("Session not found")?;

        session.state = OnboardingState::Registered;

        // Complete all steps in chain
        if let Some(ref mut chain) = session.chain {
            let steps: Vec<_> = chain.steps.iter().map(|s| s.step).collect();
            for step in steps {
                complete_step(chain, step).ok();
            }
        }

        Ok(())
    }

    /// Marks validation as failed.
    pub async fn mark_validation_failed(&self, session_id: &str, error: String) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id).ok_or("Session not found")?;

        session.state = OnboardingState::ValidationFailed { error };

        Ok(())
    }

    /// Gets the current session state.
    pub async fn get_session(&self, session_id: &str) -> Option<OnboardingSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// Gets the next action for a session.
    pub async fn get_next_action(&self, session_id: &str) -> Option<String> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id)?;

        match &session.state {
            OnboardingState::NoWallet => {
                // Return wallet creation deep link
                Some("https://phantom.app/ul/browse/https://xfchess.com".to_string())
            }
            OnboardingState::InsufficientFunds { .. } => {
                // Return funding page
                if let Some(wallet) = &session.wallet {
                    Some(format!(
                        "https://xfchess.com/fund?wallet={}&amount={}",
                        wallet,
                        0.5 // Default amount, should be calculated from tournament
                    ))
                } else {
                    Some("https://xfchess.com/fund?amount=0.5".to_string())
                }
            }
            OnboardingState::ReadyToRegister => {
                // Return registration endpoint
                Some(format!("/api/actions/tournament/{}/register", session.tournament_id))
            }
            OnboardingState::Registered => {
                // Return view match endpoint
                if let Some(wallet) = &session.wallet {
                    Some(format!("/tournament/{}/my-match?player={}", session.tournament_id, wallet))
                } else {
                    Some(format!("/tournament/{}/my-match", session.tournament_id))
                }
            }
            OnboardingState::ValidationFailed { error } => {
                Some(format!("error:{}", error))
            }
        }
    }

    /// Cleans up old sessions (older than 1 hour).
    pub async fn cleanup_old_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(1);

        sessions.retain(|_, session| session.created_at > cutoff);
    }
}

impl Default for OnboardingStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// Global onboarding state machine instance.
static ONBOARDING_MACHINE: once_cell::sync::OnceCell<OnboardingStateMachine> = once_cell::sync::OnceCell::new();

/// Gets the global onboarding state machine instance.
pub fn get_onboarding_machine() -> &'static OnboardingStateMachine {
    ONBOARDING_MACHINE.get_or_init(|| OnboardingStateMachine::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_start_session_no_wallet() {
        let machine = OnboardingStateMachine::new();
        let session = machine.start_session("test_session".to_string(), 1, None, 0.5).await;

        assert!(matches!(session.state, OnboardingState::NoWallet));
        assert_eq!(session.tournament_id, 1);
    }

    #[tokio::test]
    async fn test_start_session_with_wallet() {
        let machine = OnboardingStateMachine::new();
        let session = machine
            .start_session("test_session".to_string(), 1, Some("wallet".to_string()), 0.5)
            .await;

        assert!(matches!(session.state, OnboardingState::InsufficientFunds { .. }));
    }

    #[tokio::test]
    async fn test_update_with_balance_sufficient() {
        let machine = OnboardingStateMachine::new();
        machine.start_session("test_session".to_string(), 1, Some("wallet".to_string()), 0.5).await;

        let balance = BalanceResult {
            wallet: "wallet".to_string(),
            balance_lamports: 1_000_000_000,
            sufficient: true,
            required_lamports: 500_000_000,
        };

        machine.update_with_balance("test_session", &balance).await.unwrap();
        let session = machine.get_session("test_session").await.unwrap();

        assert!(matches!(session.state, OnboardingState::ReadyToRegister));
    }

    #[tokio::test]
    async fn test_mark_registered() {
        let machine = OnboardingStateMachine::new();
        machine.start_session("test_session".to_string(), 1, Some("wallet".to_string()), 0.5).await;

        machine.mark_registered("test_session").await.unwrap();
        let session = machine.get_session("test_session").await.unwrap();

        assert!(matches!(session.state, OnboardingState::Registered));
    }
}
