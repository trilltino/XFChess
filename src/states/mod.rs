//! State-specific plugins for XFChess.

pub mod game_over;
pub mod main_menu;
pub mod main_menu_showcase;
pub mod multiplayer_menu;
pub mod pause;
pub mod piece_viewer;
pub use multiplayer_menu::MultiplayerMenuPlugin;

pub use game_over::GameOverPlugin;
pub use main_menu::MainMenuPlugin;
pub use pause::PausePlugin;
pub use piece_viewer::PieceViewerPlugin;
