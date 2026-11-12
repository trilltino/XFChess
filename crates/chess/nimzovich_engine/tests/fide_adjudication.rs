use nimzovich_engine::{on_chain_rules::*, validate_and_apply, CompactBoard, MoveOutcome};
use shakmaty::{fen::Fen, uci::UciMove, CastlingMode, Chess, Position};
use xfchess_adjudication::*;

type TestResult = Result<(), String>;
fn checked<T>(result: Result<T, impl core::fmt::Debug>) -> Result<T, String> {
    result.map_err(|error| format!("{error:?}"))
}
fn reference(fen: &str) -> Result<Chess, String> {
    let setup: Fen = checked(fen.parse())?;
    checked(setup.into_position(CastlingMode::Standard))
}
fn bytes(uci: &str) -> Result<[u8; 5], String> {
    if !matches!(uci.len(), 4 | 5) || !uci.is_ascii() {
        return Err("bad fixture UCI".into());
    }
    let mut out = [0; 5];
    out[..uci.len()].copy_from_slice(uci.as_bytes());
    Ok(out)
}
fn apply_reference(position: &mut Chess, uci: &str) -> TestResult {
    let mv: UciMove = checked(uci.parse())?;
    let mv = checked(mv.to_move(position))?;
    position.play_unchecked(mv);
    Ok(())
}

#[test]
fn knight_cycle_live_replay_and_checkpoint_match_reference() -> TestResult {
    let mut reference = Chess::default();
    let mut board = CompactBoard::starting_position().to_on_chain_game();
    let start = checked(canonical_position_key(&board.to_compact_board()))?;
    let mut keys = [PositionKey::default(); MAX_REVERSIBLE_POSITIONS];
    let mut tracker = checked(RepetitionTracker::from_start_position(
        &mut keys, start, 0, 0,
    ))?;
    for ply in 1..=16u32 {
        let uci = ["g1f3", "g8f6", "f3g1", "f6g8"][((ply - 1) % 4) as usize];
        assert_eq!(
            checked(validate_and_apply(&mut board, &bytes(uci)?))?,
            MoveOutcome::Playing
        );
        apply_reference(&mut reference, uci)?;
        let key = checked(canonical_position_key(&board.to_compact_board()))?;
        assert_eq!(key, checked(PositionKey::from_shakmaty(&reference))?);
        checked(tracker.record_accepted_position(key, ply, false))?;
        if ply == 8 {
            assert!(tracker.current_claims().repetition);
        }
        assert_eq!(
            automatic_termination(
                PositionFacts::default(),
                tracker.occurrences(key),
                ply as u16
            ),
            if ply == 16 {
                Some(Termination::FivefoldRepetition)
            } else {
                None
            }
        );
    }
    let checkpoint = tracker.checkpoint();
    let restored = checked(RepetitionTracker::restore(&mut keys, checkpoint))?;
    assert_eq!(restored.occurrences(start), 5);
    Ok(())
}

#[test]
fn en_passant_identity_accounts_for_pins_and_missing_capturers() -> TestResult {
    for fen in [
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
        "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2",
        "k3r3/8/8/3pP3/8/8/8/4K3 w - d6 0 2",
        "8/8/8/r4pPK/8/8/8/k7 w - f6 0 2",
    ] {
        let board = CompactBoard::from_fen(fen);
        let key = checked(canonical_position_key(&board))?;
        let reference = reference(fen)?;
        assert_eq!(
            key,
            checked(PositionKey::from_shakmaty(&reference))?,
            "{fen}"
        );
        let mut without_ep = board;
        without_ep.ep_target = -1;
        assert_eq!(
            key == checked(canonical_position_key(&without_ep))?,
            reference.legal_ep_square().is_none(),
            "{fen}"
        );
    }
    Ok(())
}

#[test]
fn different_move_orders_share_identity_but_returned_rooks_lose_rights() -> TestResult {
    let mut final_keys = Vec::new();
    for sequence in [
        ["g1f3", "g8f6", "b1c3", "b8c6"],
        ["b1c3", "b8c6", "g1f3", "g8f6"],
    ] {
        let mut position = CompactBoard::starting_position().to_on_chain_game();
        for uci in sequence {
            checked(validate_and_apply(&mut position, &bytes(uci)?))?;
        }
        final_keys.push(checked(canonical_position_key(
            &position.to_compact_board(),
        ))?);
    }
    assert_eq!(final_keys[0], final_keys[1]);
    let initial = CompactBoard::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    let mut position = initial.to_on_chain_game();
    for uci in ["h1h2", "a8a7", "h2h1", "a7a8"] {
        checked(validate_and_apply(&mut position, &bytes(uci)?))?;
    }
    assert_eq!(initial.squares, position.board);
    assert_ne!(
        checked(canonical_position_key(&initial))?,
        checked(canonical_position_key(&position.to_compact_board()))?
    );
    Ok(())
}

