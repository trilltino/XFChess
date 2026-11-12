use xfchess_adjudication::*;

type TestResult = Result<(), RulesError>;

fn key(side: i8, rights: u8, salt: usize) -> Result<PositionKey, RulesError> {
    let mut squares = [0; 64];
    squares[4] = 6;
    squares[60] = -6;
    squares[salt % 4] = 4;
    PositionKey::from_parts(&squares, rights, side, None)
}

#[test]
fn exact_identity_preserves_side_rights_and_legal_ep() -> TestResult {
    let k = key(1, 15, 0)?;
    assert_ne!(k, key(-1, 15, 0)?);
    assert_ne!(k, key(1, 14, 0)?);
    assert_ne!(k, key(1, 15, 1)?);
    assert_eq!(PositionKey::from_bytes(k.to_bytes())?, k);
    let mut squares = [0; 64];
    let absent = PositionKey::from_parts(&squares, 0, 1, None)?;
    assert_ne!(absent, PositionKey::from_parts(&squares, 0, 1, Some(40))?);
    assert_eq!(
        PositionKey::from_parts(&squares, 0, 1, Some(20)),
        Err(RulesError::InvalidPosition)
    );
    squares[0] = i8::MIN;
    assert_eq!(
        PositionKey::from_parts(&squares, 0, 1, None),
        Err(RulesError::InvalidPosition)
    );
    for (index, value) in [(0, 13), (32, 32), (33, 9)] {
        let mut bytes = k.to_bytes();
        bytes[index] = value;
        assert!(PositionKey::from_bytes(bytes).is_err());
    }
    Ok(())
}

#[test]
fn third_is_claimable_fifth_is_automatic_and_initial_counts() -> TestResult {
    let mut storage = [PositionKey::default(); MAX_REVERSIBLE_POSITIONS];
    let start = key(1, 15, 0)?;
    let mut tracker = RepetitionTracker::from_start_position(&mut storage, start, 0, 0)?;
    for ply in 1..=16 {
        let next = key(if ply % 2 == 0 { 1 } else { -1 }, 15, (ply % 4) as usize)?;
        if ply == 8 {
            assert!(tracker.prospective_claims(next, false)?.repetition);
            assert!(!tracker.current_claims().repetition);
        }
        tracker.record_accepted_position(next, ply, false)?;
        let count = tracker.occurrences(tracker.current_key());
        assert_eq!(tracker.current_claims().repetition, ply >= 8);
        assert_eq!(
            automatic_termination(PositionFacts::default(), count, ply as u16),
            if ply == 16 {
                Some(Termination::FivefoldRepetition)
            } else {
                None
            }
        );
    }
    assert_eq!(tracker.occurrences(start), 5);
    assert_eq!(
        tracker.record_accepted_position(key(-1, 15, 1)?, 17, false),
        Err(RulesError::AlreadyAutomatic)
    );
    Ok(())
}

#[test]
fn fifty_is_a_claim_and_seventy_five_is_automatic_unless_mate() {
    let facts = PositionFacts::default();
    for halfmoves in [99, 100, 101, 149, 150] {
        assert_eq!(
            claim_availability(1, halfmoves).fifty_moves,
            halfmoves >= 100
        );
        assert_eq!(
            automatic_termination(facts, 1, halfmoves),
            if halfmoves == 150 {
                Some(Termination::SeventyFiveMoves)
            } else {
                None
            }
        );
    }
    assert_eq!(
        automatic_termination(
            PositionFacts {
                checkmate: true,
                ..facts
            },
            5,
            150
        ),
        Some(Termination::Checkmate)
    );
    assert!(!agreement_permitted(0));
    assert!(!agreement_permitted(1));
    assert!(agreement_permitted(2));
}

#[test]
fn prospective_query_is_read_only_and_capture_cannot_claim_fifty() -> TestResult {
    let mut storage = [PositionKey::default(); 2];
    let mut tracker = RepetitionTracker::from_start_position(&mut storage, key(1, 0, 0)?, 200, 99)?;
    let before = tracker.checkpoint();
    assert!(
        tracker
            .prospective_claims(key(-1, 0, 1)?, false)?
            .fifty_moves
    );
    assert!(
        !tracker
            .prospective_claims(key(-1, 0, 1)?, true)?
            .fifty_moves
    );
    assert_eq!(tracker.checkpoint(), before);
    tracker.record_accepted_position(key(-1, 0, 1)?, 201, false)?;
    assert!(tracker.current_claims().fifty_moves);
    Ok(())
}

