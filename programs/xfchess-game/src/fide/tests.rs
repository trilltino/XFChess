use super::*;
use anchor_lang::prelude::*;
use anyhow::ensure;
use crate::state::*;
use nimzovich_engine::{CompactBoard, validate_and_apply};

fn game(fen: &str) -> Game {
    Game {
        game_id: 42, white: Pubkey::new_unique(), black: Pubkey::new_unique(), status: GameStatus::Active,
        last_move_timestamp: 100, fees_advanced: 0, fee_payer: Pubkey::new_unique(), result: GameResult::None,
        board_state: CompactBoard::from_fen(fen).to_bytes(), move_count: 0, halfmove_clock: 0, turn: 1,
        created_at: 100, updated_at: 100, wager_amount: 1_000_000, wager_token: None, game_type: GameType::PvP,
        match_type: MatchType::Free, country_fee: 0, base_time_seconds: 300, increment_seconds: 0,
        bump: 0, is_delegated: true, tournament_id: None, nonce: 0, draw_offered_by: None,
    }
}

fn fixture(fen: &str, halfmoves: u16, f: impl FnOnce(&mut Game, &AccountInfo<'_>) -> anyhow::Result<()>) -> anyhow::Result<()> {
    let mut game = game(fen);
    let mut data = vec![0; FIDE_GAME_SPACE];
    FideState::initialize(&mut data, &game)?;
    if halfmoves != 0 {
        let mut state = FideState::load(&mut data, &game)?;
        state.put_u16(55, halfmoves)?;
        state.put_u32(51, 200)?;
        game.move_count = 200;
        game.turn = 201;
        game.nonce = 200;
        game.halfmove_clock = halfmoves;
    }
    let key = Pubkey::new_unique();
    let owner = crate::ID;
    let mut lamports = 123_456_789;
    let info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
    f(&mut game, &info)
}

fn play(game: &mut Game, info: &AccountInfo<'_>, uci: [u8; 5]) -> anyhow::Result<()> {
    let mut preview = CompactBoard::from_bytes(&game.board_state).to_on_chain_game();
    validate_and_apply(&mut preview, &uci).map_err(|_| anyhow::anyhow!("invalid fixture move"))?;
    let mover = if game.turn % 2 == 1 { game.white } else { game.black };
    apply_move(game, info, mover, uci, preview.to_compact_board().to_bytes(), game.nonce + 1, Some(game.nonce), 101)?;
    Ok(())
}

#[test]
fn third_claims_and_fifth_automatically_finishes_without_moving_money() -> anyhow::Result<()> {
    let start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    fixture(start, 0, |game, info| {
        let balance = info.lamports();
        for ply in 0..8 { play(game, info, [*b"g1f3\0", *b"g8f6\0", *b"f3g1\0", *b"f6g8\0"][ply % 4])?; }
        assert_eq!(game.status, GameStatus::Active);
        let current = game.board_state;
        adjudicate_claim(game, info, game.white, game.nonce, 0, None, true, 101)?;
        assert_eq!(game.status, GameStatus::Finished);
        assert_eq!(game.result, GameResult::Draw);
        assert_eq!(game.board_state, current);
        assert_eq!(info.lamports(), balance);
        assert!(adjudicate_claim(game, info, game.white, game.nonce, 0, None, true, 101).is_err());
        Ok(())
    })?;
    fixture(start, 0, |game, info| {
        for ply in 0..16 { play(game, info, [*b"g1f3\0", *b"g8f6\0", *b"f3g1\0", *b"f6g8\0"][ply % 4])?; }
        assert_eq!(game.result, GameResult::Draw);
        let mut bytes = info.try_borrow_mut_data()?;
        let state = FideState::load(&mut bytes, game)?;
        assert_eq!(state.bytes[10], FideTermination::FivefoldRepetition as u8);
        Ok(())
    })
}

#[test]
fn current_and_prospective_fifty_claims_preserve_board_and_100_is_nonterminal() -> anyhow::Result<()> {
    let fen = "8/8/4k3/8/8/4K3/8/R7 w - - 0 1";
    fixture(fen, 99, |game, info| {
        let before = game.board_state;
        adjudicate_claim(game, info, game.white, game.nonce, 0, Some(*b"a1a2\0"), false, 101)?;
        assert_eq!(game.result, GameResult::Draw);
        assert_eq!(game.board_state, before);
        assert_eq!(game.move_count, 200);
        Ok(())
    })?;
    fixture(fen, 99, |game, info| {
        play(game, info, *b"a1a2\0")?;
        assert_eq!(game.status, GameStatus::Active);
        assert_eq!(game.halfmove_clock, 100);
        adjudicate_claim(game, info, game.black, game.nonce, 0, None, false, 101)?;
        assert_eq!(game.result, GameResult::Draw);
        Ok(())
    })?;
    fixture(fen, 149, |game, info| {
        play(game, info, *b"a1a2\0")?;
        assert_eq!(game.result, GameResult::Draw);
        Ok(())
    })?;
    fixture("7k/8/5KQ1/8/8/8/8/8 w - - 0 1", 149, |game, info| {
        play(game, info, *b"g6g7\0")?;
        assert_eq!(game.result, GameResult::Winner(game.white));
        Ok(())
    })
}

#[test]
fn false_claim_penalty_obligation_and_replay_guard_are_durable() -> anyhow::Result<()> {
    fixture("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 0, |game, info| {
        let before = game.board_state;
        adjudicate_claim(game, info, game.white, game.nonce, 0, Some(*b"g1f3 "), true, 101)?;
        assert_eq!(game.status, GameStatus::Active);
        assert_eq!(game.board_state, before);
        assert_eq!(game.draw_offered_by, Some(game.white));
        let persisted = info.try_borrow_data()?.to_vec();
        assert!(adjudicate_claim(game, info, game.white, game.nonce, 0, None, true, 101).is_err());
        assert_eq!(&**info.try_borrow_data()?, persisted.as_slice());
        {
            let mut data = info.try_borrow_mut_data()?;
            let state = FideState::load(&mut data, game)?;
            assert_eq!(state.clock(game)?.remaining(-1).map_err(|e| anyhow::anyhow!("{e:?}"))?, 420_000);
            assert_eq!(state.claims(game)?.mandatory_move(), Some(*b"g1f3\0"));
        }
        assert!(play(game, info, *b"b1c3\0").is_err());
        play(game, info, *b"g1f3\0")?;
        Ok(())
    })
}

