//! The end-of-game popup ([`popup`]), its PGN ([`pgn`]), and payout and dispute
//! status ([`payout`]).

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::core::GameState;
use crate::game::components::piece_types::PieceColor;
use crate::game::resources::{GameOverState, MoveHistory};
use crate::ui::styles::*;

mod payout;
mod pgn;
mod popup;
pub use payout::*;
pub use pgn::*;
pub use popup::*;

pub struct GameOverPopupPlugin;

impl Plugin for GameOverPopupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameOverPayoutInfo>();
        app.init_resource::<CachedGamePgn>();

        app.add_systems(OnEnter(GameState::GameOver), cache_pgn_on_game_over);

        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            game_over_popup_system
                .run_if(in_state(GameState::GameOver))
                .run_if(|gm: Res<crate::core::GameMode>| *gm != crate::core::GameMode::Spectator),
        );

        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            spectator_game_over_overlay
                .run_if(in_state(GameState::GameOver))
                .run_if(|gm: Res<crate::core::GameMode>| *gm == crate::core::GameMode::Spectator),
        );

        app.add_systems(Update,
            apply_dispute_trigger
                .in_set(crate::core::schedule::SessionSet::Delegation)
                .run_if(resource_exists::<PendingDispute>));

        #[cfg(feature = "solana")]
        app.add_systems(OnEnter(GameState::GameOver), fetch_game_payout_info);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_sol_dynamic_zero() {
        assert_eq!(GameOverPayoutInfo::format_sol_dynamic(0.0), "0 SOL");
    }

    #[test]
    fn format_sol_dynamic_sub_millisol_uses_six_decimals() {
        // 300_000 lamports = 0.0003 SOL — would round to "0.000 SOL" at 3dp.
        let sol = 300_000.0 / 1_000_000_000.0;
        assert_eq!(GameOverPayoutInfo::format_sol_dynamic(sol), "0.000300 SOL");
    }

    #[test]
    fn format_sol_dynamic_wager_sized_uses_three_decimals() {
        let sol = 1_000_000.0 / 1_000_000_000.0; // MIN_WAGER_LAMPORTS
        assert_eq!(GameOverPayoutInfo::format_sol_dynamic(sol), "0.001 SOL");
    }

    #[test]
    fn fetch_game_payout_info_estimate_is_unconfirmed() {
        let info = GameOverPayoutInfo::default();
        assert!(!info.fee_breakdown_confirmed);
    }
}
