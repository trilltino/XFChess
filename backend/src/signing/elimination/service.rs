//! Every single-elimination state transition on a stored tournament: settled games,
//! admin results and forfeits. On-chain mirroring is [`super::chain`]'s job and reads
//! only what this persists, so a failed transaction never loses a result.

use crate::signing::storage::tournament::{
    FormatState, GameOutcome, ResultSource, TournamentRecord, TournamentStatus, TournamentStore,
};

use super::bracket::{Decision, EliminationError, EliminationState, GameApplied, Stage};

fn active_bracket(t: &mut TournamentRecord) -> Result<&mut EliminationState, EliminationError> {
    if t.status != TournamentStatus::Active {
        return Err(EliminationError::NotActive);
    }
    match &mut t.state {
        FormatState::Elimination(bracket) => Ok(bracket),
        FormatState::Swiss(_) => Err(EliminationError::NotElimination),
    }
}

/// Fills the placings a decided match settles; the final completes the event.
fn apply_placing(t: &mut TournamentRecord, decision: &Decision) {
    match decision.stage {
        Stage::Final => {
            t.winner = Some(decision.winner.clone());
            t.second_place = Some(decision.loser.clone());
            t.status = TournamentStatus::Completed;
            t.completed_at = Some(chrono::Utc::now().timestamp());
        }
        Stage::SemiFinal { loser_place: 3 } => t.third_place = Some(decision.loser.clone()),
        Stage::SemiFinal { .. } => t.fourth_place = Some(decision.loser.clone()),
        Stage::Earlier => {}
    }
}

/// Applies a settled game to `t` (see [`EliminationState::apply_game`]).
pub fn record_game(
    t: &mut TournamentRecord,
    game_id: u64,
    white: &str,
    black: &str,
    outcome: GameOutcome,
) -> Result<GameApplied, EliminationError> {
    if t.status != TournamentStatus::Active {
        return Err(EliminationError::NotActive);
    }
    let tournament_id = t.tournament_id;
    let FormatState::Elimination(bracket) = &mut t.state else {
        return Err(EliminationError::NotElimination);
    };
    // Seeds are the players in seeding order (best first) since the event started.
    let applied = bracket.apply_game(tournament_id, game_id, white, black, outcome, &t.players)?;
    if let GameApplied::Decided(decision) = &applied {
        apply_placing(t, decision);
    }
    Ok(applied)
}

/// Decides `match_index` without a game result (forfeit, admin entry).
pub fn record_decision(
    t: &mut TournamentRecord,
    match_index: u16,
    winner: &str,
    loser: &str,
    source: ResultSource,
) -> Result<Decision, EliminationError> {
    let tournament_id = t.tournament_id;
    let decision = active_bracket(t)?.decide(tournament_id, match_index, winner, loser, source)?;
    apply_placing(t, &decision);
    Ok(decision)
}

/// [`record_game`] and [`record_decision`] against the store.
#[derive(Clone)]
pub struct EliminationService {
    store: TournamentStore,
}

impl EliminationService {
    pub fn new(store: TournamentStore) -> Self {
        Self { store }
    }

    async fn apply<T>(
        &self,
        tournament_id: u64,
        change: impl FnOnce(&mut TournamentRecord) -> Result<T, EliminationError>,
    ) -> Result<T, EliminationError> {
        let mut outcome = Err(EliminationError::TournamentNotFound);
        let stored = self
            .store
            .update(tournament_id, |t| outcome = change(t))
            .await;
        if !stored && outcome.is_ok() {
            return Err(EliminationError::TournamentNotFound);
        }
        outcome
    }

    pub async fn record_game(
        &self,
        tournament_id: u64,
        game_id: u64,
        white: &str,
        black: &str,
        outcome: GameOutcome,
    ) -> Result<GameApplied, EliminationError> {
        self.apply(tournament_id, |t| {
            record_game(t, game_id, white, black, outcome)
        })
        .await
    }

