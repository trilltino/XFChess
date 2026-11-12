//! Shared strict-FIDE primitives. Callers authenticate actions, validate chess
//! moves, and persist decisions atomically. Search scores are never evidence.
#![no_std]
#![forbid(unsafe_code)]

mod claims;
mod clock;
mod identity;
mod repetition;
pub use claims::*;
pub use clock::*;
pub use identity::*;
pub use repetition::*;

#[cfg(feature = "shakmaty")]
mod shakmaty_adapter;

pub const RULES_VERSION: u8 = 1;
pub const FIFTY_MOVE_HALFMOVES: u16 = 100;
pub const SEVENTY_FIVE_MOVE_HALFMOVES: u16 = 150;
pub const INCORRECT_CLAIM_PENALTY_MILLIS: u64 = 120_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RulesError {
    InvalidPosition,
    InvalidCheckpoint,
    OutOfOrder,
    Capacity,
    Overflow,
    AlreadyAutomatic,
}

/// A reason, independent of result/payout enum discriminants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Termination {
    Checkmate,
    Stalemate,
    DeadPosition,
    ThreefoldClaim,
    FiftyMoveClaim,
    FivefoldRepetition,
    SeventyFiveMoves,
    Agreement,
    Resignation,
    ResignationWithoutPossibleMate,
    FlagFall,
    FlagFallWithoutPossibleMate,
}

/// Legal-position facts supplied by a chess validator, never by a remote claim.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PositionFacts {
    pub checkmate: bool,
    pub stalemate: bool,
    pub proven_dead: bool,
}

/// Checkmate has precedence over the 150th reversible halfmove (9.6.2).
/// A third appearance and the 100th halfmove are deliberately nonterminal.
pub fn automatic_termination(
    facts: PositionFacts,
    occurrences: u16,
    halfmoves: u16,
) -> Option<Termination> {
    if facts.checkmate {
        Some(Termination::Checkmate)
    } else if facts.stalemate {
        Some(Termination::Stalemate)
    } else if facts.proven_dead {
        Some(Termination::DeadPosition)
    } else if occurrences >= 5 {
        Some(Termination::FivefoldRepetition)
    } else if halfmoves >= SEVENTY_FIVE_MOVE_HALFMOVES {
        Some(Termination::SeventyFiveMoves)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClaimAvailability {
    pub repetition: bool,
    pub fifty_moves: bool,
}

pub fn claim_availability(occurrences: u16, halfmoves: u16) -> ClaimAvailability {
    ClaimAvailability {
        repetition: occurrences >= 3,
        fifty_moves: halfmoves >= FIFTY_MOVE_HALFMOVES,
    }
}

pub fn agreement_permitted(accepted_plies: u32) -> bool {
    accepted_plies >= 2
}
