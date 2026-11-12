use nimzovich_engine::{on_chain_replay::*, on_chain_rules::*, validate_and_apply, CompactBoard};
use xfchess_adjudication::*;
type TestResult = Result<(), String>;
fn checked<T>(result: Result<T, impl core::fmt::Debug>) -> Result<T, String> {
    result.map_err(|e| format!("{e:?}"))
}
fn next(board: &CompactBoard, uci: &[u8; 5]) -> Result<[u8; 68], String> {
    let mut position = board.to_on_chain_game();
    checked(validate_and_apply(&mut position, uci))?;
    Ok(position.to_compact_board().to_bytes())
}

#[test]
fn verified_replay_and_incremental_history_agree_through_fivefold() -> TestResult {
    let mut board = CompactBoard::starting_position();
    let mut storage = [PositionKey::default(); MAX_REVERSIBLE_POSITIONS];
    let mut history = checked(RepetitionTracker::from_start_position(
        &mut storage,
        checked(canonical_position_key(&board))?,
        0,
        0,
    ))?;
    for ply in 1..=16u32 {
        let uci = [*b"g1f3\0", *b"g8f6\0", *b"f3g1\0", *b"f6g8\0"][((ply - 1) % 4) as usize];
        let expected = next(&board, &uci)?;
        let terminal = checked(record_verified_move(
            &mut board,
            &mut history,
            &uci,
            &expected,
            ply,
        ))?;
        assert_eq!(
            terminal,
            if ply == 16 {
                Some(Termination::FivefoldRepetition)
            } else {
                None
            }
        );
    }
    let expected = next(&board, b"g1f3\0")?;
    assert_eq!(
        record_verified_move(&mut board, &mut history, b"g1f3\0", &expected, 17),
        Err(ReplayError::Finished)
    );
    Ok(())
}

#[test]
fn illegal_duplicate_or_mismatched_plies_do_not_mutate_either_state() -> TestResult {
    let mut board = CompactBoard::starting_position();
    let before = board;
    let mut storage = [PositionKey::default(); 2];
    let mut history = checked(RepetitionTracker::from_start_position(
        &mut storage,
        checked(canonical_position_key(&board))?,
        0,
        0,
    ))?;
    let checkpoint = history.checkpoint();
    let expected = next(&board, b"g1f3\0")?;
    for (uci, next_board, ply, error) in [
        (*b"e2e5\0", expected, 1, ReplayError::InvalidMove),
        (*b"g1f3\0", before.to_bytes(), 1, ReplayError::BoardMismatch),
        (
            *b"g1f3\0",
            expected,
            0,
            ReplayError::Rules(RulesError::OutOfOrder),
        ),
        (
            *b"g1f3\0",
            expected,
            2,
            ReplayError::Rules(RulesError::OutOfOrder),
        ),
    ] {
        assert_eq!(
            record_verified_move(&mut board, &mut history, &uci, &next_board, ply),
            Err(error)
        );
        assert_eq!(board, before);
        assert_eq!(history.checkpoint(), checkpoint);
    }
    checked(record_verified_move(
        &mut board,
        &mut history,
        b"g1f3\0",
        &expected,
        1,
    ))?;
    let accepted = board;
    let checkpoint = history.checkpoint();
    let expected = next(&board, b"g8f6\0")?;
    assert_eq!(
        record_verified_move(&mut board, &mut history, b"g8f6\0", &expected, 2),
        Err(ReplayError::Rules(RulesError::Capacity))
    );
    assert_eq!(history.checkpoint(), checkpoint);
    assert_eq!(board, accepted);
    Ok(())
}

#[test]
fn hundred_continues_one_fifty_draws_but_mate_has_precedence() -> TestResult {
    for (fen, halfmoves, uci, expected_reason) in [
        ("8/8/4k3/8/8/4K3/8/R7 w - - 99 80", 99, *b"a1a2\0", None),
        ("8/8/4k3/8/8/4K3/8/R7 w - - 100 80", 100, *b"a1a2\0", None),
        (
            "8/8/4k3/8/8/4K3/8/R7 w - - 149 80",
            149,
            *b"a1a2\0",
            Some(Termination::SeventyFiveMoves),
        ),
        (
            "7k/8/5KQ1/8/8/8/8/8 w - - 149 80",
            149,
            *b"g6g7\0",
            Some(Termination::Checkmate),
        ),
    ] {
        let mut board = CompactBoard::from_fen(fen);
        let mut storage = [PositionKey::default(); 2];
        let mut history = checked(RepetitionTracker::from_start_position(
            &mut storage,
            checked(canonical_position_key(&board))?,
            158,
            halfmoves,
        ))?;
        let expected = next(&board, &uci)?;
        assert_eq!(
            checked(record_verified_move(
                &mut board,
                &mut history,
                &uci,
                &expected,
                159
            ))?,
            expected_reason
        );
        assert_eq!(history.checkpoint().halfmoves, halfmoves + 1);
    }
    Ok(())
}

#[test]
fn general_locked_dead_positions_remain_an_explicit_release_gap() -> TestResult {
    // Every pawn is blocked. Kings cannot cross the attacked holes in the
    // barrier. Each bishop is trapped behind its own same-colour pawn wall,
    // so no check can ever be delivered. Material counting alone misses this.
    let board = CompactBoard::from_fen("4kb2/8/8/p1p1p1p1/P1P1P1P1/8/8/4KB2 w - - 0 1");
    checked(validate_compact_board(&board))?;
    assert_eq!(
        checked(mating_possibility(&board, 1))?,
        MatingPossibility::Unproven
    );
    assert_eq!(
        checked(mating_possibility(&board, -1))?,
        MatingPossibility::Unproven
    );
    Ok(())
}
