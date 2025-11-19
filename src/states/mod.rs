//! State-specific plugins for XFChess
//!
//! Each game state has its own plugin that manages:
//! - OnEnter: Setup systems (spawn entities, load assets)
//! - Update: State-specific systems (run_if(in_state(...)))
//! - OnExit: Cleanup (automatic via DespawnOnExit)
//!
//! # Plugin Organization
//!
//! - `main_menu` - MainMenuPlugin: Primary menu interface (with integrated loading and pyramid scene)
//! - `settings` - SettingsPlugin: Game configuration
//! - `pause` - PausePlugin: In-game pause menu
//! - `game_over` - GameOverPlugin: Post-game statistics
//!
//! The InGame state is handled by the existing GamePlugin.
//!
//! # Reference
//!
//! Pattern based on `reference/bevy/examples/games/game_menu.rs`

pub mod game_over;
pub mod main_menu;
pub mod pause;
pub mod piece_viewer;
pub mod settings;

// Re-export plugins for convenience
pub use game_over::GameOverPlugin;
pub use main_menu::MainMenuPlugin;
pub use pause::PausePlugin;
pub use piece_viewer::PieceViewerPlugin;
pub use settings::SettingsPlugin;
