use super::*;

#[derive(Component)]
pub(super) struct MenuCamera;

#[derive(Resource)]
pub struct PlayerColorChoice {
    pub play_as_white: bool,
    pub selected: bool,
}

impl Default for PlayerColorChoice {
    fn default() -> Self {
        Self {
            play_as_white: true,
            selected: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LobbyFilter {
    #[default]
    All,
    Free,
    Wagered,
}

#[derive(Resource)]
pub struct CompetitiveMenuState {
    pub lobby_filter: LobbyFilter,
    pub show_ai_setup: bool,
    pub ai_difficulty: u8,
    pub ai_side: AISide,
    pub ai_time_control: crate::game::time_control::TimeControl,
    pub show_spectator_popup: bool,
    pub show_controls_popup: bool,
    pub show_join_popup: bool,
    pub join_game_id: String,
    pub lobby_sort: crate::multiplayer::social::LobbySort,
    pub lobby_tc_min: Option<u32>,
    pub lobby_tc_max: Option<u32>,
    pub show_pgn_input: bool,
    pub pgn_input_text: String,
    pub pgn_input_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuModal {
    AiSetup,
    Spectator,
    Controls,
    PgnInput,
}

impl Default for CompetitiveMenuState {
    fn default() -> Self {
        Self {
            lobby_filter: LobbyFilter::default(),
            show_ai_setup: false,
            ai_difficulty: 4,
            ai_side: AISide::default(),
            ai_time_control: crate::game::time_control::TimeControl::Blitz,
            show_spectator_popup: false,
            show_controls_popup: false,
            show_join_popup: false,
            join_game_id: String::new(),

            lobby_sort: crate::multiplayer::social::LobbySort::default(),
            lobby_tc_min: None,
            lobby_tc_max: None,
            show_pgn_input: false,
            pgn_input_text: String::new(),
            pgn_input_error: None,
        }
    }
}

impl CompetitiveMenuState {
    pub fn active_modal(&self) -> Option<MainMenuModal> {
        if self.show_ai_setup {
            Some(MainMenuModal::AiSetup)
        } else if self.show_spectator_popup {
            Some(MainMenuModal::Spectator)
        } else if self.show_controls_popup {
            Some(MainMenuModal::Controls)
        } else if self.show_pgn_input {
            Some(MainMenuModal::PgnInput)
        } else {
            None
        }
    }

    pub fn try_open_modal(&mut self, modal: MainMenuModal) -> bool {
        if self.active_modal().is_some() {
            return false;
        }
        self.set_active_modal(Some(modal));
        true
    }

    pub fn set_active_modal(&mut self, modal: Option<MainMenuModal>) {
        self.show_ai_setup = matches!(modal, Some(MainMenuModal::AiSetup));
        self.show_spectator_popup = matches!(modal, Some(MainMenuModal::Spectator));
        self.show_controls_popup = matches!(modal, Some(MainMenuModal::Controls));
        self.show_pgn_input = matches!(modal, Some(MainMenuModal::PgnInput));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AISide {
    Black,
    #[default]
    Random,
    White,
}

#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct P2PHostState {
    pub base_time_minutes: u32,
    pub increment_seconds: u16,
    pub stake_amount: f64,
    pub wager_fiat: f64,
    pub wager_in_usd: bool,
    pub game_id: Option<String>,
    pub last_heartbeat: Option<std::time::Instant>,
    pub lobby_name: String,
    pub direct_mode: bool,
}

impl Default for P2PHostState {
    fn default() -> Self {
        Self {
            base_time_minutes: 10,
            increment_seconds: 5,
            stake_amount: 0.0,
            wager_fiat: 0.0,
            wager_in_usd: false,
            game_id: None,
            last_heartbeat: None,
            lobby_name: String::new(),
            direct_mode: false,
        }
    }
}

#[derive(Resource, Default)]
pub struct NewsBannerState {
    pub texture: Option<egui::TextureHandle>,
    pub loaded: bool,
}

#[derive(Resource, Default)]
pub struct BrandLogoState {
    pub texture: Option<egui::TextureHandle>,
    pub loaded: bool,
}

#[derive(Resource, Default)]
pub(super) struct FontsLoaded(pub(super) bool);
