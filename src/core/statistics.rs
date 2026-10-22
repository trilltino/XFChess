use bevy::prelude::*;

#[derive(Resource, Debug, Clone, Default, Reflect)]
#[reflect(Resource)]
pub struct GameStatistics {
    pub games_played: u32,

    pub white_wins: u32,

    pub black_wins: u32,

    pub draws: u32,

    pub total_moves: u32,

    pub longest_game: u32,

    pub shortest_game: u32,
}

impl GameStatistics {
    pub fn record_game(
        &mut self,
        winner: Option<crate::rendering::pieces::PieceColor>,
        moves: u32,
    ) {
        use crate::rendering::pieces::PieceColor;

        self.games_played += 1;
        self.total_moves += moves;

        match winner {
            Some(PieceColor::White) => self.white_wins += 1,
            Some(PieceColor::Black) => self.black_wins += 1,
            None => self.draws += 1,
        }

        if self.games_played == 1 {
            self.longest_game = moves;
            self.shortest_game = moves;
        } else {
            self.longest_game = self.longest_game.max(moves);
            self.shortest_game = self.shortest_game.min(moves);
        }
    }

    pub fn average_moves(&self) -> f32 {
        if self.games_played > 0 {
            self.total_moves as f32 / self.games_played as f32
        } else {
            0.0
        }
    }

    pub fn win_rate_white(&self) -> f32 {
        if self.games_played > 0 {
            self.white_wins as f32 / self.games_played as f32 * 100.0
        } else {
            0.0
        }
    }

    pub fn win_rate_black(&self) -> f32 {
        if self.games_played > 0 {
            self.black_wins as f32 / self.games_played as f32 * 100.0
        } else {
            0.0
        }
    }
}
