//! Mirrors a Swiss event onto the chain, signed by the tournament authority:
//! `record_swiss_result` per board (a bye is an extra board after the real ones),
//! `advance_round` once all of a round's boards are in, then
//! `complete_swiss_tournament` with the off-chain top 10.
//!
//! Progress lives in the store (`chain_boards_recorded`, `chain_completed`) and the
//! on-chain `Tournament.current_round`, so a restart, a dropped event or a failed
//! transaction is repaired by the next reconcile pass.

use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use swiss_pairing::MatchResult;
use tracing::{error, info, warn};

use crate::signing::orchestrator::ChainConfig;
use crate::signing::solana::instructions::{
    SWISS_BLACK_WIN, SWISS_BYE, SWISS_DRAW, SWISS_WHITE_WIN,
};
use crate::signing::solana::tournament_account::{
    fetch_swiss_progress, TOURNAMENT_STATUS_ACTIVE, TOURNAMENT_STATUS_COMPLETED,
};
use crate::signing::storage::tournament::{
    FormatState, TournamentRecord, TournamentStatus, TournamentStore,
};

use super::state::SwissStorageData;

/// Anchor code for `GameErrorCode::BoardAlreadyRecorded` (6000 + variant index): the
/// board landed on an earlier attempt whose confirmation was lost.
const BOARD_ALREADY_RECORDED: &str = "custom program error: 0x17cf";

/// Brings the on-chain tournament up to the store: record every decided board of
/// the on-chain round, advance once all are in, and complete after the last round.
pub async fn reconcile(store: &TournamentStore, cfg: &ChainConfig, t: &TournamentRecord) {
    let FormatState::Swiss(sd) = &t.state else {
        return;
    };
    if sd.chain_completed {
        return;
    }
    let Ok(program_id) = Pubkey::from_str(&cfg.program_id) else {
        error!("[swiss] invalid program ID {}", cfg.program_id);
        return;
    };
    let tournament_id = t.tournament_id;
    let max_players = t.max_players;
    let off_chain_completed = t.status == TournamentStatus::Completed;
    let places: Vec<String> = sd
        .standings
        .iter()
        .take(10)
        .map(|s| s.player_id.clone())
        .collect();
    let (cfg, sd) = (cfg.clone(), sd.clone());

    let outcome = tokio::task::spawn_blocking(move || {
        mirror_to_chain(
            &cfg,
            &program_id,
            tournament_id,
            max_players,
            &sd,
            off_chain_completed,
            &places,
        )
    })
    .await;

    let progress = match outcome {
        Ok(progress) => progress,
        Err(e) => {
            error!("[swiss] reconcile task for tournament {tournament_id} panicked: {e}");
            return;
        }
    };
    if let Some(e) = &progress.error {
        warn!("[swiss] tournament {tournament_id} chain reconcile stopped: {e}");
    }
    if progress.recorded.is_empty() && !progress.completed {
        return;
    }
    store
        .update(tournament_id, |t| {
            if let Some(sd) = t.swiss_mut() {
                for board in &progress.recorded {
                    if !sd.chain_boards_recorded.contains(board) {
                        sd.chain_boards_recorded.push(*board);
                    }
                }
                sd.chain_completed |= progress.completed;
            }
        })
        .await;
    if progress.completed {
        info!("[swiss] tournament {tournament_id} completed on-chain");
    }
}

#[derive(Default)]
struct ChainProgress {
    /// `(round, board)` pairs (backend numbering) newly confirmed on-chain.
    recorded: Vec<(u8, u16)>,
    completed: bool,
    error: Option<String>,
}

fn chain_result(result: MatchResult) -> Option<u8> {
    match result {
        MatchResult::WhiteWin | MatchResult::ForfeitWhiteWin => Some(SWISS_WHITE_WIN),
        MatchResult::BlackWin | MatchResult::ForfeitBlackWin => Some(SWISS_BLACK_WIN),
        MatchResult::Draw => Some(SWISS_DRAW),
        MatchResult::Bye => None,
    }
}

