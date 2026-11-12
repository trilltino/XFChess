use anchor_lang::prelude::*;
use crate::{constants::RECORD_RESULT_COST, errors::GameErrorCode as E, state::{Game, GameResult, GameStatus}};
use nimzovich_engine::{adjudication::*, on_chain_replay::record_verified_move, on_chain_rules::{canonical_position_key, mating_possibility, MatingPossibility}, CompactBoard, parse_uci, validate_and_apply, MoveOutcome};
use super::*;

pub fn finish(game: &mut Game, state: &mut FideState<'_>, reason: FideTermination, winner: Option<Pubkey>, now: i64) -> Result<()> {
    game.status = GameStatus::Finished;
    game.result = winner.map_or(GameResult::Draw, GameResult::Winner);
    game.draw_offered_by = None;
    game.updated_at = now;
    state.bytes[10] = reason as u8;
    state.bytes[11] = 0;
    state.bytes[65] = 1;
    emit!(FideGameEnded { game_id: game.game_id, reason, winner, timestamp: now });
    Ok(())
}

pub fn apply_move(game: &mut Game, info: &AccountInfo<'_>, moving_player: Pubkey, uci: [u8; 5], next: [u8; 68], nonce: u64, parent: Option<u64>, now: i64) -> Result<()> {
    require!(game.status == GameStatus::Active, E::GameNotActive);
    let side = if game.turn % 2 == 1 { 1 } else { -1 };
    require!(moving_player == if side == 1 { game.white } else { game.black }, E::NotYourTurn);
    if let Some(parent) = parent { require!(parent == game.nonce, E::ParentNonceMismatch); }
    require!(nonce == game.nonce.checked_add(1).ok_or(E::ArithmeticOverflow)?, E::InvalidNonce);
    let move_count = game.move_count.checked_add(1).ok_or(E::ArithmeticOverflow)?;
    let turn = game.turn.checked_add(1).ok_or(E::ArithmeticOverflow)?;
    let fees = game.fees_advanced.checked_add(RECORD_RESULT_COST).ok_or(E::ArithmeticOverflow)?;
    let mut data = info.try_borrow_mut_data()?;
    let mut state = FideState::load(&mut data, game)?;
    let mut claims = state.claims(game)?;
    claims.check_move_allowed(uci).map_err(|_| error!(E::DeclaredMoveRequired))?;
    let mut clock = state.clock(game)?;
    clock.complete_move(timestamp_millis(now)?).map_err(|_| error!(E::InvalidFideClock))?;
    let mut board = CompactBoard::from_bytes(&game.board_state);
    let (terminal, checkpoint) = {
        let mut tracker = state.tracker()?;
        let terminal = record_verified_move(&mut board, &mut tracker, &uci, &next, u32::from(move_count)).map_err(|_| error!(E::InvalidMove))?;
        (terminal, tracker.checkpoint())
    };
    claims.record_accepted_move(uci, side).map_err(rules_error)?;
    state.save_checkpoint(checkpoint)?;
    state.save_clock(&clock)?;
    state.save_claims(&claims)?;
    game.board_state = next;
    game.move_count = move_count;
    game.turn = turn;
    game.halfmove_clock = checkpoint.halfmoves;
    game.nonce = nonce;
    game.fees_advanced = fees;
    if game.draw_offered_by.is_some_and(|offerer| offerer != moving_player) { game.draw_offered_by = None; }
    crate::lifecycle::clock::mark_activity(game, now);
    if let Some(reason) = terminal {
        let winner = if reason == Termination::Checkmate { Some(moving_player) } else { None };
        finish(game, &mut state, reason.into(), winner, now)?;
    }
    Ok(())
}

