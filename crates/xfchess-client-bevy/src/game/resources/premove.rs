//! The one premove queue shared by the 2D and 3D boards.
//!
//! It only remembers an intention (two squares and the board it was made on).
//! Queueing never touches the engine, turn or network; the intention is
//! replayed through the normal validated move path once it is the local
//! player's turn (see `input::premove`).

use crate::rendering::pieces::{PieceColor, PieceType};
use bevy::prelude::*;

#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct Premove {
    from: Option<(u8, u8)>,
    to: Option<(u8, u8)>,
    /// `BoardEpoch` the first square was picked on.
    epoch: u64,
    owner: Option<(PieceColor, Option<String>)>,
    seat: Option<(u64, i64)>,
    promotion: Option<PieceType>,
}

impl Premove {
    pub fn from(&self) -> Option<(u8, u8)> {
        self.from
    }

    pub fn to(&self) -> Option<(u8, u8)> {
        self.to
    }

    /// Both squares are chosen.
    pub fn is_set(&self) -> bool {
        self.from.is_some() && self.to.is_some()
    }

    /// Anything queued or half-queued.
    pub fn is_active(&self) -> bool {
        self.from.is_some()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn bind_owner(&mut self, color: PieceColor, game_id: Option<String>) {
        self.owner = Some((color, game_id));
    }

    pub fn owned_by(&self, color: PieceColor, game_id: Option<&str>) -> bool {
        self.owner
            .as_ref()
            .is_some_and(|(c, id)| *c == color && id.as_deref() == game_id)
    }

    pub fn bind_seat(&mut self, seat: Option<(u64, i64)>) {
        self.seat = seat;
    }

    pub fn belongs_to_seat(&self, seat: Option<(u64, i64)>) -> bool {
        self.seat == seat
    }

    pub fn promotion(&self) -> Option<PieceType> {
        self.promotion
    }

    /// A queued promotion defaults to queen; callers may choose an underpromotion.
    pub fn choose_promotion(&mut self, kind: PieceType) {
        if matches!(
            kind,
            PieceType::Queen | PieceType::Rook | PieceType::Bishop | PieceType::Knight
        ) {
            self.promotion = Some(kind);
        }
    }

    /// Whether `square` is one of the queued squares, for highlighting.
    pub fn marks(&self, square: (u8, u8)) -> bool {
        self.from == Some(square) || self.to == Some(square)
    }

    /// One click while queueing. `own_piece_here` says the square holds a piece
    /// the local player may premove. Clicking the picked piece again, or a
    /// third square once both are chosen, cancels; clicking another own piece
    /// replaces the pick.
    pub fn click(&mut self, square: (u8, u8), own_piece_here: bool, epoch: u64) {
        if own_piece_here {
            if self.from == Some(square) {
                self.clear();
            } else {
                *self = Self {
                    from: Some(square),
                    to: None,
                    epoch,
                    owner: self.owner.clone(),
                    seat: self.seat,
                    promotion: None,
                };
            }
        } else if self.from.is_some() && self.to.is_none() {
            self.to = Some(square);
        } else {
            self.clear();
        }
    }

    /// Whether the queued squares were chosen on the board `epoch` identifies.
    pub fn belongs_to(&self, epoch: u64) -> bool {
        self.epoch == epoch
    }

    /// Returns the queued squares if both are set and empties the queue either
    /// way. The caller still has to validate them against the live position.
    pub fn take(&mut self) -> Option<((u8, u8), (u8, u8))> {
        let taken = self.from.zip(self.to);
        self.clear();
        taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_clicks_queue_a_move() {
        let mut premove = Premove::default();
        premove.click((4, 6), true, 7);
        assert_eq!(premove.from(), Some((4, 6)));
        assert!(!premove.is_set());
        premove.click((4, 4), false, 7);
        assert!(premove.is_set());
        assert!(premove.marks((4, 6)) && premove.marks((4, 4)));
        assert_eq!(premove.take(), Some(((4, 6), (4, 4))));
        assert!(!premove.is_active());
    }

    #[test]
    fn clicking_the_picked_piece_again_cancels() {
        let mut premove = Premove::default();
        premove.click((4, 6), true, 1);
        premove.click((4, 6), true, 1);
        assert_eq!(premove, Premove::default());
    }

    #[test]
    fn another_own_piece_replaces_the_pick_even_after_a_target() {
        let mut premove = Premove::default();
        premove.click((4, 6), true, 1);
        premove.click((4, 4), false, 1);
        premove.click((3, 6), true, 1);
        assert_eq!(premove.from(), Some((3, 6)));
        assert_eq!(premove.to(), None);
    }

    #[test]
    fn a_third_empty_click_cancels() {
        let mut premove = Premove::default();
        premove.click((4, 6), true, 1);
        premove.click((4, 4), false, 1);
        premove.click((0, 0), false, 1);
        assert!(!premove.is_active());
    }

    #[test]
    fn an_empty_first_click_queues_nothing() {
        let mut premove = Premove::default();
        premove.click((0, 0), false, 1);
        assert!(!premove.is_active());
    }

    #[test]
    fn remembers_the_board_it_was_picked_on() {
        let mut premove = Premove::default();
        premove.click((4, 6), true, 3);
        assert!(premove.belongs_to(3));
        assert!(!premove.belongs_to(4));
    }
}
