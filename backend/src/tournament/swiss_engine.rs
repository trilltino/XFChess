//! Swiss pairing adapter - bridges swiss-pairing crate with backend tournament state

use swiss_pairing::{SwissPlayer, generate_pairings, calculate_standings, Color};
use crate::tournament::store::{TournamentRecord, TournamentMatch, MatchStatus, ResultSource, TournamentStatus};
use std::collections::HashMap;

/// Errors that can occur during Swiss pairing operations
#[derive(Debug, thiserror::Error)]
pub enum SwissError {
    #[error("Not a Swiss tournament")]
    NotSwissTournament,
    #[error("Swiss pairing error: {0}")]
    PairingError(#[from] swiss_pairing::SwissError),
    #[error("Invalid round number")]
    InvalidRound,
}

/// Generate pairings for a Swiss tournament round
pub fn generate_swiss_round(
    tournament: &mut TournamentRecord,
    round: u8,
) -> Result<Vec<TournamentMatch>, SwissError> {
    let swiss_state = tournament.swiss_state.as_ref()
        .ok_or(SwissError::NotSwissTournament)?;
    
    if round == 0 || round > swiss_state.total_rounds {
        return Err(SwissError::InvalidRound);
    }
    
    // Convert backend players to swiss-pairing types
    let swiss_players: Vec<SwissPlayer> = tournament.players.iter()
        .enumerate()
        .map(|(idx, pubkey)| {
            let elo = tournament.player_elos.get(idx).copied().unwrap_or(1200);
            let score = swiss_state.player_scores.get(pubkey).copied().unwrap_or(0.0);
            let byes = swiss_state.player_byes.get(pubkey).copied().unwrap_or(0);
            
            SwissPlayer {
                id: pubkey.clone(),
                rating: elo,
                score,
                color_history: extract_color_history(pubkey, &tournament.matches),
                opponents: extract_opponents(pubkey, &tournament.matches),
                bye_count: byes,
                float_status: swiss_pairing::FloatStatus::None,
            }
        })
        .collect();
    
    let swiss_round = generate_pairings(round, &swiss_players, swiss_state.total_rounds)?;
    
    // Convert back to TournamentMatch
    let base_index = tournament.matches.len() as u16;
    let matches: Vec<TournamentMatch> = swiss_round.pairings.iter()
        .enumerate()
        .map(|(idx, pairing)| TournamentMatch {
            match_index: base_index + idx as u16,
            round: round as u16,
            player_white: Some(pairing.white.clone()),
            player_black: Some(pairing.black.clone()),
            winner: None,
            game_id: None,
            status: MatchStatus::Pending,
            result_source: None,
        })
        .collect();
    
    // Record pairings in history
    let pairings: Vec<(String, String)> = swiss_round.pairings.iter()
        .map(|p| (p.white.clone(), p.black.clone()))
        .collect();
    
    if let Some(state) = tournament.swiss_state.as_mut() {
        state.pairings_history.push(pairings);
    }
    
    Ok(matches)
}

/// Initialize a Swiss tournament with first round pairings
pub fn start_swiss(tournament: &mut TournamentRecord, total_rounds: u8) -> Result<(), SwissError> {
    tournament.swiss_state = Some(crate::tournament::store::SwissState {
        current_round: 1,
        total_rounds,
        player_scores: HashMap::new(),
        player_byes: HashMap::new(),
        pairings_history: Vec::new(),
    });
    
    // Generate first round pairings
    let first_round = generate_swiss_round(tournament, 1)?;
    tournament.matches.extend(first_round);
    tournament.status = TournamentStatus::Active;
    tournament.started_at = Some(chrono::Utc::now().timestamp());
    
    Ok(())
}

/// Advance to the next Swiss round
pub fn advance_swiss_round(tournament: &mut TournamentRecord) -> Result<Vec<TournamentMatch>, SwissError> {
    let swiss_state = tournament.swiss_state.as_mut()
        .ok_or(SwissError::NotSwissTournament)?;
    
    if swiss_state.current_round >= swiss_state.total_rounds {
        // Tournament complete
        tournament.status = TournamentStatus::Completed;
        tournament.completed_at = Some(chrono::Utc::now().timestamp());
        return Ok(Vec::new());
    }
    
    swiss_state.current_round += 1;
    let next_round = generate_swiss_round(tournament, swiss_state.current_round)?;
    tournament.matches.extend(next_round.clone());
    
    Ok(next_round)
}

/// Record a match result and update Swiss standings
pub fn record_swiss_result(
    tournament: &mut TournamentRecord,
    match_index: usize,
    winner: Option<String>, // None = draw
) -> Result<(), SwissError> {
    let swiss_state = tournament.swiss_state.as_mut()
        .ok_or(SwissError::NotSwissTournament)?;
    
    let match_ref = tournament.matches.get_mut(match_index)
        .ok_or(SwissError::InvalidRound)?;
    
    if let Some(ref mut m) = match_ref {
        m.winner = winner.clone();
        m.status = MatchStatus::Completed;
        m.result_source = Some(ResultSource::OnChain);
        
        // Update player scores
        if let (Some(white), Some(black)) = (m.player_white.clone(), m.player_black.clone()) {
            let white_score = swiss_state.player_scores.entry(white.clone()).or_insert(0.0);
            let black_score = swiss_state.player_scores.entry(black.clone()).or_insert(0.0);
            
            match winner {
                Some(w) if w == white => {
                    *white_score += 1.0;
                }
                Some(w) if w == black => {
                    *black_score += 1.0;
                }
                _ => {
                    // Draw
                    *white_score += 0.5;
                    *black_score += 0.5;
                }
            }
        }
    }
    
    Ok(())
}

/// Extract color history for a player from completed matches
fn extract_color_history(player: &str, matches: &[TournamentMatch]) -> Vec<Color> {
    matches.iter()
        .filter(|m| m.status == MatchStatus::Completed)
        .filter_map(|m| {
            if m.player_white.as_deref() == Some(player) {
                Some(Color::White)
            } else if m.player_black.as_deref() == Some(player) {
                Some(Color::Black)
            } else {
                None
            }
        })
        .collect()
}

/// Extract list of opponents for a player
fn extract_opponents(player: &str, matches: &[TournamentMatch]) -> Vec<String> {
    matches.iter()
        .filter(|m| m.status == MatchStatus::Completed)
        .filter_map(|m| {
            if m.player_white.as_deref() == Some(player) {
                m.player_black.clone()
            } else if m.player_black.as_deref() == Some(player) {
                m.player_white.clone()
            } else {
                None
            }
        })
        .collect()
}

/// Calculate current Swiss standings
pub fn calculate_swiss_standings(tournament: &TournamentRecord) -> Result<Vec<(String, f64)>, SwissError> {
    let swiss_state = tournament.swiss_state.as_ref()
        .ok_or(SwissError::NotSwissTournament)?;
    
    let mut standings: Vec<(String, f64)> = swiss_state.player_scores
        .iter()
        .map(|(player, score)| (player.clone(), *score))
        .collect();
    
    // Sort by score descending
    standings.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    
    Ok(standings)
}
