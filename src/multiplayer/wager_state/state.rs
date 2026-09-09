use crate::GameConfig;
use bevy::prelude::*;

#[derive(Resource, Debug, Clone)]
pub struct WagerState {
    pub game_id: Option<u64>,
    pub wager_amount: Option<f64>,
    pub total_pot: Option<f64>,
    pub player_color: Option<String>,
    pub game_pda: Option<String>,
    pub is_loaded: bool,
    pub country_fee: Option<f64>,
    pub elo_fee: Option<f64>,
    pub match_type: Option<String>,
}

impl Default for WagerState {
    fn default() -> Self {
        Self {
            game_id: None,
            wager_amount: None,
            total_pot: None,
            player_color: None,
            game_pda: None,
            is_loaded: false,
            country_fee: None,
            elo_fee: None,
            match_type: None,
        }
    }
}

impl WagerState {
    pub fn from_config(config: &GameConfig) -> Self {
        let total_pot = config.wager_amount.map(|w| w * 2.0);

        Self {
            game_id: config.game_id,
            wager_amount: config.wager_amount,
            total_pot,
            player_color: config.player_color.map(|c| format!("{:?}", c)),
            game_pda: config.game_pda.clone(),
            is_loaded: config.wager_amount.is_some(),
            country_fee: None, // Will be fetched from on-chain game account
            elo_fee: None,     // Will be fetched from on-chain game account
            match_type: None,  // Will be fetched from on-chain game account
        }
    }

    pub fn wager_display(&self) -> String {
        match self.wager_amount {
            Some(amount) => format!("{:.3} SOL", amount),
            None => "Free Game".to_string(),
        }
    }

    pub fn pot_display(&self) -> String {
        match self.total_pot {
            Some(amount) => format!("{:.3} SOL", amount),
            None => "0 SOL".to_string(),
        }
    }

    pub fn has_wager(&self) -> bool {
        self.wager_amount.map_or(false, |w| w > 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlayerColor;

    #[test]
    fn test_wager_state_from_config() {
        let config = GameConfig {
            game_id: Some(12345),
            player_color: Some(PlayerColor::White),
            wager_amount: Some(0.1),
            ..Default::default()
        };

        let state = WagerState::from_config(&config);

        assert_eq!(state.game_id, Some(12345));
        assert_eq!(state.wager_amount, Some(0.1));
        assert_eq!(state.total_pot, Some(0.2));
        assert!(state.is_loaded);
    }

    #[test]
    fn test_wager_display() {
        let state = WagerState {
            wager_amount: Some(0.5),
            ..Default::default()
        };

        assert_eq!(state.wager_display(), "0.500 SOL");
    }

    #[test]
    fn test_has_wager() {
        let state_with = WagerState {
            wager_amount: Some(0.1),
            ..Default::default()
        };
        assert!(state_with.has_wager());

        let state_without = WagerState {
            wager_amount: None,
            ..Default::default()
        };
        assert!(!state_without.has_wager());
    }
}
