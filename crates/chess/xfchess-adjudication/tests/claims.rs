use xfchess_adjudication::*;
type TestResult = Result<(), ClaimError>;
fn request() -> ClaimRequest {
    ClaimRequest {
        game: [7; 32],
        ply: 8,
        nonce: 0,
        claimant: 1,
        reason: ClaimReason::Repetition,
        intended_move: Some(*b"g1f3\0"),
    }
}

#[test]
fn correct_claim_pauses_clock_and_ends_without_playing_declared_move() -> TestResult {
    let mut clock = AdjudicatedClock::new(300_000, 5_000, 1, 100)?;
    let mut claim = ClaimProcedure::new([7; 32]);
    claim.receive_authenticated(request(), 8, &mut clock, 10_100)?;
    assert_eq!(clock.remaining(1)?, 290_000);
    assert!(clock.is_paused());
    assert_eq!(clock.flag_fallen(1_000_000)?, None);
    assert_eq!(
        claim.check_move_allowed(*b"g1f3\0"),
        Err(ClaimError::Pending)
    );
    let decision = claim.resolve(
        ClaimAvailability {
            repetition: true,
            fifty_moves: false,
        },
        &mut clock,
        1_000_000,
    )?;
    assert_eq!(
        decision,
        ClaimDecision::Confirmed(Termination::ThreefoldClaim)
    );
    assert_eq!(claim.mandatory_move(), None);
    assert_eq!(
        claim.check_move_allowed(*b"g1f3\0"),
        Err(ClaimError::Finished)
    );
    assert!(clock.is_paused());
    assert_eq!(clock.remaining(-1)?, 300_000);
    assert_eq!(
        claim.resolve(ClaimAvailability::default(), &mut clock, 1_000_001),
        Err(ClaimError::NotPending)
    );
    Ok(())
}

#[test]
fn false_claim_adds_two_minutes_once_and_binds_the_declared_move() -> TestResult {
    let mut clock = AdjudicatedClock::new(300_000, 5_000, 1, 0)?;
    let mut claim = ClaimProcedure::new([7; 32]);
    claim.receive_authenticated(request(), 8, &mut clock, 10_000)?;
    let decision = claim.resolve(ClaimAvailability::default(), &mut clock, 90_000)?;
    assert_eq!(
        decision,
        ClaimDecision::IncorrectPenalized {
            opponent: -1,
            mandatory_move: Some(*b"g1f3\0")
        }
    );
    assert_eq!(clock.remaining(-1)?, 420_000);
    assert_eq!(clock.remaining(1)?, 290_000, "adjudication time is paused");
    assert!(!clock.is_paused());
    assert_eq!(
        claim.receive_authenticated(request(), 8, &mut clock, 90_001),
        Err(ClaimError::StaleNonce)
    );
    assert_eq!(
        claim.resolve(ClaimAvailability::default(), &mut clock, 90_001),
        Err(ClaimError::NotPending)
    );
    assert_eq!(clock.remaining(-1)?, 420_000);
    assert_eq!(
        claim.check_move_allowed(*b"b1c3\0"),
        Err(ClaimError::MandatoryMove)
    );
    claim.check_move_allowed(*b"g1f3\0")?;
    clock.complete_move(100_000)?;
    claim.record_accepted_move(*b"g1f3\0", 1)?;
    assert_eq!(claim.draw_offered_by(), Some(1));
    assert_eq!(claim.mandatory_move(), None);
    assert_eq!(clock.remaining(1)?, 285_000);
    assert_eq!(clock.side_to_move(), -1);
    Ok(())
}

