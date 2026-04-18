//! Action chaining for Solana Blinks onboarding flow.
//!
//! This module implements the "pro move" of action chaining (Alpenglow 2026 feature)
//! for multi-step flows like wallet creation → funding → registration → viewing match.

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// A single step in an action chain.
#[derive(Serialize, Deserialize, Clone)]
pub struct ChainStep {
    /// Step number (1-indexed)
    pub step: u32,
    /// Label for this step
    pub label: String,
    /// Action to execute (URL or action type)
    pub action: ChainAction,
    /// Whether this step is completed
    pub completed: bool,
}

/// Action types for chain steps.
#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum ChainAction {
    /// Deep link to wallet app for wallet creation
    WalletCreation { deep_link: String },
    /// Link to funding page
    Funding { url: String, amount_sol: f64 },
    /// Link to validation endpoint
    Validation { url: String },
    /// Link to registration endpoint
    Registration { url: String },
    /// Link to view match/bracket
    ViewMatch { url: String },
    /// Error action
    Error { message: String },
}

/// Complete action chain definition.
#[derive(Serialize, Deserialize, Clone)]
pub struct ActionChain {
    /// Chain ID
    pub chain_id: String,
    /// Chain name
    pub name: String,
    /// Chain description
    pub description: String,
    /// Steps in the chain
    pub steps: Vec<ChainStep>,
    /// Current step index (0-indexed)
    pub current_step: usize,
}

/// Creates a registration action chain for users with existing wallets and SOL.
pub fn create_registration_chain(tournament_id: u64, wallet_pubkey: &str) -> ActionChain {
    ActionChain {
        chain_id: format!("registration-{}", tournament_id),
        name: "Tournament Registration".to_string(),
        description: "Register for the tournament and view your match".to_string(),
        steps: vec![
            ChainStep {
                step: 1,
                label: "Validate Registration".to_string(),
                action: ChainAction::Validation {
                    url: format!("/api/actions/tournament/{}/validate", tournament_id),
                },
                completed: false,
            },
            ChainStep {
                step: 2,
                label: "Register for Tournament".to_string(),
                action: ChainAction::Registration {
                    url: format!("/api/actions/tournament/{}/register", tournament_id),
                },
                completed: false,
            },
            ChainStep {
                step: 3,
                label: "View Your Match".to_string(),
                action: ChainAction::ViewMatch {
                    url: format!("/tournament/{}/my-match?player={}", tournament_id, wallet_pubkey),
                },
                completed: false,
            },
        ],
        current_step: 0,
    }
}