#[test]
fn promotion_must_be_explicit_and_invalid_suffixes_are_atomic() -> TestResult {
    let initial = CompactBoard::from_fen("8/P7/4k3/8/8/4K3/8/8 w - - 0 1").to_on_chain_game();
    for uci in ["a7a8", "a7a8x", "a7a8k", "a7a8p"] {
        let mut preview = initial;
        assert!(
            validate_and_apply(&mut preview, &bytes(uci)?).is_err(),
            "{uci}"
        );
        assert_eq!(preview, initial);
    }
    for suffix in ['q', 'r', 'b', 'n'] {
        let uci = format!("a7a8{suffix}");
        let mut preview = initial;
        checked(validate_and_apply(&mut preview, &bytes(&uci)?))?;
        let mut reference = reference("8/P7/4k3/8/8/4K3/8/8 w - - 0 1")?;
        apply_reference(&mut reference, &uci)?;
        assert_eq!(
            checked(canonical_position_key(&preview.to_compact_board()))?,
            checked(PositionKey::from_shakmaty(&reference))?
        );
    }
    for uci in ["e2e4q", "g1f3n", "e2e4x"] {
        let mut initial = CompactBoard::starting_position().to_on_chain_game();
        let before = initial;
        assert!(validate_and_apply(&mut initial, &bytes(uci)?).is_err());
        assert_eq!(initial, before);
    }
    Ok(())
}

#[test]
fn malformed_compact_boards_fail_without_panics_or_mutation() -> TestResult {
    let good = CompactBoard::starting_position();
    for (index, value) in [(0, 128), (4, 0), (64, 16), (65, 127), (66, 0), (7, 0)] {
        let mut raw = good.to_bytes();
        raw[index] = value;
        let board = CompactBoard::from_bytes(&raw);
        assert!(canonical_position_key(&board).is_err(), "index {index}");
        let mut game = board.to_on_chain_game();
        let before = game;
        assert!(validate_and_apply(&mut game, &bytes("e2e4")?).is_err());
        assert_eq!(game, before);
    }
    let mut pad = good;
    pad._pad = 99;
    assert_eq!(
        checked(canonical_position_key(&good))?,
        checked(canonical_position_key(&pad))?
    );
    let mut inconsistent = good.to_on_chain_game();
    inconsistent.white_kings = 0;
    assert!(validate_and_apply(&mut inconsistent, &bytes("e2e4")?).is_err());
    Ok(())
}

#[test]
fn mating_impossibility_is_directional_and_two_knights_are_not_dead() -> TestResult {
    for piece in ["B", "N"] {
        let fen = format!("8/8/4k3/8/8/4K3/8/5{piece}2 w - - 0 1");
        let board = CompactBoard::from_fen(&fen);
        assert_eq!(
            checked(mating_possibility(&board, 1))?,
            MatingPossibility::Impossible
        );
        assert_eq!(
            checked(mating_possibility(&board, -1))?,
            MatingPossibility::Impossible
        );
    }
    let board = CompactBoard::from_fen("8/8/4k3/8/8/4K3/8/1N3N2 w - - 0 1");
    assert_eq!(
        checked(mating_possibility(&board, 1))?,
        MatingPossibility::Unproven
    );
    assert_eq!(
        checked(mating_possibility(&board, -1))?,
        MatingPossibility::Impossible
    );
    Ok(())
}

#[test]
fn legal_move_sets_and_keys_match_shakmaty_across_seeded_games() -> TestResult {
    let mut seed = 0xF1DE_2026_u64;
    for game in 0..12 {
        let mut position = Chess::default();
        let mut board = CompactBoard::starting_position().to_on_chain_game();
        for ply in 0..160 {
            let legal = position.legal_moves();
            // Compare all candidates against the independent legal-move list,
            // including every explicit promotion and every illegal geometry.
            for from in 0..64i8 {
                if board.board[from as usize] * board.side_to_move <= 0 {
                    continue;
                }
                for to in 0..64i8 {
                    let promotes = board.board[from as usize].abs() == 1 && matches!(to / 8, 0 | 7);
                    let suffixes: &[i8] = if promotes { &[2, 3, 4, 5] } else { &[0] };
                    for &promotion in suffixes {
                        let mut candidate = board;
                        let actual = nimzovich_engine::validate_and_apply_sq(
                            &mut candidate,
                            from,
                            to,
                            promotion,
                        )
                        .is_ok();
                        let expected = legal.iter().any(|mv| {
                            let normal = UciMove::from_move(*mv, CastlingMode::Standard);
                            matches!(normal, UciMove::Normal { from: f, to: t, promotion: p }
                                if i8::from(f) == from && i8::from(t) == to && p.map_or(0, |r| r as i8) == promotion)
                        });
                        assert_eq!(
                            actual, expected,
                            "seed {seed:x}, game {game}, ply {ply}, {from}->{to} promo {promotion}"
                        );
                    }
                }
            }
            if legal.is_empty() {
                break;
            }
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let mv = legal[(seed as usize) % legal.len()];
            let uci = UciMove::from_move(mv, CastlingMode::Standard).to_string();
            checked(validate_and_apply(&mut board, &bytes(&uci)?))?;
            position.play_unchecked(mv);
            assert_eq!(
                checked(canonical_position_key(&board.to_compact_board()))?,
                checked(PositionKey::from_shakmaty(&position))?,
                "game {game}, ply {ply}"
            );
        }
    }
    Ok(())
}