    pub async fn record_decision(
        &self,
        tournament_id: u64,
        match_index: u16,
        winner: &str,
        loser: &str,
        source: ResultSource,
    ) -> Result<Decision, EliminationError> {
        self.apply(tournament_id, |t| {
            record_decision(t, match_index, winner, loser, source)
        })
        .await
    }

    pub async fn set_game_id(
        &self,
        tournament_id: u64,
        match_index: u16,
        game_id: u64,
    ) -> Result<(), EliminationError> {
        self.apply(tournament_id, |t| {
            active_bracket(t)?.set_game_id(match_index, game_id)
        })
        .await
    }

    /// Records matches confirmed on-chain by the chain mirror.
    pub async fn mark_synced(&self, tournament_id: u64, match_indices: &[u16]) {
        self.store
            .update(tournament_id, |t| {
                if let FormatState::Elimination(bracket) = &mut t.state {
                    for &index in match_indices {
                        bracket.mark_synced(index);
                    }
                }
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::storage::tournament::TournamentFormat;
    use anyhow::Context as _;

    fn started(n: usize) -> TournamentRecord {
        let mut t = TournamentRecord::new(5, "t", 0);
        t.max_players = n as u16;
        t.players = (0..n).map(|i| format!("P{i}")).collect();
        t.player_elos = (0..n).map(|i| 2000 - i as u32).collect();
        let seeds = t.players.clone();
        t.state = FormatState::Elimination(EliminationState::draw(5, &seeds));
        t.status = TournamentStatus::Active;
        t
    }

    fn game(t: &TournamentRecord, index: usize) -> anyhow::Result<u64> {
        Ok({
            t.elimination().context("test setup failed")?.matches[index]
                .game_id
                .context("test setup failed")?
        })
    }

    #[test]
    fn semifinals_place_third_and_fourth_and_the_final_completes() -> anyhow::Result<()> {
        let mut t = started(4);
        let g = game(&t, 0)?;
        record_game(&mut t, g, "P0", "P3", GameOutcome::WhiteWin).context("test setup failed")?;
        let g = game(&t, 1)?;
        record_game(&mut t, g, "P1", "P2", GameOutcome::WhiteWin).context("test setup failed")?;
        assert_eq!(t.fourth_place.as_deref(), Some("P3"));
        assert_eq!(t.third_place.as_deref(), Some("P2"));
        assert_eq!(t.status, TournamentStatus::Active);

        let g = game(&t, 2)?;
        record_game(&mut t, g, "P0", "P1", GameOutcome::BlackWin).context("test setup failed")?;
        assert_eq!(t.status, TournamentStatus::Completed);
        assert_eq!(t.winner.as_deref(), Some("P1"));
        assert_eq!(t.second_place.as_deref(), Some("P0"));
        Ok(())
    }

    #[test]
    fn a_two_player_event_has_no_semifinal_places() -> anyhow::Result<()> {
        let mut t = started(2);
        record_decision(&mut t, 0, "P1", "P0", ResultSource::Forfeit)
            .context("test setup failed")?;
        assert_eq!(t.status, TournamentStatus::Completed);
        assert_eq!(t.winner.as_deref(), Some("P1"));
        assert!(t.third_place.is_none() && t.fourth_place.is_none());
        Ok(())
    }

    #[test]
    fn only_active_brackets_take_results() {
        let mut t = started(2);
        t.status = TournamentStatus::Cancelled;
        assert_eq!(
            record_decision(&mut t, 0, "P1", "P0", ResultSource::Oracle),
            Err(EliminationError::NotActive)
        );
        let mut swiss = started(2);
        swiss.state = FormatState::for_format(&TournamentFormat::Swiss { rounds: 3 });
        assert_eq!(
            record_decision(&mut swiss, 0, "P1", "P0", ResultSource::Oracle),
            Err(EliminationError::NotElimination)
        );
    }
}