/// Creates an onboarding chain for users without wallets or SOL.
pub fn create_onboarding_chain(
    tournament_id: u64,
    wallet_pubkey: Option<String>,
    required_sol: f64,
) -> ActionChain {
    let steps = if wallet_pubkey.is_none() {
        // User has no wallet
        vec![
            ChainStep {
                step: 1,
                label: "Create Wallet".to_string(),
                action: ChainAction::WalletCreation {
                    deep_link: "https://phantom.app/ul/browse/https://xfchess.com".to_string(),
                },
                completed: false,
            },
            ChainStep {
                step: 2,
                label: "Fund Wallet".to_string(),
                action: ChainAction::Funding {
                    url: format!("https://xfchess.com/fund?amount={}", required_sol),
                    amount_sol: required_sol,
                },
                completed: false,
            },
            ChainStep {
                step: 3,
                label: "Validate Registration".to_string(),
                action: ChainAction::Validation {
                    url: format!("/api/actions/tournament/{}/validate", tournament_id),
                },
                completed: false,
            },
            ChainStep {
                step: 4,
                label: "Register for Tournament".to_string(),
                action: ChainAction::Registration {
                    url: format!("/api/actions/tournament/{}/register", tournament_id),
                },
                completed: false,
            },
            ChainStep {
                step: 5,
                label: "View Your Match".to_string(),
                action: ChainAction::ViewMatch {
                    url: format!("/tournament/{}/my-match", tournament_id),
                },
                completed: false,
            },
        ]
    } else {
        // User has wallet but needs funding
        let wallet = wallet_pubkey.clone().unwrap();
        vec![
            ChainStep {
                step: 1,
                label: "Fund Wallet".to_string(),
                action: ChainAction::Funding {
                    url: format!(
                        "https://xfchess.com/fund?wallet={}&amount={}",
                        wallet,
                        required_sol
                    ),
                    amount_sol: required_sol,
                },
                completed: false,
            },
            ChainStep {
                step: 2,
                label: "Validate Registration".to_string(),
                action: ChainAction::Validation {
                    url: format!("/api/actions/tournament/{}/validate", tournament_id),
                },
                completed: false,
            },
            ChainStep {
                step: 3,
                label: "Register for Tournament".to_string(),
                action: ChainAction::Registration {
                    url: format!("/api/actions/tournament/{}/register", tournament_id),
                },
                completed: false,
            },
            ChainStep {
                step: 4,
                label: "View Your Match".to_string(),
                action: ChainAction::ViewMatch {
                    url: format!(
                        "/tournament/{}/my-match?player={}",
                        tournament_id,
                        wallet
                    ),
                },
                completed: false,
            },
        ]
    };

    ActionChain {
        chain_id: format!("onboarding-{}", tournament_id),
        name: "Tournament Onboarding".to_string(),
        description: "Create wallet, fund with SOL, and register for tournament".to_string(),
        steps,
        current_step: 0,
    }
}

/// Marks a step as completed in the chain.
pub fn complete_step(chain: &mut ActionChain, step_number: u32) -> Result<(), String> {
    let step_index = (step_number - 1) as usize;
    if step_index >= chain.steps.len() {
        return Err(format!("Step {} does not exist", step_number));
    }
    chain.steps[step_index].completed = true;
    
    // Advance to next uncompleted step
    while chain.current_step < chain.steps.len() && chain.steps[chain.current_step].completed {
        chain.current_step += 1;
    }
    
    Ok(())
}

/// Gets the next action to execute in the chain.
pub fn get_next_action(chain: &ActionChain) -> Option<&ChainStep> {
    if chain.current_step >= chain.steps.len() {
        None
    } else {
        Some(&chain.steps[chain.current_step])
    }
}

/// Checks if the chain is complete.
pub fn is_chain_complete(chain: &ActionChain) -> bool {
    chain.steps.iter().all(|step| step.completed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_registration_chain() {
        let chain = create_registration_chain(1, "test_pubkey");
        assert_eq!(chain.steps.len(), 3);
        assert_eq!(chain.current_step, 0);
        assert!(!chain.steps[0].completed);
    }

    #[test]
    fn test_create_onboarding_chain_no_wallet() {
        let chain = create_onboarding_chain(1, None, 0.5);
        assert_eq!(chain.steps.len(), 5);
        assert_eq!(chain.current_step, 0);
    }

    #[test]
    fn test_create_onboarding_chain_with_wallet() {
        let chain = create_onboarding_chain(1, Some("test_pubkey".to_string()), 0.5);
        assert_eq!(chain.steps.len(), 4);
        assert_eq!(chain.current_step, 0);
    }

    #[test]
    fn test_complete_step() {
        let mut chain = create_registration_chain(1, "test_pubkey");
        complete_step(&mut chain, 1).unwrap();
        assert!(chain.steps[0].completed);
        assert_eq!(chain.current_step, 1);
    }

    #[test]
    fn test_get_next_action() {
        let chain = create_registration_chain(1, "test_pubkey");
        let next = get_next_action(&chain);
        assert!(next.is_some());
        assert_eq!(next.unwrap().step, 1);
    }

    #[test]
    fn test_is_chain_complete() {
        let mut chain = create_registration_chain(1, "test_pubkey");
        assert!(!is_chain_complete(&chain));
        
        for i in 1..=3 {
            complete_step(&mut chain, i).unwrap();
        }
        assert!(is_chain_complete(&chain));
    }
}
