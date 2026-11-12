//! Legal move/history transaction used by replay and live adapters alike.
//! Callers bind authentication, game identity, nonce and timestamps separately.
use crate::{
    has_any_legal_move, is_in_check_fast,
    on_chain_rules::{canonical_position_key, mating_possibility, MatingPossibility},
    parse_uci, validate_and_apply, CompactBoard, MoveOutcome,
};
use xfchess_adjudication::{
    automatic_termination, PositionFacts, RepetitionTracker, RulesError, Termination,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayError {
    Rules(RulesError),
    InvalidMove,
    BoardMismatch,
    Finished,
}

impl From<RulesError> for ReplayError {
    fn from(error: RulesError) -> Self {
        Self::Rules(error)
    }
}

/// Reject stale/mismatched current boards and verified sequences that play on
/// after automatic termination. Illegal moves, wrong post-boards, duplicates and
/// overflow/capacity errors leave both official board and history unchanged.
pub fn record_verified_move(
    board: &mut CompactBoard,
    tracker: &mut RepetitionTracker<'_>,
    uci: &[u8; 5],
    expected_next: &[u8; 68],
    ply: u32,
) -> Result<Option<Termination>, ReplayError> {
    let current_key = canonical_position_key(board)?;
    if current_key != tracker.current_key() {
        return Err(ReplayError::BoardMismatch);
    }
    let mut position = board.to_on_chain_game();
    let has_moves = has_any_legal_move(&mut position, board.side_to_move);
    let current_facts = PositionFacts {
        checkmate: !has_moves && is_in_check_fast(&position, board.side_to_move),
        stalemate: !has_moves && !is_in_check_fast(&position, board.side_to_move),
        proven_dead: proven_dead(board)?,
    };
    if automatic_termination(
        current_facts,
        tracker.occurrences(current_key),
        tracker.checkpoint().halfmoves,
    )
    .is_some()
    {
        return Err(ReplayError::Finished);
    }
    let (from, to, _) = parse_uci(uci).map_err(|_| ReplayError::InvalidMove)?;
    let piece = board.squares[from as usize];
    let pawn_move_or_capture = matches!(piece, 1 | -1) || board.squares[to as usize] != 0;
    let outcome = validate_and_apply(&mut position, uci).map_err(|_| ReplayError::InvalidMove)?;
    let next = position.to_compact_board();
    if next.to_bytes() != *expected_next {
        return Err(ReplayError::BoardMismatch);
    }
    let key = canonical_position_key(&next)?;
    let facts = PositionFacts {
        checkmate: outcome == MoveOutcome::Checkmate,
        stalemate: outcome == MoveOutcome::Stalemate,
        proven_dead: proven_dead(&next)?,
    };
    tracker.record_accepted_position(key, ply, pawn_move_or_capture)?;
    *board = next;
    Ok(automatic_termination(
        facts,
        tracker.occurrences(key),
        tracker.checkpoint().halfmoves,
    ))
}

fn proven_dead(board: &CompactBoard) -> Result<bool, RulesError> {
    Ok(
        mating_possibility(board, 1)? == MatingPossibility::Impossible
            && mating_possibility(board, -1)? == MatingPossibility::Impossible,
    )
}