#[test]
fn stale_or_wrong_seat_requests_never_pause_or_penalize() -> TestResult {
    let initial_clock = AdjudicatedClock::new(300_000, 0, 1, 0)?;
    let initial_claim = ClaimProcedure::new([7; 32]);
    for (bad, error) in [
        (
            ClaimRequest {
                game: [8; 32],
                ..request()
            },
            ClaimError::WrongGame,
        ),
        (
            ClaimRequest {
                ply: 7,
                ..request()
            },
            ClaimError::StalePly,
        ),
        (
            ClaimRequest {
                nonce: 1,
                ..request()
            },
            ClaimError::StaleNonce,
        ),
        (
            ClaimRequest {
                claimant: -1,
                ..request()
            },
            ClaimError::WrongTurn,
        ),
    ] {
        let mut clock = initial_clock;
        let mut claim = initial_claim;
        assert_eq!(
            claim.receive_authenticated(bad, 8, &mut clock, 10),
            Err(error)
        );
        assert_eq!(clock, initial_clock);
        assert_eq!(claim, initial_claim);
    }
    Ok(())
}

#[test]
fn exact_flag_boundary_and_monotonic_time_precede_new_claims() -> TestResult {
    let mut clock = AdjudicatedClock::new(1_000, 0, 1, 100)?;
    assert_eq!(clock.flag_fallen(1_099)?, None);
    assert_eq!(clock.flag_fallen(1_100)?, Some(1));
    let mut claim = ClaimProcedure::new([7; 32]);
    assert_eq!(
        claim.receive_authenticated(request(), 8, &mut clock, 99),
        Err(ClaimError::Clock(ClockError::TimeWentBackwards))
    );
    assert_eq!(
        claim.receive_authenticated(request(), 8, &mut clock, 1_100),
        Err(ClaimError::Clock(ClockError::FlagFallen(1)))
    );
    assert!(!clock.is_paused());
    assert_eq!(claim.next_nonce(), 0);
    Ok(())
}

#[test]
fn malformed_intent_never_pauses_or_consumes_a_claim_nonce() -> TestResult {
    let initial = AdjudicatedClock::new(300_000, 0, 1, 0)?;
    for intended in [*b"g1g1\0", *b"i1f3\0", *b"g1f3x"] {
        let mut clock = initial;
        let mut claim = ClaimProcedure::new([7; 32]);
        assert_eq!(
            claim.receive_authenticated(
                ClaimRequest {
                    intended_move: Some(intended),
                    ..request()
                },
                8,
                &mut clock,
                1
            ),
            Err(ClaimError::MalformedIntent)
        );
        assert_eq!(claim.next_nonce(), 0);
        assert_eq!(clock, initial);
    }
    Ok(())
}

#[test]
fn prospective_fifty_move_claim_is_terminal_and_current_false_claim_is_penalized() -> TestResult {
    let mut clock = AdjudicatedClock::new(300_000, 0, 1, 0)?;
    let mut claim = ClaimProcedure::new([7; 32]);
    claim.receive_authenticated(
        ClaimRequest {
            reason: ClaimReason::FiftyMoves,
            ..request()
        },
        8,
        &mut clock,
        1,
    )?;
    let before = clock;
    assert_eq!(
        claim.resolve(
            ClaimAvailability {
                repetition: false,
                fifty_moves: true
            },
            &mut clock,
            0
        ),
        Err(ClaimError::Clock(ClockError::TimeWentBackwards))
    );
    assert_eq!(clock, before);
    assert!(claim.pending().is_some());
    assert_eq!(
        claim.resolve(
            ClaimAvailability {
                repetition: false,
                fifty_moves: true
            },
            &mut clock,
            2
        )?,
        ClaimDecision::Confirmed(Termination::FiftyMoveClaim)
    );
    let mut clock = AdjudicatedClock::new(300_000, 0, 1, 0)?;
    let mut claim = ClaimProcedure::new([7; 32]);
    claim.receive_authenticated(
        ClaimRequest {
            intended_move: None,
            ..request()
        },
        8,
        &mut clock,
        1,
    )?;
    assert_eq!(
        claim.resolve(ClaimAvailability::default(), &mut clock, 2)?,
        ClaimDecision::IncorrectPenalized {
            opponent: -1,
            mandatory_move: None
        }
    );
    assert_eq!(claim.draw_offered_by(), Some(1));
    claim.record_accepted_move(*b"g1f3 ", 1)?;
    assert_eq!(claim.draw_offered_by(), Some(1));
    claim.record_accepted_move(*b"g8f6\0", -1)?;
    assert_eq!(claim.draw_offered_by(), None);
    Ok(())
}
