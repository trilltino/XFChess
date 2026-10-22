//! Mirrors a bracket onto the chain, signed by the tournament authority.
//!
//! Results are decided off-chain first ([`super::service`]) and marked
//! `chain_synced` only once both `record_result` and the winner's
//! `advance_winner` have landed. Each step checks the on-chain match first, so a
//! retry after a lost confirmation or a restart never re-sends a landed
//! transaction. Unsynced matches are retried by the orchestrator's reconcile pass.

use std::str::FromStr;

use solana_client::rpc_client::RpcClient;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use tracing::{error, info, warn};

use crate::signing::orchestrator::ChainConfig;
use crate::signing::solana::tournament_account::{fetch_bracket_match, MATCH_STATUS_COMPLETED};
use crate::signing::solana::{
    advance_winner_ix, initialize_match_ix, make_rpc, record_result_ix, sign_and_submit,
};
use crate::signing::storage::tournament::{FormatState, TournamentRecord};

use super::bracket::{EliminationState, TournamentMatch};
use super::service::EliminationService;

/// `initialize_match` for every match of a freshly drawn bracket, round one through
/// the final, in transaction-sized batches. Indices, rounds and pointers come from
/// `bracket`, the same draw the store keeps, so the two cannot disagree.
pub fn initialize_match_batches(
    program_id: &Pubkey,
    authority: &Pubkey,
    tournament_id: u64,
    bracket: &EliminationState,
) -> Vec<Vec<Instruction>> {
    const BATCH: usize = 20;
    let seat = |player: &Option<String>| player.as_deref().and_then(|p| Pubkey::from_str(p).ok());
    bracket
        .matches
        .chunks(BATCH)
        .map(|batch| {
            batch
                .iter()
                .map(|m| {
                    initialize_match_ix(
                        program_id,
                        crate::signing::solana::MatchInit {
                            tournament_id,
                            match_index: m.match_index,
                            round: m.round,
                            player_white: seat(&m.player_white).as_ref(),
                            player_black: seat(&m.player_black).as_ref(),
                            next_match_for_winner: m.next_match_for_winner,
                            next_match_slot: m.next_match_slot,
                            authority,
                        },
                    )
                })
                .collect()
        })
        .collect()
}

/// Creates every match account of a freshly drawn bracket on-chain.
pub fn initialize_matches(
    rpc: &RpcClient,
    program_id: &Pubkey,
    authority: &Keypair,
    tournament_id: u64,
    bracket: &EliminationState,
) -> Result<(), String> {
    let batches = initialize_match_batches(program_id, &authority.pubkey(), tournament_id, bracket);
    for (batch, ixs) in batches.iter().enumerate() {
        sign_and_submit(rpc, authority, ixs)
            .map_err(|e| format!("initialize_match batch {batch}: {e}"))?;
    }
    Ok(())
}

/// Brings the on-chain bracket up to the store and records what landed.
pub async fn reconcile(service: &EliminationService, cfg: &ChainConfig, t: &TournamentRecord) {
    let FormatState::Elimination(bracket) = &t.state else {
        return;
    };
    if bracket.unsynced().next().is_none() {
        return;
    }
    let Ok(program_id) = Pubkey::from_str(&cfg.program_id) else {
        error!("[elimination] invalid program ID {}", cfg.program_id);
        return;
    };
    let tournament_id = t.tournament_id;
    let (cfg, bracket) = (cfg.clone(), bracket.clone());
    let progress = match tokio::task::spawn_blocking(move || {
        mirror_to_chain(&cfg, &program_id, tournament_id, &bracket)
    })
    .await
    {
        Ok(progress) => progress,
        Err(e) => {
            error!("[elimination] tournament {tournament_id} chain mirror panicked: {e}");
            return;
        }
    };
    if let Some(e) = &progress.error {
        warn!("[elimination] tournament {tournament_id} chain reconcile stopped: {e}");
    }
    if !progress.synced.is_empty() {
        service.mark_synced(tournament_id, &progress.synced).await;
        info!(
            "[elimination] tournament {tournament_id}: matches {:?} confirmed on-chain",
            progress.synced
        );
    }
}

#[derive(Debug, Default)]
struct ChainProgress {
    synced: Vec<u16>,
    error: Option<String>,
}

/// Blocking body of [`reconcile`]. Stops at the first failure: a later match's seat
/// is filled by an earlier match's advance, so bracket order matters.
fn mirror_to_chain(
    cfg: &ChainConfig,
    program_id: &Pubkey,
    tournament_id: u64,
    bracket: &EliminationState,
) -> ChainProgress {
    let rpc = match make_rpc(&cfg.rpc_url) {
        Ok(rpc) => rpc,
        Err(error) => {
            return ChainProgress {
                error: Some(error.to_string()),
                ..Default::default()
            }
        }
    };
    let mut progress = ChainProgress::default();
    for m in bracket.unsynced() {
        match sync_match(&rpc, program_id, &cfg.authority, tournament_id, m) {
            Ok(()) => progress.synced.push(m.match_index),
            Err(e) => {
                progress.error = Some(format!("match {}: {e}", m.match_index));
                break;
            }
        }
    }
    progress
}

fn sync_match(
    rpc: &RpcClient,
    program_id: &Pubkey,
    authority: &Keypair,
    tournament_id: u64,
    m: &TournamentMatch,
) -> Result<(), String> {
    let pubkey = |player: Option<&str>| {
        player
            .and_then(|p| Pubkey::from_str(p).ok())
            .ok_or_else(|| format!("{player:?} is not a wallet"))
    };
    let winner = pubkey(m.winner.as_deref())?;
    let loser = pubkey(m.loser())?;

    let on_chain = fetch_bracket_match(rpc, program_id, tournament_id, m.match_index)?;
    if on_chain.status == MATCH_STATUS_COMPLETED {
        if on_chain.winner != Some(winner) {
            return Err(format!(
                "on-chain winner {:?} differs from {winner}",
                on_chain.winner
            ));
        }
    } else {
        let ix = record_result_ix(
            program_id,
            tournament_id,
            m.match_index,
            &winner,
            &loser,
            &authority.pubkey(),
        );
        sign_and_submit(rpc, authority, &[ix]).map_err(|e| format!("record_result: {e}"))?;
    }

    let Some(next) = m.next_match_for_winner else {
        return Ok(());
    };
    let next_on_chain = fetch_bracket_match(rpc, program_id, tournament_id, next)?;
    let seated = if m.next_match_slot == 0 {
        next_on_chain.player_white
    } else {
        next_on_chain.player_black
    };
    if seated != Some(winner) {
        let ix = advance_winner_ix(
            program_id,
            tournament_id,
            m.match_index,
            next,
            &authority.pubkey(),
        );
        sign_and_submit(rpc, authority, &[ix]).map_err(|e| format!("advance_winner: {e}"))?;
    }
    Ok(())
}