/// Blocking body of [`reconcile`]. Backend rounds and boards are 1-based; the
/// program's are 0-based, and this is the only place that converts.
fn mirror_to_chain(
    cfg: &ChainConfig,
    program_id: &Pubkey,
    tournament_id: u64,
    max_players: u16,
    sd: &SwissStorageData,
    off_chain_completed: bool,
    places: &[String],
) -> ChainProgress {
    use crate::signing::solana::{
        advance_round_ix, complete_swiss_tournament_ix, make_rpc, record_swiss_result_ix,
        sign_and_submit,
    };

    let mut progress = ChainProgress::default();
    let rpc = match make_rpc(&cfg.rpc_url) {
        Ok(rpc) => rpc,
        Err(error) => {
            return ChainProgress {
                error: Some(error.to_string()),
                ..Default::default()
            }
        }
    };
    let authority = cfg.authority.as_ref();
    let mut chain = match fetch_swiss_progress(&rpc, program_id, tournament_id) {
        Ok(chain) => chain,
        Err(e) => {
            progress.error = Some(e);
            return progress;
        }
    };
    if chain.status == TOURNAMENT_STATUS_COMPLETED {
        progress.completed = true;
        return progress;
    }
    if chain.status != TOURNAMENT_STATUS_ACTIVE {
        // Not started on-chain yet; the scheduler's start will land first.
        return progress;
    }

    while chain.current_round < chain.total_rounds {
        let round = chain.current_round + 1;
        let Some(paired) = sd.round(round) else {
            return progress;
        };

        // Board slots for this round: decided pairings at their own board number,
        // then each bye as a pseudo-board after the last real one.
        let mut slots: Vec<(u16, u8, &str, &str)> = paired
            .pairings
            .iter()
            .filter_map(|p| {
                Some((
                    p.board,
                    chain_result(sd.result(round, p.board)?)?,
                    p.white.as_str(),
                    p.black.as_str(),
                ))
            })
            .collect();
        let first_bye_board = paired.pairings.len() as u16 + 1;
        slots.extend(paired.byes.iter().enumerate().map(|(i, bye)| {
            (
                first_bye_board + i as u16,
                SWISS_BYE,
                bye.as_str(),
                bye.as_str(),
            )
        }));
        let total_slots = paired.pairings.len() + paired.byes.len();

        for (board, variant, white, black) in slots {
            let key = (round, board);
            if sd.chain_boards_recorded.contains(&key) {
                continue;
            }
            let (Ok(white), Ok(black)) = (Pubkey::from_str(white), Pubkey::from_str(black)) else {
                progress.error = Some(format!("round {round} board {board}: non-pubkey player id"));
                return progress;
            };
            let ix = record_swiss_result_ix(
                program_id,
                tournament_id,
                max_players,
                chain.current_round,
                board - 1,
                variant,
                &white,
                &black,
                &authority.pubkey(),
            );
            match sign_and_submit(&rpc, authority, &[ix]) {
                Ok(_) => progress.recorded.push(key),
                Err(e)
                    if e.to_string()
                        .to_lowercase()
                        .contains(BOARD_ALREADY_RECORDED) =>
                {
                    progress.recorded.push(key)
                }
                Err(e) => {
                    progress.error = Some(format!(
                        "record_swiss_result round {round} board {board}: {e}"
                    ));
                    return progress;
                }
            }
        }

        let all_recorded = (1..=total_slots as u16).all(|board| {
            sd.chain_boards_recorded.contains(&(round, board))
                || progress.recorded.contains(&(round, board))
        });
        if !sd.round_complete(round) || !all_recorded {
            return progress;
        }

        let ix = advance_round_ix(
            program_id,
            tournament_id,
            total_slots as u16,
            &authority.pubkey(),
        );
        if let Err(e) = sign_and_submit(&rpc, authority, &[ix]) {
            progress.error = Some(format!("advance_round from round {round}: {e}"));
            return progress;
        }
        chain.current_round += 1;
    }

    if !off_chain_completed {
        return progress;
    }
    let places: Result<Vec<Pubkey>, _> = places.iter().map(|p| Pubkey::from_str(p)).collect();
    let Ok(places) = places else {
        progress.error = Some("final standings contain a non-pubkey player id".into());
        return progress;
    };
    let ix = complete_swiss_tournament_ix(
        program_id,
        tournament_id,
        max_players,
        &places,
        &authority.pubkey(),
    );
    match sign_and_submit(&rpc, authority, &[ix]) {
        Ok(_) => progress.completed = true,
        Err(e) => progress.error = Some(format!("complete_swiss_tournament: {e}")),
    }
    progress
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scored_result_maps_to_a_chain_variant() {
        assert_eq!(chain_result(MatchResult::WhiteWin), Some(SWISS_WHITE_WIN));
        assert_eq!(
            chain_result(MatchResult::ForfeitWhiteWin),
            Some(SWISS_WHITE_WIN)
        );
        assert_eq!(chain_result(MatchResult::BlackWin), Some(SWISS_BLACK_WIN));
        assert_eq!(
            chain_result(MatchResult::ForfeitBlackWin),
            Some(SWISS_BLACK_WIN)
        );
        assert_eq!(chain_result(MatchResult::Draw), Some(SWISS_DRAW));
        assert_eq!(chain_result(MatchResult::Bye), None);
    }
}