#[test]
fn rejected_updates_are_atomic_and_irreversible_moves_bound_storage() -> TestResult {
    let mut storage = [PositionKey::default(); 2];
    let mut tracker = RepetitionTracker::from_start_position(&mut storage, key(1, 15, 0)?, 0, 0)?;
    assert_eq!(
        tracker.record_accepted_position(key(-1, 15, 1)?, 2, false),
        Err(RulesError::OutOfOrder)
    );
    tracker.record_accepted_position(key(-1, 15, 1)?, 1, false)?;
    let before = tracker.checkpoint();
    let position = tracker.current_key();
    assert_eq!(
        tracker.record_accepted_position(key(1, 15, 2)?, 2, false),
        Err(RulesError::Capacity)
    );
    assert_eq!(tracker.checkpoint(), before);
    assert_eq!(tracker.current_key(), position);
    assert_eq!(
        tracker.record_accepted_position(key(1, 15, 2)?, 1, false),
        Err(RulesError::OutOfOrder)
    );
    tracker.record_accepted_position(key(1, 14, 2)?, 2, false)?;
    assert_eq!(
        tracker.checkpoint().len,
        1,
        "lost castling rights reset repetition only"
    );
    assert_eq!(tracker.checkpoint().halfmoves, 2);
    assert_eq!(
        tracker.prospective_claims(key(-1, 15, 3)?, false),
        Err(RulesError::InvalidPosition)
    );
    tracker.record_accepted_position(key(-1, 14, 3)?, 3, true)?;
    assert_eq!(tracker.checkpoint().len, 1);
    assert_eq!(tracker.checkpoint().halfmoves, 0);
    let checkpoint = tracker.checkpoint();
    let restored = RepetitionTracker::restore(&mut storage, checkpoint)?;
    assert_eq!(restored.checkpoint(), checkpoint);
    assert_eq!(restored.occurrences(restored.current_key()), 1);
    Ok(())
}

#[test]
fn invalid_checkpoints_and_ply_overflow_fail_closed() -> TestResult {
    let mut storage = [PositionKey::default(); 2];
    let mut tracker =
        RepetitionTracker::from_start_position(&mut storage, key(1, 0, 0)?, u32::MAX, 0)?;
    assert_eq!(
        tracker.record_accepted_position(key(-1, 0, 1)?, 0, false),
        Err(RulesError::Overflow)
    );
    for checkpoint in [
        RepetitionCheckpoint {
            len: 0,
            ply: 0,
            halfmoves: 0,
        },
        RepetitionCheckpoint {
            len: 3,
            ply: 2,
            halfmoves: 2,
        },
        RepetitionCheckpoint {
            len: 2,
            ply: 1,
            halfmoves: 0,
        },
        RepetitionCheckpoint {
            len: 1,
            ply: 1,
            halfmoves: 151,
        },
    ] {
        assert!(RepetitionTracker::restore(&mut storage, checkpoint).is_err());
    }
    Ok(())
}

#[test]
fn entire_150_ply_window_is_retained_without_eviction() -> TestResult {
    // Synthetic distinct keys isolate the storage predicate; legal threshold
    // and move validation fixtures live in the engine differential suite.
    let unique = |ply: u32| {
        let mut squares = [0; 64];
        squares[4] = 6;
        squares[60] = -6;
        squares[0] = (ply % 6 + 1) as i8;
        squares[1] = ((ply / 6) % 6 + 1) as i8;
        squares[2] = ((ply / 36) % 6 + 1) as i8;
        PositionKey::from_parts(
            &squares,
            0,
            if ply.is_multiple_of(2) { 1 } else { -1 },
            None,
        )
    };
    let mut storage = [PositionKey::default(); MAX_REVERSIBLE_POSITIONS];
    let start = unique(0)?;
    let mut history = RepetitionTracker::from_start_position(&mut storage, start, 0, 0)?;
    for ply in 1..=150 {
        history.record_accepted_position(unique(ply)?, ply, false)?;
    }
    assert_eq!(history.checkpoint().len, 151);
    assert_eq!(history.occurrences(start), 1);
    let checkpoint = history.checkpoint();
    assert_eq!(
        history.record_accepted_position(unique(151)?, 151, true),
        Err(RulesError::AlreadyAutomatic)
    );
    assert_eq!(history.checkpoint(), checkpoint);
    for entry in &mut storage {
        *entry = PositionKey::from_bytes(entry.to_bytes())?;
    }
    let restored = RepetitionTracker::restore(&mut storage, checkpoint)?;
    assert_eq!(restored.checkpoint(), checkpoint);
    assert_eq!(restored.occurrences(start), 1);
    Ok(())
}
