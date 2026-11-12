//! The in-game HUD: the 2D board, side panels, player bars, chat and promotion
//! picker.

pub mod chat_ui;
pub mod game_2d;
pub mod game_ui;
pub mod left_panel;
pub mod player_bar;
pub mod promotion_ui;
pub mod top_bar;

pub use chat_ui::{drain_chat_messages, ChatState};
