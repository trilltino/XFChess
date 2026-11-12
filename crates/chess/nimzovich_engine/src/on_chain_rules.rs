//! Checked adapter for allocation-free adjudication. This is independent of
//! search hashes and can be used with authoritative, legally replayed history.
use crate::{is_in_check_fast, validate_and_apply_sq, CompactBoard, OnChainGame};
use xfchess_adjudication::{PositionKey, RulesError};

/// Structural/legal-state checks, not a proof of reachability from move one.
/// Padding is intentionally ignored by canonical identity.
pub fn validate_compact_board(board: &CompactBoard) -> Result<(), RulesError> {
    if !matches!(board.side_to_move, -1 | 1)
        || board.castling & !15 != 0
        || !(-1..64).contains(&board.ep_target)
        || board
            .squares
            .iter()
            .any(|&piece| !(-6..=6).contains(&piece))
        || board.squares.iter().filter(|&&p| p == 6).count() != 1
        || board.squares.iter().filter(|&&p| p == -6).count() != 1
        || board.squares[..8]
            .iter()
            .chain(&board.squares[56..])
            .any(|&p| matches!(p, -1 | 1))
    {
        return Err(RulesError::InvalidPosition);
    }
    for (bit, king, rook, side) in [(1, 4, 7, 1), (2, 4, 0, 1), (4, 60, 63, -1), (8, 60, 56, -1)] {
        if board.castling & bit != 0
            && (board.squares[king] != side * 6 || board.squares[rook] != side * 4)
        {
            return Err(RulesError::InvalidPosition);
        }
    }
    if board.ep_target != -1 {
        let ep = board.ep_target;
        let side = board.side_to_move;
        if ep / 8 != if side == 1 { 5 } else { 2 }
            || board.squares[ep as usize] != 0
            || board.squares[(ep - side * 8) as usize] != -side
            || board.squares[(ep + side * 8) as usize] != 0
        {
            return Err(RulesError::InvalidPosition);
        }
    }
    // The side which just moved may not have left its king attacked. This
    // also rejects adjacent kings and both-kings-in-check positions.
    if is_in_check_fast(&board.to_on_chain_game(), -board.side_to_move) {
        return Err(RulesError::InvalidPosition);
    }
    Ok(())
}

/// Reject stale or forged bitboards before trusting fast attack lookups.
pub fn validate_on_chain_game(game: &OnChainGame) -> Result<(), RulesError> {
    let board = game.to_compact_board();
    validate_compact_board(&board)?;
    if board.to_on_chain_game() != *game {
        return Err(RulesError::InvalidPosition);
    }
    Ok(())
}

/// Raw FEN/compact EP is relevant only if at least one EP capture is legal,
/// including the moving king's safety after removal of both pawns.
pub fn legal_ep_square(board: &CompactBoard) -> Result<Option<u8>, RulesError> {
    validate_compact_board(board)?;
    if board.ep_target < 0 {
        return Ok(None);
    }
    let target = board.ep_target;
    let base = target - board.side_to_move * 8;
    for from in [base - 1, base + 1] {
        if !(0..64).contains(&from)
            || (from % 8 - target % 8).abs() != 1
            || board.squares[from as usize] != board.side_to_move
        {
            continue;
        }
        let mut preview = board.to_on_chain_game();
        if validate_and_apply_sq(&mut preview, from, target, 0).is_ok() {
            return Ok(Some(target as u8));
        }
    }
    Ok(None)
}

pub fn canonical_position_key(board: &CompactBoard) -> Result<PositionKey, RulesError> {
    PositionKey::from_parts(
        &board.squares,
        board.castling,
        board.side_to_move,
        legal_ep_square(board)?,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatingPossibility {
    /// A sound impossibility proof for this side, independent of cooperation.
    Impossible,
    /// Includes exotic locked positions: no conclusion, never evidence of a win.
    Unproven,
}

/// Sound material proofs only. Full legal reachability for locked positions is
/// not implemented; consumers must not certify strict FIDE completeness from
/// an Unproven result. K+NN versus K is deliberately Unproven.
pub fn mating_possibility(
    board: &CompactBoard,
    attacker: i8,
) -> Result<MatingPossibility, RulesError> {
    validate_compact_board(board)?;
    if !matches!(attacker, -1 | 1) {
        return Err(RulesError::InvalidPosition);
    }
    let own = |id| {
        board
            .squares
            .iter()
            .filter(|&&p| p == attacker * id)
            .count()
    };
    let other = |id| {
        board
            .squares
            .iter()
            .filter(|&&p| p == -attacker * id)
            .count()
    };
    if own(1) + own(4) + own(5) != 0 {
        return Ok(MatingPossibility::Unproven);
    }
    if own(2) + own(3) == 0 {
        return Ok(MatingPossibility::Impossible);
    }
    if own(2) + own(3) == 1 && (1..=5).all(|id| other(id) == 0) {
        return Ok(MatingPossibility::Impossible);
    }
    if board
        .squares
        .iter()
        .all(|p| matches!(p, 0 | 3 | -3 | 6 | -6))
    {
        let mut colours = 0u8;
        for (sq, &p) in board.squares.iter().enumerate() {
            if matches!(p, 3 | -3) {
                colours |= 1 << ((sq / 8 + sq % 8) & 1);
            }
        }
        if colours != 3 {
            return Ok(MatingPossibility::Impossible);
        }
    }
    Ok(MatingPossibility::Unproven)
}