#[test]
fn legacy_accounts_fail_closed_and_borsh_prefix_writes_preserve_history() -> anyhow::Result<()> {
    let mut game = game("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let mut old = vec![0; LEGACY_GAME_SPACE];
    assert!(FideState::load(&mut old, &game).is_err());
    let mut bytes = vec![0; FIDE_GAME_SPACE];
    FideState::initialize(&mut bytes, &game)?;
    let tail = bytes[LEGACY_GAME_SPACE..].to_vec();
    game.result = GameResult::Winner(game.white);
    game.draw_offered_by = Some(game.black);
    game.try_serialize(&mut bytes.as_mut_slice())?;
    ensure!(bytes[LEGACY_GAME_SPACE..] == tail, "variable Borsh prefix overwrote FIDE state");
    Ok(())
}

#[test]
fn impossible_mate_is_a_draw_and_real_material_keeps_the_ordinary_result() -> anyhow::Result<()> {
    let start = CompactBoard::starting_position();
    assert!(mating_possible(&start, -1)?);
    assert!(mating_possible(&start, 1)?);
    let bare = CompactBoard::from_fen("8/8/4k3/8/8/4K3/8/1N3N2 w - - 0 1");
    // Two knights are never proven impossible; a lone knight is.
    assert!(mating_possible(&bare, 1)?);
    let lone = CompactBoard::from_fen("8/8/4k3/8/8/4K3/8/1N6 w - - 0 1");
    assert!(!mating_possible(&lone, 1)?);
    assert!(!mating_possible(&lone, -1)?);
    Ok(())
}

#[test]
fn resignation_and_timeout_resolve_with_ordinary_material_and_no_witness() -> anyhow::Result<()> {
    let start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    fixture(start, 0, |game, info| {
        let (white, black) = (game.white, game.black);
        finish_resignation(game, info, white, 101)?;
        assert_eq!(game.result, GameResult::Winner(black));
        Ok(())
    })?;
    fixture("8/8/4k3/8/8/4K3/8/1N6 w - - 0 1", 0, |game, info| {
        let white = game.white;
        finish_resignation(game, info, white, 101)?;
        assert_eq!(game.result, GameResult::Draw);
        Ok(())
    })?;
    fixture(start, 0, |game, info| {
        // White to move; 300s elapsed before the claim.
        assert!(finish_timeout(game, info, 100 + 400, true)?);
        assert_eq!(game.result, GameResult::Winner(game.black));
        Ok(())
    })?;
    fixture("8/8/4k3/8/8/4K3/8/1N6 w - - 0 1", 0, |game, info| {
        assert!(finish_timeout(game, info, 100 + 400, true)?);
        assert_eq!(game.result, GameResult::Draw);
        Ok(())
    })
}