pub fn adjudicate_claim(game: &mut Game, info: &AccountInfo<'_>, claimant: Pubkey, expected_nonce: u64, claim_nonce: u64, intended: Option<[u8; 5]>, repetition: bool, now: i64) -> Result<()> {
    require!(game.status == GameStatus::Active, E::GameNotActive);
    require!(game.nonce == expected_nonce, E::ParentNonceMismatch);
    let side = if game.turn % 2 == 1 { 1 } else { -1 };
    require!(claimant == if side == 1 { game.white } else { game.black }, E::NotYourTurn);
    let mut data = info.try_borrow_mut_data()?;
    let mut state = FideState::load(&mut data, game)?;
    let mut claims = state.claims(game)?;
    let mut clock = state.clock(game)?;
    let board = CompactBoard::from_bytes(&game.board_state);
    let eligibility = {
        let tracker = state.tracker()?;
        require!(tracker.current_key() == canonical_position_key(&board).map_err(rules_error)?, E::InvalidFideState);
        require!(automatic_termination(PositionFacts::default(), tracker.occurrences(tracker.current_key()), tracker.checkpoint().halfmoves).is_none(), E::GameNotActive);
        if let Some(uci) = intended {
            let mut preview = board.to_on_chain_game();
            validate_and_apply(&mut preview, &uci).map_err(|_| error!(E::InvalidMove))?;
            let (from, to, _) = parse_uci(&uci).map_err(|_| error!(E::InvalidMove))?;
            let reset = matches!(board.squares[from as usize], 1 | -1) || board.squares[to as usize] != 0;
            tracker.prospective_claims(canonical_position_key(&preview.to_compact_board()).map_err(rules_error)?, reset).map_err(rules_error)?
        } else { tracker.current_claims() }
    };
    let request = ClaimRequest { game: game_claim_id(game.game_id), ply: u32::from(game.move_count), nonce: claim_nonce,
        claimant: side, reason: if repetition { ClaimReason::Repetition } else { ClaimReason::FiftyMoves }, intended_move: intended };
    claims.receive_authenticated(request, u32::from(game.move_count), &mut clock, timestamp_millis(now)?).map_err(|_| error!(E::InvalidFideClaim))?;
    let decision = claims.resolve(eligibility, &mut clock, timestamp_millis(now)?).map_err(rules_error)?;
    state.save_clock(&clock)?;
    state.save_claims(&claims)?;
    let confirmed = if let ClaimDecision::Confirmed(reason) = decision {
        finish(game, &mut state, reason.into(), None, now)?;
        true
    } else {
        game.draw_offered_by = Some(claimant);
        // A substantive false claim succeeds as an instruction, recording the
        // penalty/nonce/obligation. Transport and malformed errors revert.
        game.updated_at = now;
        false
    };
    emit!(FideClaimAdjudicated { game_id: game.game_id, claimant, claim_nonce, move_nonce: game.nonce,
        repetition_claim: repetition, confirmed, intended_move: intended,
        remaining_millis: clock.checkpoint().remaining_millis, timestamp: now });
    Ok(())
}

/// Whether `attacker` could ever checkmate. Only a sound material proof of
/// impossibility yields `false` (a draw). Everything else, including exotic
/// locked fortresses that material cannot classify, keeps the ordinary
/// result and is left to the dispute path: demanding a constructive witness
/// would stop every resignation and crank timeout in a game with real material.
pub fn mating_possible(board: &CompactBoard, attacker: i8) -> Result<bool> {
    Ok(mating_possibility(board, attacker).map_err(rules_error)? != MatingPossibility::Impossible)
}

pub fn finish_resignation(game: &mut Game, info: &AccountInfo<'_>, resigner: Pubkey, now: i64) -> Result<()> {
    require!(game.status == GameStatus::Active, E::GameNotActive);
    require!(resigner == game.white || resigner == game.black, E::NotInGame);
    let attacker = if resigner == game.white { -1 } else { 1 };
    let possible = mating_possible(&CompactBoard::from_bytes(&game.board_state), attacker)?;
    let mut data = info.try_borrow_mut_data()?;
    let mut state = FideState::load(&mut data, game)?;
    let clock = state.clock(game)?;
    require!(clock.flag_fallen(timestamp_millis(now)?).map_err(rules_error)?.is_none(), E::InvalidFideClock);
    let winner = if possible { Some(if attacker == 1 { game.white } else { game.black }) } else { None };
    finish(game, &mut state, if possible { FideTermination::Resignation } else { FideTermination::ResignationWithoutPossibleMate }, winner, now)
}

pub fn finish_timeout(game: &mut Game, info: &AccountInfo<'_>, now: i64, optional: bool) -> Result<bool> {
    if optional && game.status != GameStatus::Active { return Ok(false); }
    require!(game.status == GameStatus::Active, E::GameNotActive);
    let mut data = info.try_borrow_mut_data()?;
    let mut state = FideState::load(&mut data, game)?;
    let clock = state.clock(game)?;
    let flagged = clock.flag_fallen(timestamp_millis(now)?).map_err(rules_error)?;
    if flagged.is_none() && optional { return Ok(false); }
    let flagged = flagged.ok_or(E::TimeoutNotExpired)?;
    let board = CompactBoard::from_bytes(&game.board_state);
    let possible = mating_possible(&board, -flagged)?;
    let winner = if possible { Some(if flagged == 1 { game.black } else { game.white }) } else { None };
    finish(game, &mut state, if possible { FideTermination::FlagFall } else { FideTermination::FlagFallWithoutPossibleMate }, winner, now)?;
    Ok(true)
}

pub fn finish_agreement(game: &mut Game, info: &AccountInfo<'_>, accepter: Pubkey, now: i64) -> Result<()> {
    let mut data = info.try_borrow_mut_data()?;
    let mut state = FideState::load(&mut data, game)?;
    let clock = state.clock(game)?;
    require!(clock.flag_fallen(timestamp_millis(now)?).map_err(rules_error)?.is_none(), E::InvalidFideClock);
    crate::lifecycle::terminal::finish_by_draw_agreement(game, accepter, now)?;
    finish(game, &mut state, FideTermination::Agreement, None, now)
}
